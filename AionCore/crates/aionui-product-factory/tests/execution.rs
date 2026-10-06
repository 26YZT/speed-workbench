use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use aionui_api_types::{
    CreateProductFactoryRunRequest, CreateTeamRequest, HandoffProductFactoryRequest, StartProductFactoryRequest,
    TeamAgentInput, TeamTaskResponse, TeamTaskReviewDecision, TeamTaskReviewRequest, TeamTaskUsageResponse,
    TeamTaskWorkspaceResponse, TeamUsageSummaryResponse,
};
use aionui_db::{IProductFactoryRepository, SqliteProductFactoryRepository, models::TeamRow};
use aionui_product_factory::{
    ProductFactoryDispatchReceipt, ProductFactoryError, ProductFactoryExecutionPort, ProductFactoryReviewPort,
    ProductFactoryService, ProductFactoryTaskReviewSnapshot, ProductFactoryTeamPort,
};

struct TeamPort;
#[async_trait::async_trait]
impl ProductFactoryTeamPort for TeamPort {
    async fn prepare_team(
        &self,
        user: &str,
        id: &str,
        request: CreateTeamRequest,
    ) -> Result<TeamRow, ProductFactoryError> {
        Ok(TeamRow {
            id: id.into(),
            user_id: user.into(),
            name: request.name,
            workspace: request.workspace.unwrap(),
            workspace_mode: "shared".into(),
            agents: serde_json::json!([{"slot_id":"lead-slot","name":"Developer","role":"lead",
                "conversation_id":"lead-conversation","backend":"fixture","model":"configured"}])
            .to_string(),
            lead_agent_id: Some("lead-slot".into()),
            session_mode: None,
            agents_version: "1.0.1".into(),
            created_at: 20,
            updated_at: 20,
            project_id: None,
            folder_id: None,
        })
    }
    async fn discard_team(&self, _: &str, _: &TeamRow) -> Result<(), ProductFactoryError> {
        Ok(())
    }
}

struct ExecutionPort {
    _workspace: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    dispatched_task_id: std::sync::Mutex<Option<String>>,
    fail_preflight: AtomicBool,
    fail_after_write: AtomicBool,
}

struct ReviewPort {
    pool: sqlx::SqlitePool,
}

#[async_trait::async_trait]
impl ProductFactoryReviewPort for ReviewPort {
    async fn get_task_snapshot(
        &self,
        _user: &str,
        _team: &str,
        task_id: &str,
    ) -> Result<ProductFactoryTaskReviewSnapshot, ProductFactoryError> {
        let row = sqlx::query_as::<_, aionui_db::models::TeamTaskRow>("SELECT * FROM team_tasks WHERE id = ?")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| ProductFactoryError::ReviewFailed(error.to_string()))?;
        let blocked_by = serde_json::from_str(&row.blocked_by).unwrap();
        let blocks = serde_json::from_str(&row.blocks).unwrap();
        Ok(ProductFactoryTaskReviewSnapshot {
            task: TeamTaskResponse {
                id: row.id,
                team_id: row.team_id,
                subject: row.subject,
                description: row.description,
                status: row.status,
                owner: row.owner,
                blocked_by,
                blocks,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            workspace: TeamTaskWorkspaceResponse {
                task_id: task_id.to_owned(),
                path: "/tmp/product-factory-task".into(),
            },
            usage: vec![TeamTaskUsageResponse {
                id: format!("usage-{task_id}"),
                task_id: Some(task_id.to_owned()),
                agent_id: None,
                model: Some("fixture-model".into()),
                input_tokens: 100,
                output_tokens: 20,
                cost_est: Some(0.01),
                cached_read_tokens: Some(80),
                cached_write_tokens: Some(0),
                cost_source: Some("configured_estimate".into()),
                pricing_snapshot: Some("{}".into()),
                cost_unknown_reason: None,
                conversation_id: "c".into(),
                turn_id: task_id.to_owned(),
                attempt_id: "fixture".into(),
                created_at: 1,
            }],
            usage_summary: TeamUsageSummaryResponse {
                unassigned_usage: Vec::new(),
                team_id: "team".into(),
                input_tokens: 100,
                output_tokens: 20,
                cost_est: Some(0.01),
                cost_unknown: false,
                budget_limit_usd: None,
                budget_remaining_usd: None,
                budget_exceeded: false,
                tasks: vec![aionui_api_types::TeamTaskUsageSummaryResponse {
                    task_id: "team-data-1".into(),
                    input_tokens: 100,
                    output_tokens: 20,
                    cost_est: Some(0.01),
                    turn_count: 1,
                }],
            },
        })
    }

