use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use aionui_api_types::{
    CreateProductFactoryRunRequest, CreateTeamRequest, HandoffProductFactoryRequest, TeamAgentInput,
};
use aionui_db::{IProductFactoryRepository, SqliteProductFactoryRepository, models::TeamRow};
use aionui_product_factory::{ProductFactoryError, ProductFactoryService, ProductFactoryTeamPort, generate_task_draft};

#[derive(Default)]
struct TeamPort {
    prepared: AtomicUsize,
    discarded: AtomicUsize,
    fail: AtomicBool,
    invalid_team: AtomicBool,
}

#[async_trait::async_trait]
impl ProductFactoryTeamPort for TeamPort {
    async fn prepare_team(
        &self,
        user: &str,
        id: &str,
        request: CreateTeamRequest,
    ) -> Result<TeamRow, ProductFactoryError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        if self.fail.load(Ordering::SeqCst) {
            return Err(ProductFactoryError::TeamPreparation(
                "configured assistant unavailable".into(),
            ));
        }
        Ok(TeamRow {
            id: id.into(),
            user_id: user.into(),
            name: if self.invalid_team.load(Ordering::SeqCst) {
                "different".into()
            } else {
                request.name
            },
            workspace: request.workspace.unwrap(),
            workspace_mode: "shared".into(),
            agents: "[]".into(),
            lead_agent_id: None,
            session_mode: None,
            agents_version: "1.0.1".into(),
            created_at: 20,
            updated_at: 20,
            project_id: None,
            folder_id: None,
        })
    }

    async fn discard_team(&self, _user: &str, _team: &TeamRow) -> Result<(), ProductFactoryError> {
        self.discarded.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn request(revision: u64) -> HandoffProductFactoryRequest {
    HandoffProductFactoryRequest {
        expected_revision: revision,
        agents: vec![TeamAgentInput {
            name: "Developer".into(),
            role: "lead".into(),
            backend: None,
            model: "configured-model".into(),
            assistant_id: Some("configured-assistant".into()),
            conversation_id: None,
        }],
    }
}

async fn setup() -> (
    aionui_db::Database,
    Arc<SqliteProductFactoryRepository>,
    ProductFactoryService,
    Arc<TeamPort>,
    String,
) {
    let db = aionui_db::init_database_memory().await.unwrap();
    let repo = Arc::new(SqliteProductFactoryRepository::new(db.pool().clone()));
    let port = Arc::new(TeamPort::default());
    let service = ProductFactoryService::new(repo.clone()).with_team_port(port.clone());
    let run = service
        .create_run(
            "owner",
            CreateProductFactoryRunRequest {
                name: "Product".into(),
                idea: "Build a product".into(),
                target_user: None,
                problem: None,
                expected_output: None,
                workspace_path: Some(std::env::current_dir().unwrap().to_string_lossy().into_owned()),
                budget_usd: Some(25.0),
            },
        )
        .await
        .unwrap();
    let mut row = repo.get_run("owner", &run.id).await.unwrap().unwrap();
    let mut draft = generate_task_draft(&serde_json::json!({"sections":[{"content":"Core flow"}]}), 10);
    draft.confirmed = true;
    draft.revision = 2;
    row.status = "task_draft_ready".into();
    row.task_draft_json = Some(serde_json::to_string(&draft).unwrap());
    row.plan_revision += 1;
    repo.update_run(&row).await.unwrap();
    (db, repo, service, port, run.id)
}

#[tokio::test]
async fn handoff_preserves_dependencies_criteria_budget_and_pending_state() {
    let (db, _, service, port, id) = setup().await;
    let result = service.handoff("owner", &id, request(2)).await.unwrap();
    assert_eq!(result.run.status.as_str(), "handed_off");
    assert_eq!(result.task_id_map.len(), 4);
    assert_eq!(result.run.team_id.as_deref(), Some(result.team_id.as_str()));
    let tasks: Vec<aionui_db::models::TeamTaskRow> = sqlx::query_as("SELECT * FROM team_tasks WHERE team_id = ?")
        .bind(&result.team_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
    assert!(
        tasks
            .iter()
            .all(|task| task.status == "pending" && task.owner.is_none())
    );
    let backend = tasks
        .iter()
        .find(|task| task.id == result.task_id_map["backend-1"])
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&backend.blocked_by).unwrap(),
        vec![result.task_id_map["data-1"].clone()]
    );
    let metadata: serde_json::Value = serde_json::from_str(backend.metadata.as_ref().unwrap()).unwrap();
    assert!(
        !metadata["product_factory"]["acceptance_criteria"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let budget: f64 = sqlx::query_scalar("SELECT limit_usd FROM team_budgets WHERE team_id = ?")
        .bind(&result.team_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(budget, 25.0);
    assert_eq!(port.discarded.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn concurrent_and_lost_response_retries_return_one_team_and_graph() {
    let (db, _, service, port, id) = setup().await;
    let (first, second) = tokio::join!(
        service.handoff("owner", &id, request(2)),
        service.handoff("owner", &id, request(2))
    );
    assert_eq!(first.unwrap().team_id, second.unwrap().team_id);
    let replay = service
        .handoff(
            "owner",
            &id,
            HandoffProductFactoryRequest {
                expected_revision: 2,
                agents: vec![],
            },
        )
        .await
        .unwrap();
    assert_eq!(replay.task_id_map.len(), 4);
    assert_eq!(port.prepared.load(Ordering::SeqCst), 1);
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM teams), (SELECT COUNT(*) FROM team_tasks)")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(counts, (1, 4));
}

#[tokio::test]
async fn independent_service_instances_still_commit_only_one_team() {
    let (db, repo, first, port, id) = setup().await;
    let second = ProductFactoryService::new(repo).with_team_port(port.clone());
    let (a, b) = tokio::join!(
        first.handoff("owner", &id, request(2)),
        second.handoff("owner", &id, request(2))
    );
    assert_eq!(a.unwrap().team_id, b.unwrap().team_id);
    assert_eq!(port.prepared.load(Ordering::SeqCst), 2);
    assert_eq!(port.discarded.load(Ordering::SeqCst), 1);
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM teams), (SELECT COUNT(*) FROM team_tasks)")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(counts, (1, 4));
}

#[tokio::test]
async fn stale_revision_is_a_conflict_even_after_successful_handoff() {
    let (_, _, service, port, id) = setup().await;
    assert!(matches!(
        service.handoff("owner", &id, request(1)).await,
        Err(ProductFactoryError::RevisionConflict)
    ));
    assert_eq!(port.prepared.load(Ordering::SeqCst), 0);
    service.handoff("owner", &id, request(2)).await.unwrap();
    assert!(matches!(
        service.handoff("owner", &id, request(1)).await,
        Err(ProductFactoryError::RevisionConflict)
    ));
}

#[tokio::test]
async fn foreign_owner_cannot_prepare_or_read_back_a_team() {
    let (_, _, service, port, id) = setup().await;
    assert!(matches!(
        service.handoff("other", &id, request(2)).await,
        Err(ProductFactoryError::NotFound(_))
    ));
    service.handoff("owner", &id, request(2)).await.unwrap();
    assert!(matches!(
        service.handoff("other", &id, request(2)).await,
        Err(ProductFactoryError::NotFound(_))
    ));
    assert_eq!(port.prepared.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn incomplete_or_invalid_drafts_are_rejected_before_preparing_members() {
    for invalid in ["unconfirmed", "cycle", "source", "version", "title", "revision"] {
        let (_, repo, service, port, id) = setup().await;
        let mut row = repo.get_run("owner", &id).await.unwrap().unwrap();
        let mut draft: serde_json::Value = serde_json::from_str(row.task_draft_json.as_ref().unwrap()).unwrap();
        match invalid {
            "unconfirmed" => draft["confirmed"] = false.into(),
            "cycle" => draft["tasks"][0]["blocked_by"] = serde_json::json!(["test-1"]),
            "source" => draft["generated_by"] = "unknown".into(),
            "version" => draft["version"] = 9.into(),
            "title" => draft["tasks"][0]["title"] = " ".into(),
            _ => draft["revision"] = 0.into(),
        }
        row.task_draft_json = Some(draft.to_string());
        row.plan_revision += 1;
        repo.update_run(&row).await.unwrap();
        assert!(
            matches!(
                service.handoff("owner", &id, request(2)).await,
                Err(ProductFactoryError::InvalidHandoff(_))
            ),
            "accepted {invalid}"
        );
        assert_eq!(port.prepared.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn unavailable_workspaces_and_missing_member_configuration_do_not_create_a_team() {
    let (_, repo, service, port, id) = setup().await;
    assert!(matches!(
        service
            .handoff(
                "owner",
                &id,
                HandoffProductFactoryRequest {
                    expected_revision: 2,
                    agents: vec![]
                }
            )
            .await,
        Err(ProductFactoryError::InvalidHandoff(_))
    ));
    let mut row = repo.get_run("owner", &id).await.unwrap().unwrap();
    row.workspace_path = format!("/unavailable-product-factory-{id}");
    row.plan_revision += 1;
    repo.update_run(&row).await.unwrap();
    assert!(matches!(
        service.handoff("owner", &id, request(2)).await,
        Err(ProductFactoryError::InvalidHandoff(_))
    ));
    assert_eq!(port.prepared.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn preparation_failure_keeps_run_ready_and_can_be_retried() {
    let (_, repo, service, port, id) = setup().await;
    port.fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        service.handoff("owner", &id, request(2)).await,
        Err(ProductFactoryError::TeamPreparation(_))
    ));
    assert!(repo.get_run("owner", &id).await.unwrap().unwrap().team_id.is_none());
    port.fail.store(false, Ordering::SeqCst);
    assert!(service.handoff("owner", &id, request(2)).await.is_ok());
}

#[tokio::test]
async fn persistence_failure_discards_only_the_prepared_members_and_does_not_claim_success() {
    let (_, repo, service, port, id) = setup().await;
    port.invalid_team.store(true, Ordering::SeqCst);
    assert!(matches!(
        service.handoff("owner", &id, request(2)).await,
        Err(ProductFactoryError::Database(_))
    ));
    assert_eq!(port.discarded.load(Ordering::SeqCst), 1);
    let row = repo.get_run("owner", &id).await.unwrap().unwrap();
    assert_eq!(row.status, "task_draft_ready");
    assert!(row.team_id.is_none());
    port.invalid_team.store(false, Ordering::SeqCst);
    assert!(service.handoff("owner", &id, request(2)).await.is_ok());
}

#[tokio::test]
async fn deleted_team_is_not_silently_recreated_on_retry() {
    let (db, _, service, port, id) = setup().await;
    let result = service.handoff("owner", &id, request(2)).await.unwrap();
    sqlx::query("DELETE FROM teams WHERE id = ?")
        .bind(&result.team_id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        service.handoff("owner", &id, request(2)).await,
        Err(ProductFactoryError::InvalidHandoff(_))
    ));
    assert_eq!(port.prepared.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn http_handoff_returns_committed_result_and_stable_revision_conflict() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware::{self, Next},
        response::Response,
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let (_, _, service, _, id) = setup().await;
    let app = aionui_product_factory::product_factory_routes(aionui_product_factory::ProductFactoryRouterState {
        service: Arc::new(service),
    })
    .layer(middleware::from_fn(
        |mut request: Request<Body>, next: Next| async move {
            let mut user = aionui_auth::CurrentUser::local_default();
            user.id = "owner".into();
            request.extensions_mut().insert(user);
            Ok::<Response, std::convert::Infallible>(next.run(request).await)
        },
    ));
    let uri = format!("/api/product-factory/runs/{id}/handoff");
    let body = r#"{"expected_revision":2,"agents":[{"name":"Developer","role":"lead","assistant_id":"configured-assistant","model":"configured-model"}]}"#;
    let response = app
        .clone()
        .oneshot(
            Request::post(&uri)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["data"]["run"]["status"], "handed_off");
    let response = app
        .oneshot(
            Request::post(&uri)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"expected_revision":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["code"], "PRODUCT_FACTORY_REVISION_CONFLICT");
}
