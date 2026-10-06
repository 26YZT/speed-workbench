#![cfg(any(target_os = "macos", target_os = "linux"))]
mod common;

use aionui_app::{AppConfig, AppServices};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::{body_json, get_with_token, json_with_token, setup_and_login};
use serde_json::{Value, json};
use tower::ServiceExt;

struct Fixture {
    root: tempfile::TempDir,
    app: axum::Router,
    services: AppServices,
    token: String,
    csrf: String,
    id: String,
}
impl Fixture {
    async fn new(completed: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let db = aionui_db::init_database_memory().await.unwrap();
        let config = AppConfig {
            data_dir: root.path().to_path_buf(),
            work_dir: root.path().to_path_buf(),
            ..Default::default()
        };
        let services = AppServices::from_config(db, &config).await.unwrap();
        let mut app = aionui_app::create_router(&services).await.unwrap();
        let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
        let workspace = root.path().join("source");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::write(workspace.join("START.md"), "Local test fixture launch guide").unwrap();
        std::fs::write(
            workspace.join("ACCEPTANCE.md"),
            "Internal fixture only, not customer acceptance",
        )
        .unwrap();
        std::fs::write(workspace.join("app.py"), "print('source fixture')").unwrap();
        let response = app
            .clone()
            .oneshot(json_with_token(
                "POST",
                "/api/product-factory/runs",
                json!({"name":"Version security fixture","idea":"Local code snapshot","workspace_path":workspace}),
                &token,
                &csrf,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let id = body_json(response).await["data"]["id"].as_str().unwrap().to_owned();
        if completed {
            let owner = services.user_repo.find_by_username("admin").await.unwrap().unwrap().id;
            let team = format!("team-{id}");
            sqlx::query("INSERT INTO teams (id,user_id,name,workspace,created_at,updated_at) VALUES (?,?, 'fixture team',?,1,1)").bind(&team).bind(&owner).bind(workspace.to_string_lossy().as_ref()).execute(services.database.pool()).await.unwrap();
            let task = format!("{team}-final");
            sqlx::query("INSERT INTO team_tasks (id,team_id,subject,status,created_at,updated_at) VALUES (?,?,'completed fixture','completed',1,1)").bind(&task).bind(&team).execute(services.database.pool()).await.unwrap();
            let draft = json!({"version":1,"generated_by":"draft","confirmed":true,"revision":1,"updated_at":1,"tasks":[{"id":"final","title":"Code fixture","description":"Internal fixture","type":"test","blocked_by":[],"acceptance_criteria":["Internal verification"],"suggested_role":"developer","effort":"low"}]});
            sqlx::query("UPDATE product_factory_runs SET status='completed',team_id=?,task_draft_json=? WHERE id=? AND user_id=?").bind(&team).bind(draft.to_string()).bind(&id).bind(&owner).execute(services.database.pool()).await.unwrap();
            sqlx::query("INSERT INTO product_factory_execution (run_id,user_id,team_id,task_id,state,message_id,team_run_id,requested_at,updated_at) VALUES (?,?,?,?,'enqueued','fixture-msg','fixture-run',1,1)").bind(&id).bind(&owner).bind(&team).bind(&task).execute(services.database.pool()).await.unwrap();
            sqlx::query("INSERT INTO agent_usage (id,user_id,team_id,task_id,model,input_tokens,output_tokens,cached_read_tokens,cached_write_tokens,cost_est,cost_unknown_reason,conversation_id,turn_id,attempt_id,created_at) VALUES ('old-fee',?,?,?,'fixture-model',100,20,0,0,NULL,'model_price_missing','fixture-conv','fixture-turn','fixture-attempt',1)")
                .bind(&owner).bind(&team).bind(&task).execute(services.database.pool()).await.unwrap();
        }
        Self {
            root,
            app,
            services,
            token,
            csrf,
            id,
        }
    }
    fn path(&self, suffix: &str) -> String {
        format!("/api/product-factory/runs/{}/versions{suffix}", self.id)
    }
    async fn snapshot_body(&self, key: &str) -> Value {
        let response = self
            .app
            .clone()
            .oneshot(get_with_token(&self.path("/source-manifest"), &self.token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value = body_json(response).await;
        let files = value["data"]["manifest"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| json!({"path":file["path"],"sha256":file["sha256"]}))
            .collect::<Vec<_>>();
        json!({"expected_plan_revision":value["data"]["plan_revision"],"idempotency_key":key,"files":files})
    }
    async fn seal(&self) -> Value {
        let response = self
            .app
            .clone()
            .oneshot(json_with_token(
                "POST",
                &self.path("/snapshot"),
                self.snapshot_body("seal").await,
                &self.token,
                &self.csrf,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        body_json(response).await["data"].clone()
    }
}

#[tokio::test]
async fn version_routes_require_authentication_csrf_and_owned_run() {
    let mut fixture = Fixture::new(true).await;
    for suffix in ["", "/source-manifest"] {
        let response = fixture
            .app
            .clone()
            .oneshot(Request::get(fixture.path(suffix)).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let snapshot = fixture.snapshot_body("security").await;
    for (suffix, body) in [
        ("/snapshot", snapshot.clone()),
        (
            "/iterate",
            json!({"source_version_id":"fixture","expected_product_revision":1,"idempotency_key":"fixture","change_request":"Change"}),
        ),
        (
            "/activate",
            json!({"version_id":"fixture","expected_product_revision":1}),
        ),
    ] {
        let response = fixture
            .app
            .clone()
            .oneshot(
                Request::post(fixture.path(suffix))
                    .header("content-type", "application/json")
                    .header("x-csrf-token", &fixture.csrf)
                    .header("cookie", format!("aionui-csrf-token={}", fixture.csrf))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = fixture
            .app
            .clone()
            .oneshot(
                Request::post(fixture.path(suffix))
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", fixture.token))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let (other, other_csrf) = setup_and_login(&mut fixture.app, &fixture.services, "other", "StrongP@ss2").await;
    for suffix in ["", "/source-manifest"] {
        let response = fixture
            .app
            .clone()
            .oneshot(get_with_token(&fixture.path(suffix), &other))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    for (suffix, body) in [
        ("/snapshot", snapshot),
        (
            "/iterate",
            json!({"source_version_id":"fixture","expected_product_revision":1,"idempotency_key":"fixture","change_request":"Change"}),
        ),
        (
            "/activate",
            json!({"version_id":"fixture","expected_product_revision":1}),
        ),
    ] {
        let response = fixture
            .app
            .clone()
            .oneshot(json_with_token(
                "POST",
                &fixture.path(suffix),
                body,
                &other,
                &other_csrf,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_versions")
        .fetch_one(fixture.services.database.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(fixture.services.worker_task_manager.active_count(), 0);
    fixture.services.database.close().await;
}

#[tokio::test]
async fn sealed_code_is_read_only_but_retained_costs_and_cancel_routes_remain_readable() {
    let fixture = Fixture::new(true).await;
    let sealed = fixture.seal().await;
    assert_eq!(sealed["version"]["state"], "sealed");
    let base = format!("/api/product-factory/runs/{}", fixture.id);
    for (path, body) in [
        (
            format!("{base}/interview"),
            json!({"interview":{"summary":"new","questions":[]}}),
        ),
        (
            format!("{base}/planning"),
            json!({"phase":"interview","expected_plan_revision":1,"idempotency_key":"frozen","assistant_id":"fixture","model":""}),
        ),
        (format!("{base}/handoff"), json!({"expected_revision":1,"agents":[]})),
        (format!("{base}/start"), json!({"expected_revision":1})),
    ] {
        let response = fixture
            .app
            .clone()
            .oneshot(json_with_token(
                if path.ends_with("/interview") { "PUT" } else { "POST" },
                &path,
                body,
                &fixture.token,
                &fixture.csrf,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT, "frozen writes: {path}");
    }
    let report = fixture
        .app
        .clone()
        .oneshot(get_with_token(&format!("{base}/cost-report"), &fixture.token))
        .await
        .unwrap();
    assert_eq!(report.status(), StatusCode::OK);
    let report = body_json(report).await;
    assert_eq!(report["data"]["usage"][0]["input_tokens"], 100);
    assert!(report["data"]["usage_summary"]["cost_est"].is_null());
    assert_eq!(report["data"]["usage_summary"]["cost_unknown"], true);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_usage")
        .fetch_one(fixture.services.database.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let owner = fixture
        .services
        .user_repo
        .find_by_username("admin")
        .await
        .unwrap()
        .unwrap()
        .id;
    sqlx::query("INSERT INTO product_factory_planning (id,user_id,run_id,phase,input_plan_revision,input_hash,idempotency_key,assistant_id,model,state,created_at,updated_at) VALUES ('cancel-fixture',?,?,'interview',1,'fixture','cancel-fixture','fixture','','reserved',1,1)").bind(owner).bind(&fixture.id).execute(fixture.services.database.pool()).await.unwrap();
    let cancelled = fixture
        .app
        .clone()
        .oneshot(json_with_token(
            "POST",
            &format!("{base}/planning/cancel-fixture/cancel"),
            json!({}),
            &fixture.token,
            &fixture.csrf,
        ))
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    assert_eq!(body_json(cancelled).await["data"]["state"], "cancelled");
    assert_eq!(fixture.services.worker_task_manager.active_count(), 0);
    fixture.services.database.close().await;
}

#[tokio::test]
async fn copying_run_rejects_planning_handoff_and_execution_without_a_model_send() {
    let fixture = Fixture::new(false).await;
    let owner = fixture
        .services
        .user_repo
        .find_by_username("admin")
        .await
        .unwrap()
        .unwrap()
        .id;
    sqlx::query("INSERT INTO product_factory_products (id,user_id,name,revision,created_at,updated_at) VALUES ('copy-product',?,'copy fixture',1,1,1)").bind(&owner).execute(fixture.services.database.pool()).await.unwrap();
    sqlx::query("INSERT INTO product_factory_versions (id,user_id,product_id,run_id,version_no,state,created_at,updated_at) VALUES ('copy-version',?,'copy-product',?,1,'copying',1,1)").bind(&owner).bind(&fixture.id).execute(fixture.services.database.pool()).await.unwrap();
    let base = format!("/api/product-factory/runs/{}", fixture.id);
    for (path, body) in [
        (
            format!("{base}/interview"),
            json!({"interview":{"summary":"new","questions":[]}}),
        ),
        (
            format!("{base}/planning"),
            json!({"phase":"interview","expected_plan_revision":1,"idempotency_key":"copying","assistant_id":"fixture","model":""}),
        ),
        (format!("{base}/handoff"), json!({"expected_revision":1,"agents":[]})),
        (format!("{base}/start"), json!({"expected_revision":1})),
    ] {
        let response = fixture
            .app
            .clone()
            .oneshot(json_with_token(
                if path.ends_with("/interview") { "PUT" } else { "POST" },
                &path,
                body,
                &fixture.token,
                &fixture.csrf,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT, "pending-copy writes: {path}");
    }
    let list = fixture
        .app
        .clone()
        .oneshot(get_with_token(&fixture.path(""), &fixture.token))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    assert_eq!(body_json(list).await["data"]["versions"][0]["state"], "copying");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_planning")
        .fetch_one(fixture.services.database.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(fixture.services.worker_task_manager.active_count(), 0);
    assert!(!fixture.root.path().join("source").join(".tasks").exists());
    fixture.services.database.close().await;
}

#[tokio::test]
async fn snapshot_busy_conversation_claim_and_missing_activity_port_fail_before_copy() {
    use std::sync::Arc;
    let fixture = Fixture::new(true).await;
    let owner = fixture
        .services
        .user_repo
        .find_by_username("admin")
        .await
        .unwrap()
        .unwrap()
        .id;
    sqlx::query("INSERT INTO conversations (id,user_id,name,type,status,extra,created_at,updated_at) VALUES ('busy-fixture-convo',?,'busy fixture','acp','pending','{}',1,1)").bind(&owner).execute(fixture.services.database.pool()).await.unwrap();
    let members = json!([{"slot_id":"lead","name":"Fixture","role":"lead","conversation_id":"busy-fixture-convo","backend":"fixture","model":"fixture","status":"idle"}]);
    sqlx::query("UPDATE teams SET agents=? WHERE id=? AND user_id=?")
        .bind(members.to_string())
        .bind(format!("team-{}", fixture.id))
        .bind(&owner)
        .execute(fixture.services.database.pool())
        .await
        .unwrap();
    let claim = fixture
        .services
        .conversation_runtime_state
        .try_claim_turn("busy-fixture-convo", "fixture-active-turn")
        .unwrap();
    let response = fixture
        .app
        .clone()
        .oneshot(json_with_token(
            "POST",
            &fixture.path("/snapshot"),
            fixture.snapshot_body("busy-proof").await,
            &fixture.token,
            &fixture.csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        body_json(response).await["code"],
        "PRODUCT_FACTORY_VERSION_RUNTIME_BUSY"
    );
    assert!(!fixture.root.path().join("product-factory-versions").exists());
    drop(claim);
    let (mut states, _) = aionui_app::build_module_states(&fixture.services).await.unwrap();
    states.product_factory.service = Arc::new(states.product_factory.service.as_ref().clone().with_versioning(
        Arc::new(aionui_db::SqliteProductFactoryVersionsRepository::new(
            fixture.services.database.pool().clone(),
        )),
        fixture.root.path().join("product-factory-versions"),
    ));
    let no_port = aionui_app::create_router_with_states(&fixture.services, states);
    let response = no_port
        .oneshot(json_with_token(
            "POST",
            &fixture.path("/snapshot"),
            fixture.snapshot_body("missing-port-proof").await,
            &fixture.token,
            &fixture.csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        body_json(response).await["code"],
        "PRODUCT_FACTORY_VERSION_RUNTIME_UNAVAILABLE"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_versions")
        .fetch_one(fixture.services.database.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(fixture.services.worker_task_manager.active_count(), 0);
    fixture.services.database.close().await;
}