    async fn review_task(
        &self,
        _user: &str,
        _team: &str,
        task_id: &str,
        request: TeamTaskReviewRequest,
    ) -> Result<TeamTaskResponse, ProductFactoryError> {
        let status = match request.decision {
            TeamTaskReviewDecision::Approve => "completed",
            TeamTaskReviewDecision::RequestChanges => "in_progress",
        };
        sqlx::query("UPDATE team_tasks SET status = ? WHERE id = ?")
            .bind(status)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(|error| ProductFactoryError::ReviewFailed(error.to_string()))?;
        self.get_task_snapshot("owner", "team", task_id)
            .await
            .map(|snapshot| snapshot.task)
    }
}
#[async_trait::async_trait]
impl ProductFactoryExecutionPort for ExecutionPort {
    async fn check_start(&self, _: &str, _: &str) -> Result<(), ProductFactoryError> {
        if self.fail_preflight.load(Ordering::SeqCst) {
            return Err(ProductFactoryError::ExecutionBlocked(
                "budget cannot be verified".into(),
            ));
        }
        Ok(())
    }
    async fn enqueue(
        &self,
        user: &str,
        team: &str,
        task_id: &str,
        content: &str,
    ) -> Result<ProductFactoryDispatchReceipt, ProductFactoryError> {
        *self.dispatched_task_id.lock().unwrap() = Some(task_id.to_owned());
        // The real durable side effect is retained; only external Agent execution is replaced.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mailbox")
            .fetch_one(&self.pool)
            .await
            .unwrap();
        let message_id = format!("mail-{}", count + 1);
        sqlx::query(
            "INSERT INTO mailbox (id, team_id, to_agent_id, from_agent_id, type, content, read, created_at) \
            SELECT ?, ?, 'lead-slot', 'user', 'message', ?, 0, 30 \
            WHERE EXISTS (SELECT 1 FROM teams WHERE id = ? AND user_id = ?)",
        )
        .bind(&message_id)
        .bind(team)
        .bind(content)
        .bind(team)
        .bind(user)
        .execute(&self.pool)
        .await
        .unwrap();
        if self.fail_after_write.load(Ordering::SeqCst) {
            return Err(ProductFactoryError::ExecutionDispatchFailed);
        }
        Ok(ProductFactoryDispatchReceipt {
            message_id,
            team_run_id: format!("turn-{}", count + 1),
        })
    }

    async fn resume(
        &self,
        user: &str,
        team: &str,
        task_id: &str,
    ) -> Result<ProductFactoryDispatchReceipt, ProductFactoryError> {
        self.enqueue(user, team, task_id, "resume").await
    }
}

async fn setup() -> (
    aionui_db::Database,
    Arc<SqliteProductFactoryRepository>,
    ProductFactoryService,
    Arc<ExecutionPort>,
    String,
) {
    let db = aionui_db::init_database_memory().await.unwrap();
    let repo = Arc::new(SqliteProductFactoryRepository::new(db.pool().clone()));
    let workspace = tempfile::tempdir().unwrap();
    let workspace_path = workspace.path().to_string_lossy().into_owned();
    let execution = Arc::new(ExecutionPort {
        _workspace: workspace,
        pool: db.pool().clone(),
        dispatched_task_id: std::sync::Mutex::new(None),
        fail_preflight: AtomicBool::new(false),
        fail_after_write: AtomicBool::new(false),
    });
    let service = ProductFactoryService::new(repo.clone())
        .with_team_port(Arc::new(TeamPort))
        .with_execution_port(execution.clone());
    let run = service
        .create_run(
            "owner",
            CreateProductFactoryRunRequest {
                name: "Title tool".into(),
                idea: "Generate titles from a product word".into(),
                target_user: Some("Nontechnical users".into()),
                problem: None,
                expected_output: Some("A locally runnable product".into()),
                workspace_path: Some(workspace_path),
                budget_usd: Some(10.0),
            },
        )
        .await
        .unwrap();
    service
        .save_interview(
            "owner",
            &run.id,
            serde_json::json!({"version":1,"summary":"Persist title history"}),
        )
        .await
        .unwrap();
    service.confirm_interview("owner", &run.id).await.unwrap();
    service.confirm_blueprint("owner", &run.id).await.unwrap();
    service.generate_task_draft("owner", &run.id).await.unwrap();
    service
        .confirm_task_draft(
            "owner",
            &run.id,
            aionui_api_types::ConfirmTaskDraftRequest { expected_revision: 1 },
        )
        .await
        .unwrap();
    service
        .handoff(
            "owner",
            &run.id,
            HandoffProductFactoryRequest {
                expected_revision: 1,
                agents: vec![TeamAgentInput {
                    name: "Developer".into(),
                    role: "lead".into(),
                    backend: None,
                    model: "configured".into(),
                    assistant_id: Some("assistant".into()),
                    conversation_id: None,
                }],
            },
        )
        .await
        .unwrap();
    (db, repo, service, execution, run.id)
}

