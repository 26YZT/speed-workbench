//! Controlled model fixtures only; the reported dollar amounts are not real rates.
use aionui_api_types::{
    ApplyProductFactoryPlanningRequest as Apply, CreateProductFactoryRunRequest, ProductFactoryPlanningPhase as Phase,
    StartProductFactoryPlanningRequest as Start,
};
use aionui_db::{
    AgentUsageRow, Database, IAgentUsageRepository, IProductFactoryPlanningRepository, SqliteAgentUsageRepository,
    SqliteProductFactoryPlanningRepository, SqliteProductFactoryRepository, init_database_memory,
};
use aionui_product_factory::{
    ProductFactoryError, ProductFactoryPlanningInvocation, ProductFactoryPlanningOutcome, ProductFactoryPlanningPort,
    ProductFactoryPlanningPrepared, ProductFactoryPlanningStarted, ProductFactoryService,
};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
struct Behavior {
    output: Option<String>,
    complete: bool,
    reported: bool,
    cost: Option<f64>,
}
impl Default for Behavior {
    fn default() -> Self {
        Self {
            output: None,
            complete: true,
            reported: true,
            cost: Some(0.1),
        }
    }
}
struct FixturePort {
    usage: Arc<dyn IAgentUsageRepository>,
    service: Mutex<Weak<ProductFactoryService>>,
    behavior: Mutex<Behavior>,
    prepared: AtomicUsize,
    model_calls: AtomicUsize,
}
#[async_trait::async_trait]
impl ProductFactoryPlanningPort for FixturePort {
    async fn prepare(
        &self,
        input: &ProductFactoryPlanningInvocation,
    ) -> Result<ProductFactoryPlanningPrepared, ProductFactoryError> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(ProductFactoryPlanningPrepared {
            conversation_id: format!("fixture-{}", input.attempt_id),
            assistant_snapshot: serde_json::json!({"fixture_only":true,"assistant_id":input.assistant_id}),
        })
    }
    async fn run(
        &self,
        input: &ProductFactoryPlanningInvocation,
        conversation: &str,
        on_started: ProductFactoryPlanningStarted,
    ) -> Result<ProductFactoryPlanningOutcome, ProductFactoryError> {
        let turn = format!("turn-{}", input.attempt_id);
        on_started(turn.clone()).await;
        let service = self.service.lock().unwrap().upgrade().unwrap();
        if service
            .check_planning_send_budget(&input.user_id, &input.run_id, conversation, &turn)
            .await
            .is_err()
        {
            return Ok(ProductFactoryPlanningOutcome {
                app_turn_id: turn,
                completed: false,
                output_complete: false,
                final_text: None,
                admission_rejected: true,
            });
        }
        self.model_calls.fetch_add(1, Ordering::SeqCst);
        let behavior = self.behavior.lock().unwrap().clone();
        if behavior.reported {
            self.usage
                .record_usage(&AgentUsageRow {
                    id: format!("usage-{}", input.attempt_id),
                    user_id: input.user_id.clone(),
                    team_id: None,
                    task_id: None,
                    agent_id: Some("fixture-model".into()),
                    model: Some("fixture-model".into()),
                    input_tokens: 100,
                    output_tokens: 10,
                    cost_est: behavior.cost,
                    cached_read_tokens: Some(80),
                    cached_write_tokens: Some(0),
                    cost_source: behavior.cost.map(|_| "fixture_report_not_real_rates".into()),
                    pricing_snapshot: None,
                    cost_unknown_reason: behavior.cost.is_none().then(|| "model_price_missing".into()),
                    conversation_id: conversation.into(),
                    turn_id: turn.clone(),
                    attempt_id: input.attempt_id.clone(),
                    created_at: 1,
                })
                .await?;
        }
        let output=behavior.output.unwrap_or_else(||match input.phase {
            Phase::Interview=>serde_json::json!({"questions":[{"id":"q1","question":"Who uses this CLI and what CSV fields matter?","reason":"Choose the contract before implementation"}]}).to_string(),
            Phase::Blueprint=>serde_json::json!({"sections":[{"id":"goals","title":"Goal","content":"A local CLI CSV summarizer"}],"requirements":[{"id":"r1","title":"Summarize CSV amounts","acceptance_criteria":["Known CSV yields exact total"]}],"risks":[],"open_questions":[]}).to_string(),
            Phase::TaskGraph=>graph().to_string(),
        });
        Ok(ProductFactoryPlanningOutcome {
            app_turn_id: turn,
            completed: true,
            output_complete: behavior.complete,
            final_text: Some(output),
            admission_rejected: false,
        })
    }
    async fn cancel(&self, _: &str, _: &str, _: &str) -> Result<(), ProductFactoryError> {
        Ok(())
    }
}
fn graph() -> serde_json::Value {
    serde_json::json!({"tasks":[
        {"id":"cli","title":"Build CLI","description":"Implement CSV domain and terminal entry","type":"backend","blocked_by":[],"acceptance_criteria":["CSV produces exact total"],"suggested_role":"backend","effort":"medium","execution_scope":"task_workspace","requirement_ids":["r1"]},
        {"id":"unit","title":"Check domain","description":"Verify parser errors in isolation","type":"test","blocked_by":["cli"],"acceptance_criteria":["Bad CSV rejected"],"suggested_role":"qa","effort":"low","execution_scope":"task_workspace","requirement_ids":["r1"]},
        {"id":"delivery","title":"Integrate delivery","description":"Assemble runnable CLI and START.md","type":"test","blocked_by":["unit"],"acceptance_criteria":["Independent CLI restart works"],"suggested_role":"qa","effort":"medium","execution_scope":"project_integration","requirement_ids":["r1"]}
    ]})
}
async fn setup(budget: Option<f64>) -> (Database, Arc<ProductFactoryService>, Arc<FixturePort>, String) {
    let db = init_database_memory().await.unwrap();
    let usage: Arc<dyn IAgentUsageRepository> = Arc::new(SqliteAgentUsageRepository::new(db.pool().clone()));
    let port = Arc::new(FixturePort {
        usage: usage.clone(),
        service: Mutex::new(Weak::new()),
        behavior: Mutex::new(Default::default()),
        prepared: Default::default(),
        model_calls: Default::default(),
    });
    let service = Arc::new(
        ProductFactoryService::new(Arc::new(SqliteProductFactoryRepository::new(db.pool().clone())))
            .with_planning_repository(Arc::new(SqliteProductFactoryPlanningRepository::new(db.pool().clone())))
            .with_usage_repository(usage)
            .with_planning_port(port.clone()),
    );
    *port.service.lock().unwrap() = Arc::downgrade(&service);
    let run = service
        .create_run(
            "owner",
            CreateProductFactoryRunRequest {
                name: "Fixture CLI".into(),
                idea: "CSV CLI".into(),
                target_user: None,
                problem: None,
                expected_output: None,
                workspace_path: Some("/tmp/fixture-cli".into()),
                budget_usd: budget,
            },
        )
        .await
        .unwrap();
    (db, service, port, run.id)
}
fn request(phase: Phase, revision: u64, key: &str) -> Start {
    Start {
        phase,
        expected_plan_revision: revision,
        idempotency_key: key.into(),
        assistant_id: "fixture-assistant".into(),
        model: "".into(),
    }
}
async fn settled(
    service: &ProductFactoryService,
    run: &str,
    id: &str,
) -> aionui_api_types::ProductFactoryPlanningResponse {
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        loop {
            let result = service.get_planning("owner", run, id).await.unwrap();
            if !matches!(result.state.as_str(), "reserved" | "preparing" | "running") {
                return result;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn two_windows_reserve_once_and_apply_is_owned_revision_cas_without_confirmation() {
    let (_db, service, port, run) = setup(Some(1.0)).await;
    let (a, b) = tokio::join!(
        service.start_planning("owner", &run, request(Phase::Interview, 1, "same-key")),
        service.start_planning("owner", &run, request(Phase::Interview, 1, "same-key"))
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.id, b.id);
    let candidate = settled(&service, &run, &a.id).await;
    assert_eq!(candidate.state, "candidate_ready");
    assert_eq!(port.model_calls.load(Ordering::SeqCst), 1);
    assert!(matches!(
        service.get_planning("foreign", &run, &a.id).await,
        Err(ProductFactoryError::NotFound(_))
    ));
    assert!(matches!(
        service
            .apply_planning(
                "owner",
                &run,
                &a.id,
                Apply {
                    expected_plan_revision: 2
                }
            )
            .await,
        Err(ProductFactoryError::RevisionConflict)
    ));
    let applied = service
        .apply_planning(
            "owner",
            &run,
            &a.id,
            Apply {
                expected_plan_revision: 1,
            },
        )
        .await
        .unwrap();
    assert_eq!(applied.plan_revision, 2);
    assert_eq!(applied.interview.unwrap()["confirmed"], false);
    assert_eq!(
        service.get_planning("owner", &run, &a.id).await.unwrap().state,
        "applied"
    );
    assert!(matches!(
        service
            .apply_planning(
                "owner",
                &run,
                &a.id,
                Apply {
                    expected_plan_revision: 1
                }
            )
            .await,
        Err(ProductFactoryError::RevisionConflict)
    ));
    assert_eq!(port.prepared.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn full_planning_candidates_form_cli_graph_and_costs_remain_visible_before_handoff() {
    let (_db, service, port, run) = setup(Some(1.0)).await;
    let first = service
        .start_planning("owner", &run, request(Phase::Interview, 1, "interview"))
        .await
        .unwrap();
    settled(&service, &run, &first.id).await;
    service
        .apply_planning(
            "owner",
            &run,
            &first.id,
            Apply {
                expected_plan_revision: 1,
            },
        )
        .await
        .unwrap();
    service.save_interview("owner",&run,serde_json::json!({"summary":"CLI users need local CSV totals","questions":[{"id":"q1","question":"Audience?","answer":"Finance staff","not_sure":false}]})).await.unwrap();
    let confirmed = service.confirm_interview("owner", &run).await.unwrap();
    let blue = service
        .start_planning(
            "owner",
            &run,
            request(Phase::Blueprint, confirmed.plan_revision, "blueprint"),
        )
        .await
        .unwrap();
    settled(&service, &run, &blue.id).await;
    service
        .apply_planning(
            "owner",
            &run,
            &blue.id,
            Apply {
                expected_plan_revision: confirmed.plan_revision,
            },
        )
        .await
        .unwrap();
    let confirmed = service.confirm_blueprint("owner", &run).await.unwrap();
    let tasks = service
        .start_planning(
            "owner",
            &run,
            request(Phase::TaskGraph, confirmed.plan_revision, "tasks"),
        )
        .await
        .unwrap();
    let candidate = settled(&service, &run, &tasks.id).await;
    assert_eq!(candidate.state, "candidate_ready");
    let artifact = candidate.candidate.unwrap();
    assert_eq!(artifact["version"], 2);
    assert_eq!(artifact["generated_by"], "model");
    assert_eq!(artifact["tasks"].as_array().unwrap().len(), 3);
    assert_eq!(artifact["tasks"][1]["execution_scope"], "task_workspace");
    assert!(
        !artifact["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["type"] == "frontend")
    );
    let applied = service
        .apply_planning(
            "owner",
            &run,
            &tasks.id,
            Apply {
                expected_plan_revision: confirmed.plan_revision,
            },
        )
        .await
        .unwrap();
    let report = service.get_cost_report("owner", &run).await.unwrap();
    assert_eq!(report.planning_usage.len(), 3);
    assert_eq!(report.usage.len(), 3);
    assert_eq!(report.usage_summary.cost_est, Some(0.3));
    assert!(!report.usage_summary.cost_unknown);
    assert!(applied.team_id.is_none());
    assert_eq!(port.model_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn incomplete_or_non_strict_json_never_creates_a_candidate_or_rule_fallback() {
    for (output, complete) in [
        ("{\"questions\":[", false),
        ("```json\n{\"questions\":[]}\n```", true),
        ("{\"questions\":[],\"confirmed\":true}", true),
    ] {
        let (_db, service, port, run) = setup(Some(1.0)).await;
        *port.behavior.lock().unwrap() = Behavior {
            output: Some(output.into()),
            complete,
            ..Default::default()
        };
        let attempt = service
            .start_planning("owner", &run, request(Phase::Interview, 1, "invalid"))
            .await
            .unwrap();
        let final_state = settled(&service, &run, &attempt.id).await;
        assert_eq!(final_state.state, "failed");
        assert!(final_state.candidate.is_none());
        assert!(service.get_run("owner", &run).await.unwrap().interview.is_none());
        assert_eq!(
            service
                .get_cost_report("owner", &run)
                .await
                .unwrap()
                .usage_summary
                .cost_est,
            Some(0.1)
        );
    }
}

#[tokio::test]
async fn input_edit_rejects_stale_candidate_without_refunding_its_cost() {
    let (_db, service, _port, run) = setup(Some(1.0)).await;
    let attempt = service
        .start_planning("owner", &run, request(Phase::Interview, 1, "original"))
        .await
        .unwrap();
    settled(&service, &run, &attempt.id).await;
    service
        .save_interview(
            "owner",
            &run,
            serde_json::json!({"summary":"Human changed the requirement"}),
        )
        .await
        .unwrap();
    assert!(matches!(
        service
            .apply_planning(
                "owner",
                &run,
                &attempt.id,
                Apply {
                    expected_plan_revision: 1
                }
            )
            .await,
        Err(ProductFactoryError::RevisionConflict)
    ));
    assert_eq!(
        service
            .get_cost_report("owner", &run)
            .await
            .unwrap()
            .usage_summary
            .cost_est,
        Some(0.1)
    );
}

#[tokio::test]
async fn budget_unknown_and_exhausted_block_new_calls_but_unbudgeted_unknown_can_continue() {
    for budget in [Some(1.0), None] {
        let (_db, service, port, run) = setup(budget).await;
        port.behavior.lock().unwrap().cost = None;
        let attempt = service
            .start_planning("owner", &run, request(Phase::Interview, 1, "unknown"))
            .await
            .unwrap();
        settled(&service, &run, &attempt.id).await;
        service.cancel_planning("owner", &run, &attempt.id).await.unwrap();
        let retry = service
            .start_planning("owner", &run, request(Phase::Interview, 1, "retry"))
            .await;
        if budget.is_some() {
            assert!(
                matches!(retry,Err(ProductFactoryError::PlanningBlocked(ref code)) if code=="PLANNING_COST_UNKNOWN")
            );
            assert_eq!(port.model_calls.load(Ordering::SeqCst), 1);
        } else {
            settled(&service, &run, &retry.unwrap().id).await;
            assert_eq!(port.model_calls.load(Ordering::SeqCst), 2);
        }
    }
    let (_db, service, port, run) = setup(Some(0.1)).await;
    let attempt = service
        .start_planning("owner", &run, request(Phase::Interview, 1, "full-budget"))
        .await
        .unwrap();
    settled(&service, &run, &attempt.id).await;
    service.cancel_planning("owner", &run, &attempt.id).await.unwrap();
    assert!(
        matches!(service.start_planning("owner",&run,request(Phase::Interview,1,"no-room")).await,Err(ProductFactoryError::PlanningBlocked(ref code)) if code=="PLANNING_BUDGET_EXHAUSTED")
    );
    assert_eq!(port.model_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn first_send_requires_bound_app_turn_and_a_persisted_marker_blocks_later_unknown_send() {
    let (db, service, port, run) = setup(Some(1.0)).await;
    sqlx::query("CREATE TRIGGER reject_app_turn BEFORE UPDATE OF app_turn_id ON product_factory_planning BEGIN SELECT RAISE(FAIL,'fixture binding failure'); END").execute(db.pool()).await.unwrap();
    let attempt = service
        .start_planning("owner", &run, request(Phase::Interview, 1, "binding-error"))
        .await
        .unwrap();
    let result = settled(&service, &run, &attempt.id).await;
    assert_eq!(result.error_code.as_deref(), Some("USER_MODEL_SEND_ADMISSION_REJECTED"));
    assert_eq!(port.model_calls.load(Ordering::SeqCst), 0);
    let report = service.get_cost_report("owner", &run).await.unwrap();
    assert_eq!(report.usage.len(), 0);
    assert_eq!(report.usage_summary.cost_est, Some(0.0));
    assert!(!report.usage_summary.cost_unknown);
    sqlx::query("DROP TRIGGER reject_app_turn")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO product_factory_planning(id,user_id,run_id,phase,input_plan_revision,input_hash,idempotency_key,assistant_id,model,state,conversation_id,app_turn_id,started_at,created_at,updated_at) VALUES('guard','owner',?,'interview',1,'fixture','guard','fixture-assistant','','running','guard-conversation','correct-app-turn',1,2,2)").bind(&run).execute(db.pool()).await.unwrap();
    assert!(
        matches!(service.check_planning_send_budget("owner",&run,"guard-conversation","wrong-app-turn").await,Err(ProductFactoryError::PlanningBlocked(ref code)) if code=="PLANNING_APP_TURN_NOT_BOUND")
    );
    service
        .check_planning_send_budget("owner", &run, "guard-conversation", "correct-app-turn")
        .await
        .unwrap();
    port.usage
        .ensure_turn_admission(&AgentUsageRow {
            id: "marker".into(),
            user_id: "owner".into(),
            team_id: None,
            task_id: None,
            agent_id: None,
            model: None,
            input_tokens: 0,
            output_tokens: 0,
            cost_est: None,
            cached_read_tokens: None,
            cached_write_tokens: None,
            cost_source: None,
            pricing_snapshot: None,
            cost_unknown_reason: None,
            conversation_id: "guard-conversation".into(),
            turn_id: "correct-app-turn".into(),
            attempt_id: "pending".into(),
            created_at: 3,
        })
        .await
        .unwrap();
    assert!(
        matches!(service.check_planning_send_budget("owner",&run,"guard-conversation","correct-app-turn").await,Err(ProductFactoryError::PlanningBlocked(ref code)) if code=="PLANNING_COST_UNKNOWN")
    );
}

#[tokio::test]
async fn persisted_running_attempt_recovers_uncertain_without_resending_and_keeps_unknown_cost() {
    let (db, service, port, run) = setup(Some(1.0)).await;
    let repo = SqliteProductFactoryPlanningRepository::new(db.pool().clone());
    let current = service.get_run("owner", &run).await.unwrap();
    let raw = serde_json::json!({"id":"orphan","user_id":"owner","run_id":run,"phase":"interview","input_plan_revision":1,"input_hash":"fixture","idempotency_key":"lost-response","assistant_id":"fixture-assistant","model":""});
    sqlx::query("INSERT INTO product_factory_planning(id,user_id,run_id,phase,input_plan_revision,input_hash,idempotency_key,assistant_id,model,state,started_at,created_at,updated_at) VALUES('orphan','owner',?,'interview',1,'fixture','lost-response','fixture-assistant','','running',1,1,1)").bind(&run).execute(db.pool()).await.unwrap();
    let orphan = service.get_planning("owner", &run, "orphan").await.unwrap();
    assert_eq!(orphan.state, "uncertain");
    assert_eq!(orphan.error_code.as_deref(), Some("PLANNING_RESTART_UNCERTAIN"));
    let replay = service
        .start_planning(
            "owner",
            &run,
            request(Phase::Interview, current.plan_revision, "lost-response"),
        )
        .await
        .unwrap();
    assert_eq!(replay.id, "orphan");
    assert_eq!(port.model_calls.load(Ordering::SeqCst), 0);
    assert!(
        matches!(service.start_planning("owner",&run,request(Phase::Interview,1,"fresh")).await,Err(ProductFactoryError::PlanningBlocked(ref code)) if code=="PLANNING_UNCERTAIN_ATTEMPT")
    );
    assert!(
        service
            .get_cost_report("owner", &run)
            .await
            .unwrap()
            .usage_summary
            .cost_unknown
    );
    assert_eq!(repo.list("foreign", &run).await.unwrap().len(), 0);
    assert_eq!(raw["input_plan_revision"], 1);
}

#[tokio::test]
async fn usage_storage_error_fails_closed_and_does_not_start_another_model_call() {
    let (db, service, port, run) = setup(Some(1.0)).await;
    let attempt = service
        .start_planning("owner", &run, request(Phase::Interview, 1, "first"))
        .await
        .unwrap();
    settled(&service, &run, &attempt.id).await;
    service.cancel_planning("owner", &run, &attempt.id).await.unwrap();
    sqlx::query("ALTER TABLE agent_usage RENAME TO unavailable_usage_fixture")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        service
            .start_planning("owner", &run, request(Phase::Interview, 1, "second"))
            .await,
        Err(ProductFactoryError::Database(_))
    ));
    assert_eq!(port.model_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_model_graph_scope_dependencies_and_requirement_links_are_rejected() {
    for variant in 0..4 {
        let (db, service, port, run) = setup(Some(1.0)).await;
        sqlx::query("UPDATE product_factory_runs SET status='blueprint_ready',blueprint_json=? WHERE id=?")
            .bind(serde_json::json!({"version":2,"generated_by":"model","confirmed":true,"requirements":[{"id":"r1","title":"CSV totals","acceptance_criteria":["Known CSV total"]}],"sections":[{"id":"goals","title":"Goal","content":"A CLI"}]}).to_string()).bind(&run).execute(db.pool()).await.unwrap();
        let mut output = graph();
        match variant {
            0 => output["tasks"][1]["execution_scope"] = serde_json::json!("project_integration"),
            1 => output["tasks"][2]["blocked_by"] = serde_json::json!([]),
            2 => output["tasks"][0]["requirement_ids"] = serde_json::json!(["missing"]),
            _ => output["tasks"][0]["blocked_by"] = serde_json::json!(["unit"]),
        };
        port.behavior.lock().unwrap().output = Some(output.to_string());
        let attempt = service
            .start_planning("owner", &run, request(Phase::TaskGraph, 1, "invalid-graph"))
            .await
            .unwrap();
        let outcome = settled(&service, &run, &attempt.id).await;
        assert_eq!(outcome.state, "failed");
        assert_eq!(outcome.error_code.as_deref(), Some("PLANNING_INVALID_TASK_GRAPH"));
        assert!(outcome.candidate.is_none());
        assert!(service.get_run("owner", &run).await.unwrap().task_draft.is_none());
    }
}