#[tokio::test]
async fn review_snapshot_exposes_the_current_task_and_approval_completes_the_run() {
    let (db, repo, service, execution, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    let task_id = format!("{team_id}-data-1");
    sqlx::query("UPDATE team_tasks SET status = 'in_review' WHERE id = ?")
        .bind(&task_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'completed' WHERE team_id = ? AND id != ?")
        .bind(&team_id)
        .bind(&task_id)
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));
    let snapshot = service.get_review("owner", &id).await.unwrap();
    assert_eq!(snapshot.task.status, "in_review");
    assert_eq!(snapshot.usage_summary.team_id, "team");
    assert_eq!(snapshot.workspace.path, "/tmp/product-factory-task");
    let reviewed = service
        .review(
            "owner",
            &id,
            TeamTaskReviewRequest {
                decision: TeamTaskReviewDecision::Approve,
                feedback: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(reviewed.task.status, "completed");
    assert_eq!(reviewed.run.status.as_str(), "completed");
    assert!(execution.dispatched_task_id.lock().unwrap().is_some());
}

#[tokio::test]
async fn approval_keeps_run_running_when_another_task_is_still_blocked() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    let first_task_id = format!("{team_id}-data-1");
    sqlx::query("UPDATE team_tasks SET status = 'in_review' WHERE id = ?")
        .bind(&first_task_id)
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));

    let reviewed = service
        .review(
            "owner",
            &id,
            TeamTaskReviewRequest {
                decision: TeamTaskReviewDecision::Approve,
                feedback: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(reviewed.run.status.as_str(), "running");
    assert!(reviewed.next_task.is_none());
}

#[tokio::test]
async fn completed_run_exposes_delivery_checks_and_task_counts() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'completed' WHERE team_id = ?")
        .bind(&team_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE product_factory_runs SET status = 'completed' WHERE id = ?")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();
    let workspace = repo.get_run("owner", &id).await.unwrap().unwrap().workspace_path;
    std::fs::write(std::path::Path::new(&workspace).join("START.md"), "# Start\n").unwrap();
    std::fs::write(std::path::Path::new(&workspace).join("ACCEPTANCE.md"), "# Acceptance\n").unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));

    let delivery = service.get_delivery("owner", &id).await.unwrap();

    assert!(delivery.ready);
    assert_eq!(delivery.task_counts.total, 4);
    assert_eq!(delivery.task_counts.completed, 4);
    assert!(delivery.checks.iter().all(|check| check.passed));
}

#[tokio::test]
async fn completed_run_exposes_a_cost_report_with_task_breakdown() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'completed' WHERE team_id = ?")
        .bind(&team_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE product_factory_runs SET status = 'completed' WHERE id = ?")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));

    let report = service.get_cost_report("owner", &id).await.unwrap();

    assert_eq!(report.usage_summary.input_tokens, 100);
    assert_eq!(report.usage_summary.output_tokens, 20);
    assert_eq!(report.usage_summary.cost_est, Some(0.01));
    assert_eq!(report.task_counts.total, 4);
    assert!(report.generated_at > 0);
    assert_eq!(
        report.usage.len(),
        4,
        "export must include every task, not only the current task"
    );
    assert!(
        report
            .usage
            .iter()
            .all(|row| row.cached_read_tokens == Some(80) && row.cost_source.as_deref() == Some("configured_estimate"))
    );
    assert!(
        report
            .usage
            .iter()
            .any(|row| row.task_id.as_deref() == Some(&format!("{team_id}-data-1")))
    );
}

#[tokio::test]
async fn sequential_approval_completes_all_tasks_and_unlocks_delivery() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    sqlx::query("UPDATE team_tasks SET blocked_by = '[]' WHERE team_id = ?")
        .bind(&team_id)
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));

    for index in 0..4 {
        let execution = repo.get_execution("owner", &id).await.unwrap().unwrap();
        sqlx::query("UPDATE team_tasks SET status = 'in_review' WHERE id = ?")
            .bind(&execution.task_id)
            .execute(db.pool())
            .await
            .unwrap();
        let reviewed = service
            .review(
                "owner",
                &id,
                TeamTaskReviewRequest {
                    decision: TeamTaskReviewDecision::Approve,
                    feedback: None,
                },
            )
            .await
            .unwrap();
        if index < 3 {
            assert!(reviewed.next_task.is_some());
            service.continue_run("owner", &id).await.unwrap();
        } else {
            assert_eq!(reviewed.run.status.as_str(), "completed");
            assert!(reviewed.next_task.is_none());
        }
    }

    let workspace = repo.get_run("owner", &id).await.unwrap().unwrap().workspace_path;
    std::fs::write(std::path::Path::new(&workspace).join("START.md"), "# Start\n").unwrap();
    std::fs::write(std::path::Path::new(&workspace).join("ACCEPTANCE.md"), "# Acceptance\n").unwrap();
    let delivery = service.get_delivery("owner", &id).await.unwrap();
    assert!(delivery.ready);
    assert_eq!(delivery.task_counts.completed, 4);
}

#[tokio::test]
async fn delivery_is_not_ready_when_tasks_remain_pending() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    sqlx::query("UPDATE product_factory_runs SET status = 'completed' WHERE id = ?")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));

    let delivery = service.get_delivery("owner", &id).await.unwrap();

    assert!(!delivery.ready);
    assert!(delivery.task_counts.completed < delivery.task_counts.total);
    assert!(
        delivery
            .checks
            .iter()
            .any(|check| check.code == "all_tasks_completed" && !check.passed)
    );
    assert!(repo.get_run("owner", &id).await.unwrap().is_some());
}

#[tokio::test]
async fn approval_unlocks_the_next_task_but_continue_is_the_only_new_dispatch() {
    let (db, repo, service, execution, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    let first_task_id = format!("{team_id}-data-1");
    let next_task_id = format!("{team_id}-backend-1");
    sqlx::query("UPDATE team_tasks SET blocked_by = '[]' WHERE id = ?")
        .bind(&next_task_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'in_review' WHERE id = ?")
        .bind(&first_task_id)
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));
    let reviewed = service
        .review(
            "owner",
            &id,
            TeamTaskReviewRequest {
                decision: TeamTaskReviewDecision::Approve,
                feedback: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(reviewed.run.status.as_str(), "running");
    assert_eq!(
        reviewed.next_task.as_ref().map(|task| task.id.as_str()),
        Some("backend-1")
    );
    assert_eq!(
        execution.dispatched_task_id.lock().unwrap().as_deref(),
        Some(first_task_id.as_str())
    );

    let receipt = service.continue_run("owner", &id).await.unwrap().execution.unwrap();
    assert_eq!(receipt.task_id, next_task_id);
    assert_eq!(receipt.state.as_str(), "enqueued");
    assert_eq!(
        execution.dispatched_task_id.lock().unwrap().as_deref(),
        Some(next_task_id.as_str())
    );
}

#[tokio::test]
async fn continue_does_not_complete_run_when_unfinished_tasks_are_not_ready() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    let first_task_id = format!("{team_id}-data-1");
    let next_task_id = format!("{team_id}-backend-1");

    sqlx::query("UPDATE team_tasks SET status = 'completed' WHERE id = ?")
        .bind(&first_task_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE product_factory_runs SET status = 'running' WHERE id = ?")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();

    let result = service.continue_run("owner", &id).await;

    assert!(matches!(result, Err(ProductFactoryError::ReviewBlocked(message)) if message.contains("not ready")));
    assert_eq!(repo.get_run("owner", &id).await.unwrap().unwrap().status, "running");
    assert_eq!(
        repo.get_handoff_task("owner", &team_id, &next_task_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "pending"
    );
}

#[tokio::test]
async fn explicit_retry_reserves_same_task_without_changing_execution_identity() {
    let (db, repo, service, execution, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
    let task_id = format!("{team_id}-data-1");
    sqlx::query("UPDATE team_tasks SET status = 'in_progress', description = description || '\\n\\n--- Human review feedback ---\\nfix test' WHERE id = ?")
        .bind(&task_id)
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));
    service.continue_run("owner", &id).await.unwrap();
    assert_eq!(
        execution.dispatched_task_id.lock().unwrap().as_deref(),
        Some(task_id.as_str())
    );
    let receipt = repo.get_execution("owner", &id).await.unwrap().unwrap();
    assert_eq!(receipt.task_id, task_id);
}
fn request() -> StartProductFactoryRequest {
    StartProductFactoryRequest { expected_revision: 1 }
}

#[tokio::test]
async fn interrupted_task_can_be_explicitly_resumed_without_review_feedback() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let previous = repo.get_execution("owner", &id).await.unwrap().unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'in_progress' WHERE id = ?")
        .bind(&previous.task_id)
        .execute(db.pool())
        .await
        .unwrap();
    service.continue_run("owner", &id).await.unwrap();
    let resumed = repo.get_execution("owner", &id).await.unwrap().unwrap();
    assert_eq!(resumed.task_id, previous.task_id);
    assert_ne!(resumed.message_id, previous.message_id);
    assert_eq!(message_count(&db).await, 2);
}

#[tokio::test]
async fn uncertain_interrupted_task_is_not_resent() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let previous = repo.get_execution("owner", &id).await.unwrap().unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'in_progress' WHERE id = ?")
        .bind(&previous.task_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE product_factory_execution SET state = 'uncertain' WHERE run_id = ?")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        matches!(service.continue_run("owner", &id).await, Err(ProductFactoryError::ExecutionBlocked(message)) if message.contains("uncertain"))
    );
    assert_eq!(message_count(&db).await, 1);
}
async fn message_count(db: &aionui_db::Database) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM mailbox")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn explicit_start_delivers_only_the_first_ready_task_without_claiming_agent_execution() {
    let (db, _, service, execution_port, id) = setup().await;
    assert!(service.get_execution("owner", &id).await.unwrap().execution.is_none());
    assert_eq!(message_count(&db).await, 0);
    let execution = service
        .start_run("owner", &id, request())
        .await
        .unwrap()
        .execution
        .unwrap();
    assert_eq!(execution.state.as_str(), "enqueued");
    assert!(execution.task_id.ends_with("-data-1"));
    assert_eq!(
        execution_port.dispatched_task_id.lock().unwrap().as_deref(),
        Some(execution.task_id.as_str())
    );
    let content: String = sqlx::query_scalar("SELECT content FROM mailbox WHERE id = 'mail-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(content.contains("Generate titles from a product word"));
    assert!(content.contains("Persist title history"));
    assert!(content.contains("数据结构可持久化并可被后续接口读取"));
    assert_eq!(
        service.get_run("owner", &id).await.unwrap().status.as_str(),
        "handed_off"
    );
}

#[tokio::test]
async fn concurrent_instances_and_lost_response_retries_never_dispatch_again() {
    let (db, repo, service, execution, id) = setup().await;
    let other = ProductFactoryService::new(repo).with_execution_port(execution);
    let (a, b) = tokio::join!(
        service.start_run("owner", &id, request()),
        other.start_run("owner", &id, request())
    );
    assert!(a.is_ok());
    assert!(b.is_ok());
    let receipt = service
        .start_run("owner", &id, request())
        .await
        .unwrap()
        .execution
        .unwrap();
    assert_eq!(receipt.message_id.as_deref(), Some("mail-1"));
    assert_eq!(message_count(&db).await, 1);
}

#[tokio::test]
async fn dispatch_error_after_a_durable_write_is_uncertain_and_is_not_retried() {
    let (db, _, service, execution, id) = setup().await;
    execution.fail_after_write.store(true, Ordering::SeqCst);
    let result = service
        .start_run("owner", &id, request())
        .await
        .unwrap()
        .execution
        .unwrap();
    assert_eq!(result.state.as_str(), "uncertain");
    execution.fail_after_write.store(false, Ordering::SeqCst);
    assert_eq!(
        service
            .start_run("owner", &id, request())
            .await
            .unwrap()
            .execution
            .unwrap()
            .state
            .as_str(),
        "uncertain"
    );
    assert_eq!(message_count(&db).await, 1);
}

#[tokio::test]
async fn failed_budget_preflight_reserves_nothing_and_can_be_safely_retried() {
    let (db, _, service, execution, id) = setup().await;
    execution.fail_preflight.store(true, Ordering::SeqCst);
    assert!(matches!(
        service.start_run("owner", &id, request()).await,
        Err(ProductFactoryError::ExecutionBlocked(_))
    ));
    assert!(service.get_execution("owner", &id).await.unwrap().execution.is_none());
    assert_eq!(message_count(&db).await, 0);
    execution.fail_preflight.store(false, Ordering::SeqCst);
    service.start_run("owner", &id, request()).await.unwrap();
    assert_eq!(message_count(&db).await, 1);
}

#[tokio::test]
async fn foreign_owners_and_stale_revisions_cannot_start_a_run() {
    let (db, _, service, _, id) = setup().await;
    assert!(matches!(
        service.start_run("other", &id, request()).await,
        Err(ProductFactoryError::NotFound(_))
    ));
    assert!(matches!(
        service.get_execution("other", &id).await,
        Err(ProductFactoryError::NotFound(_))
    ));
    assert!(matches!(
        service
            .start_run("owner", &id, StartProductFactoryRequest { expected_revision: 2 })
            .await,
        Err(ProductFactoryError::RevisionConflict)
    ));
    assert_eq!(message_count(&db).await, 0);
}

#[tokio::test]
async fn changed_team_or_task_is_blocked_before_any_dispatch() {
    for invalid in ["members", "workspace", "task"] {
        let (db, repo, service, _, id) = setup().await;
        let team_id = repo.get_run("owner", &id).await.unwrap().unwrap().team_id.unwrap();
        match invalid {
            "members" => {
                sqlx::query("UPDATE teams SET agents = '[]' WHERE id = ?")
                    .bind(&team_id)
                    .execute(db.pool())
                    .await
                    .unwrap();
            }
            "workspace" => {
                sqlx::query("UPDATE teams SET workspace = '/changed' WHERE id = ?")
                    .bind(&team_id)
                    .execute(db.pool())
                    .await
                    .unwrap();
            }
            _ => {
                sqlx::query("UPDATE team_tasks SET status = 'in_progress' WHERE team_id = ?")
                    .bind(&team_id)
                    .execute(db.pool())
                    .await
                    .unwrap();
            }
        }
        assert!(
            matches!(
                service.start_run("owner", &id, request()).await,
                Err(ProductFactoryError::ExecutionBlocked(_))
            ),
            "accepted {invalid}"
        );
        assert_eq!(message_count(&db).await, 0);
        assert!(repo.get_execution("owner", &id).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn a_crash_after_reservation_never_turns_a_reload_into_a_second_attempt() {
    let (db, repo, service, _, id) = setup().await;
    let row = repo.get_run("owner", &id).await.unwrap().unwrap();
    let team = repo
        .get_handoff_team("owner", row.team_id.as_deref().unwrap())
        .await
        .unwrap()
        .unwrap();
    let task = repo
        .get_handoff_task("owner", &team.id, &format!("{}-data-1", team.id))
        .await
        .unwrap()
        .unwrap();
    repo.reserve_execution(&row, &team, &task, 10).await.unwrap();
    let receipt = service
        .start_run("owner", &id, request())
        .await
        .unwrap()
        .execution
        .unwrap();
    assert_eq!(receipt.state.as_str(), "pending");
    assert_eq!(message_count(&db).await, 0);
}

#[tokio::test]
async fn mailbox_success_followed_by_receipt_storage_failure_is_not_resent() {
    let (db, _, service, _, id) = setup().await;
    sqlx::query(
        "CREATE TRIGGER reject_receipt BEFORE UPDATE ON product_factory_execution \
        BEGIN SELECT RAISE(ABORT, 'fixture receipt failure'); END",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        service.start_run("owner", &id, request()).await,
        Err(ProductFactoryError::Database(_))
    ));
    let receipt = service
        .start_run("owner", &id, request())
        .await
        .unwrap()
        .execution
        .unwrap();
    assert_eq!(receipt.state.as_str(), "pending");
    assert_eq!(message_count(&db).await, 1);
}

#[tokio::test]
async fn changed_acceptance_criteria_are_rejected_instead_of_silently_overridden() {
    let (db, _, service, _, id) = setup().await;
    sqlx::query("UPDATE team_tasks SET metadata = json_set(metadata, '$.product_factory.acceptance_criteria', json('[\"Changed criteria\"]'))")
        .execute(db.pool()).await.unwrap();
    assert!(matches!(
        service.start_run("owner", &id, request()).await,
        Err(ProductFactoryError::ExecutionBlocked(_))
    ));
    assert_eq!(message_count(&db).await, 0);
}

struct ExhaustedBudgetReviewPort(ReviewPort);
#[async_trait::async_trait]
impl ProductFactoryReviewPort for ExhaustedBudgetReviewPort {
    async fn get_task_snapshot(
        &self,
        user: &str,
        team: &str,
        task: &str,
    ) -> Result<ProductFactoryTaskReviewSnapshot, ProductFactoryError> {
        let mut snapshot = self.0.get_task_snapshot(user, team, task).await?;
        snapshot.usage_summary.budget_limit_usd = Some(0.01);
        snapshot.usage_summary.budget_remaining_usd = Some(0.0);
        snapshot.usage_summary.budget_exceeded = true;
        Ok(snapshot)
    }
    async fn review_task(
        &self,
        user: &str,
        team: &str,
        task: &str,
        request: TeamTaskReviewRequest,
    ) -> Result<TeamTaskResponse, ProductFactoryError> {
        self.0.review_task(user, team, task, request).await
    }
}

#[tokio::test]
async fn exhausted_budget_keeps_otherwise_complete_delivery_unready() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let row = repo.get_run("owner", &id).await.unwrap().unwrap();
    sqlx::query("UPDATE team_tasks SET status='completed' WHERE team_id=?")
        .bind(&row.team_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE product_factory_runs SET status='completed' WHERE id=?")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();
    std::fs::write(
        std::path::Path::new(&row.workspace_path).join("START.md"),
        "fixture start guide",
    )
    .unwrap();
    std::fs::write(
        std::path::Path::new(&row.workspace_path).join("ACCEPTANCE.md"),
        "fixture acceptance",
    )
    .unwrap();
    let service = service.with_review_port(Arc::new(ExhaustedBudgetReviewPort(ReviewPort {
        pool: db.pool().clone(),
    })));
    let delivery = service.get_delivery("owner", &id).await.unwrap();
    assert!(!delivery.ready);
    assert!(
        delivery
            .checks
            .iter()
            .any(|check| check.code == "budget_within_limit" && !check.passed)
    );
    assert!(
        delivery
            .checks
            .iter()
            .filter(|check| check.code != "budget_within_limit")
            .all(|check| check.passed)
    );
}

#[tokio::test]
async fn runtime_usage_marks_a_handed_off_workflow_running_after_enqueue() {
    let (db, repo, service, _, id) = setup().await;
    service.start_run("owner", &id, request()).await.unwrap();
    let row = repo.get_run("owner", &id).await.unwrap().unwrap();
    assert_eq!(row.status, "handed_off", "enqueue is not execution proof");
    sqlx::query("UPDATE team_tasks SET status='in_progress' WHERE id=?")
        .bind(format!("{}-data-1", row.team_id.unwrap()))
        .execute(db.pool())
        .await
        .unwrap();
    let service = service.with_review_port(Arc::new(ReviewPort {
        pool: db.pool().clone(),
    }));
    let review = service.get_review("owner", &id).await.unwrap();
    assert_eq!(review.run.status, aionui_api_types::ProductFactoryRunStatus::Running);
}
