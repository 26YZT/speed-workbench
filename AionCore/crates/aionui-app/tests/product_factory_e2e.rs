mod common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{build_app, get_with_token, json_with_token, setup_and_login};

#[tokio::test]
async fn planning_routes_require_auth_csrf_and_owner_before_any_model_invocation() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    for (path, body) in [
        (
            "/api/product-factory/runs/missing/planning",
            json!({"phase":"interview","expected_plan_revision":1,"idempotency_key":"fixture","assistant_id":"fixture","model":""}),
        ),
        (
            "/api/product-factory/runs/missing/planning/attempt/apply",
            json!({"expected_plan_revision":1}),
        ),
        ("/api/product-factory/runs/missing/planning/attempt/cancel", json!({})),
    ] {
        let unauthenticated = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .header("x-csrf-token", &csrf)
                    .header("cookie", format!("aionui-csrf-token={csrf}"))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
        let no_csrf = app
            .clone()
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);
    }
    let response = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            "/api/product-factory/runs",
            json!({"name":"Owned planning","idea":"A CLI"}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    let id = common::body_json(response).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (foreign, foreign_csrf) = setup_and_login(&mut app, &services, "other", "StrongP@ss2").await;
    let get = app
        .clone()
        .oneshot(get_with_token(
            &format!("/api/product-factory/runs/{id}/planning"),
            &foreign,
        ))
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::NOT_FOUND);
    let post=app.clone().oneshot(json_with_token("POST",&format!("/api/product-factory/runs/{id}/planning"),json!({"phase":"interview","expected_plan_revision":1,"idempotency_key":"foreign","assistant_id":"fixture","model":""}),&foreign,&foreign_csrf)).await.unwrap();
    assert_eq!(post.status(), StatusCode::NOT_FOUND);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_factory_planning")
        .fetch_one(services.database.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(services.worker_task_manager.active_count(), 0);
    services.database.close().await;
}

#[tokio::test]
async fn planning_preparation_uses_a_separate_folder_and_snapshot_without_warming_runtime() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    seed_handoff_assistant(&app, &services, &token, &csrf).await;
    let response = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            "/api/product-factory/runs",
            json!({"name":"Planning prep fixture","idea":"A local CLI"}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    let run = common::body_json(response).await;
    let id = run["data"]["id"].as_str().unwrap();
    let product_folder = run["data"]["workspace_path"].as_str().unwrap();
    sqlx::query("CREATE TRIGGER cancel_before_model_fixture BEFORE UPDATE OF state ON product_factory_planning WHEN OLD.state='preparing' AND NEW.state='running' BEGIN UPDATE product_factory_planning SET state='cancelled' WHERE id=OLD.id; SELECT RAISE(IGNORE); END").execute(services.database.pool()).await.unwrap();
    let response=app.clone().oneshot(json_with_token("POST",&format!("/api/product-factory/runs/{id}/planning"),json!({"phase":"interview","expected_plan_revision":1,"idempotency_key":"prepare-only","assistant_id":"factory-e2e-assistant","model":""}),&token,&csrf)).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let attempt = common::body_json(response).await;
    let attempt_id = attempt["data"]["id"].as_str().unwrap();
    let prepared = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let response = app
                .clone()
                .oneshot(get_with_token(
                    &format!("/api/product-factory/runs/{id}/planning/{attempt_id}"),
                    &token,
                ))
                .await
                .unwrap();
            let result = common::body_json(response).await;
            if result["data"]["state"] == "cancelled" {
                break result;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(prepared["data"]["app_turn_id"].is_null());
    let conversation_id = prepared["data"]["conversation_id"].as_str().unwrap();
    let user: String = sqlx::query_scalar("SELECT id FROM users WHERE username='admin'")
        .fetch_one(services.database.pool())
        .await
        .unwrap();
    let conversation = services
        .conversation_repo
        .get(&user, conversation_id)
        .await
        .unwrap()
        .unwrap();
    let extra: Value = serde_json::from_str(&conversation.extra).unwrap();
    let planning_folder = extra["workspace"].as_str().unwrap();
    assert_ne!(planning_folder, product_folder);
    assert!(
        std::path::Path::new(planning_folder)
            .starts_with(std::fs::canonicalize(services.data_dir.join("product-factory-planning")).unwrap())
    );
    assert!(std::path::Path::new(planning_folder).is_dir());
    assert_eq!(extra["factory_planning_run_id"], id);
    assert!(extra.get("teamId").is_none());
    let snapshot = services
        .conversation_repo
        .get_assistant_snapshot(&user, conversation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.assistant_id, "factory-e2e-assistant");
    assert_eq!(services.worker_task_manager.active_count(), 0);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_usage")
        .fetch_one(services.database.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
    services.database.close().await;
}

#[tokio::test]
async fn factory_execution_budget_includes_planning_and_report_is_readable_before_execution() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    seed_handoff_assistant(&app, &services, &token, &csrf).await;
    let workspace = tempfile::tempdir().unwrap();
    let id = confirmed_run(&app, &token, &csrf, workspace.path().to_str().unwrap()).await;
    let response=app.clone().oneshot(json_with_token("POST",&format!("/api/product-factory/runs/{id}/handoff"),json!({"expected_revision":1,"agents":[{"name":"Developer","role":"lead","assistant_id":"factory-e2e-assistant","model":"claude"}]}),&token,&csrf)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let handoff = common::body_json(response).await;
    let team = handoff["data"]["team_id"].as_str().unwrap();
    let task = format!("{team}-data-1");
    let owner: String = sqlx::query_scalar("SELECT id FROM users WHERE username='admin'")
        .fetch_one(services.database.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO product_factory_planning(id,user_id,run_id,phase,input_plan_revision,input_hash,idempotency_key,assistant_id,model,conversation_id,state,started_at,created_at,updated_at) VALUES('fixture-planning',?,?,'blueprint',1,'fixture','fixture','fixture','fixture-model','fixture-planning-conversation','applied',1,1,1)").bind(&owner).bind(&id).execute(services.database.pool()).await.unwrap();
    for (name, conversation, task_id, cost) in [
        ("planning", "fixture-planning-conversation", None, 0.6),
        ("execution", "fixture-execution-conversation", Some(task.as_str()), 0.3),
    ] {
        sqlx::query("INSERT INTO agent_usage(id,user_id,task_id,model,input_tokens,output_tokens,cost_est,cached_read_tokens,cached_write_tokens,cost_source,conversation_id,turn_id,created_at) VALUES(?,?,?,'fixture-model',100,10,?,80,0,'fixture_cost_not_real_rates',?,'fixture-turn',1)").bind(name).bind(&owner).bind(task_id).bind(cost).bind(conversation).execute(services.database.pool()).await.unwrap();
    }
    let budget = app
        .clone()
        .oneshot(json_with_token(
            "PUT",
            &format!("/api/teams/{team}/budget"),
            json!({"limit_usd":0.8}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(budget.status(), StatusCode::OK);
    let report = app
        .clone()
        .oneshot(get_with_token(
            &format!("/api/product-factory/runs/{id}/cost-report"),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(report.status(), StatusCode::OK);
    let report = common::body_json(report).await;
    assert_eq!(report["data"]["usage_summary"]["cost_est"], 0.9);
    assert_eq!(report["data"]["usage_summary"]["budget_exceeded"], true);
    assert_eq!(report["data"]["planning_usage"].as_array().unwrap().len(), 1);
    assert_eq!(report["data"]["usage"].as_array().unwrap().len(), 2);
    assert!(!workspace.path().join(".tasks").exists());
    let response = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            &format!("/api/product-factory/runs/{id}/start"),
            json!({"expected_revision":1}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        common::body_json(response).await["code"],
        "PRODUCT_FACTORY_EXECUTION_BLOCKED"
    );
    assert_eq!(services.worker_task_manager.active_count(), 0);
    services.database.close().await;
}

#[tokio::test]
async fn startup_does_not_price_legacy_usage_without_captured_cache_buckets() {
    let data = tempfile::tempdir().unwrap();
    std::fs::write(data.path().join("model-pricing.json"), r#"{"version":1,"models":{"cache-model":{"input_usd_per_million":2,"output_usd_per_million":4,"input_includes_cached_tokens":true,"cached_read_usd_per_million":0.1,"cached_write_usd_per_million":0.2}}}"#).unwrap();
    let database = aionui_db::init_database_memory().await.unwrap();
    sqlx::query("INSERT INTO agent_usage (id,user_id,task_id,model,input_tokens,output_tokens,cost_est,conversation_id,turn_id,created_at) VALUES ('legacy','owner','task','cache-model',1000,500,NULL,'conversation','turn',1)")
        .execute(database.pool()).await.unwrap();
    let config = aionui_app::AppConfig {
        data_dir: data.path().into(),
        work_dir: data.path().into(),
        ..Default::default()
    };
    let services = aionui_app::AppServices::from_config(database, &config).await.unwrap();
    let cost: Option<f64> = sqlx::query_scalar("SELECT cost_est FROM agent_usage WHERE id = 'legacy'")
        .fetch_one(services.database.pool())
        .await
        .unwrap();
    assert_eq!(
        cost, None,
        "missing historical cache usage must not be treated as zero cache usage"
    );
    services.database.close().await;
}

#[tokio::test]
async fn product_factory_routes_are_wired_into_the_application() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    let create = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            "/api/product-factory/runs",
            json!({"name": "Alpha", "idea": "Build a better brief"}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created = common::body_json(create).await;
    let folder = std::path::Path::new(created["data"]["workspace_path"].as_str().unwrap());
    assert!(
        folder.is_absolute() && folder.is_dir(),
        "a name and idea must be sufficient to prepare a usable local workspace"
    );

    let list = app
        .oneshot(get_with_token("/api/product-factory/runs", &token))
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
    assert_eq!(json["data"][0]["name"], "Alpha");

    services.database.close().await;
}

#[tokio::test]
async fn product_factory_mutations_require_csrf() {
    let (mut app, services) = build_app().await;
    let (token, _csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;

    let response = app
        .oneshot(
            Request::post("/api/product-factory/runs")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(r#"{"name":"Alpha","idea":"Build a better brief"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    services.database.close().await;
}

#[tokio::test]
async fn product_factory_routes_require_authentication() {
    let database = aionui_db::init_database_memory().await.unwrap();
    let services = aionui_app::AppServices::from_config(database, &aionui_app::AppConfig::default())
        .await
        .unwrap();
    let app = aionui_app::create_router(&services).await.expect("build router");

    let response = app
        .oneshot(Request::get("/api/product-factory/runs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    services.database.close().await;
}

#[tokio::test]
async fn handoff_mutation_requires_authentication_and_csrf() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    let path = "/api/product-factory/runs/missing/handoff";
    // Satisfy the outer CSRF layer so this request specifically tests auth.
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .header("x-csrf-token", &csrf)
                .header("cookie", format!("aionui-csrf-token={csrf}"))
                .body(Body::from(r#"{"expected_revision":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(r#"{"expected_revision":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    services.database.close().await;
}

#[tokio::test]
async fn factory_start_requires_authentication_and_csrf() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    let path = "/api/product-factory/runs/missing/start";
    let unauthenticated = app
        .clone()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .header("x-csrf-token", &csrf)
                .header("cookie", format!("aionui-csrf-token={csrf}"))
                .body(Body::from(r#"{"expected_revision":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    let no_csrf = app
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(r#"{"expected_revision":1}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);
    services.database.close().await;
}

#[tokio::test]
async fn real_execution_adapter_checks_budget_before_starting_any_runtime() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    seed_handoff_assistant(&app, &services, &token, &csrf).await;
    let workspace = tempfile::tempdir().unwrap();
    let id = confirmed_run(&app, &token, &csrf, workspace.path().to_str().unwrap()).await;
    let handoff = app.clone().oneshot(json_with_token("POST", &format!("/api/product-factory/runs/{id}/handoff"),
        json!({"expected_revision":1,"agents":[{"name":"Developer","role":"lead","assistant_id":"factory-e2e-assistant","model":"claude"}]}),
        &token, &csrf)).await.unwrap();
    assert_eq!(handoff.status(), StatusCode::OK);
    let result = common::body_json(handoff).await;
    let task_id = result["data"]["task_id_map"]["data-1"].as_str().unwrap();
    sqlx::query("INSERT INTO agent_usage (id, user_id, task_id, cost_est, conversation_id, turn_id, created_at) \
        SELECT 'spent', user_id, ?, 20, 'fixture-conversation', 'fixture-turn', 0 FROM product_factory_runs WHERE id = ?")
        .bind(task_id).bind(&id).execute(services.database.pool()).await.unwrap();
    let response = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            &format!("/api/product-factory/runs/{id}/start"),
            json!({"expected_revision":1}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error = common::body_json(response).await;
    assert_eq!(error["code"], "PRODUCT_FACTORY_EXECUTION_BLOCKED");
    assert_eq!(error["error"], "Team budget has been exhausted");
    let receipt = app
        .oneshot(get_with_token(
            &format!("/api/product-factory/runs/{id}/execution"),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(common::body_json(receipt).await["data"]["execution"], Value::Null);
    assert_eq!(services.worker_task_manager.active_count(), 0);
    services.database.close().await;
}

#[tokio::test]
async fn real_execution_adapter_blocks_unknown_cost_before_starting_any_runtime() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    seed_handoff_assistant(&app, &services, &token, &csrf).await;
    let workspace = tempfile::tempdir().unwrap();
    let id = confirmed_run(&app, &token, &csrf, workspace.path().to_str().unwrap()).await;
    let handoff = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            &format!("/api/product-factory/runs/{id}/handoff"),
            json!({"expected_revision":1,"agents":[{"name":"Developer","role":"lead","assistant_id":"factory-e2e-assistant","model":"claude"}]}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(handoff.status(), StatusCode::OK);
    let result = common::body_json(handoff).await;
    let task_id = result["data"]["task_id_map"]["data-1"].as_str().unwrap();
    sqlx::query(
        "INSERT INTO agent_usage (id, user_id, task_id, cost_est, conversation_id, turn_id, created_at) SELECT 'unknown-cost', user_id, ?, NULL, 'fixture-unknown-cost', 'fixture-turn', 0 FROM product_factory_runs WHERE id = ?",
    )
        .bind(task_id)
        .bind(&id)
        .execute(services.database.pool())
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            &format!("/api/product-factory/runs/{id}/start"),
            json!({"expected_revision":1}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error = common::body_json(response).await;
    assert_eq!(error["code"], "PRODUCT_FACTORY_COST_UNKNOWN");
    assert_eq!(
        error["error"],
        "Team cost cannot be verified while a budget is configured"
    );
    let receipt = app
        .oneshot(get_with_token(
            &format!("/api/product-factory/runs/{id}/execution"),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(common::body_json(receipt).await["data"]["execution"], Value::Null);
    assert_eq!(services.worker_task_manager.active_count(), 0);
    services.database.close().await;
}

async fn confirmed_run(app: &axum::Router, token: &str, csrf: &str, workspace: &str) -> String {
    let response = app.clone().oneshot(json_with_token("POST", "/api/product-factory/runs",
        json!({"name":"Handoff Product", "idea":"Build a reviewable product", "workspace_path":workspace, "budget_usd":10}), token, csrf)).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let value = common::body_json(response).await;
    let id = value["data"]["id"].as_str().unwrap().to_owned();
    for (method, suffix, body) in [
        (
            "PUT",
            "interview",
            json!({"interview":{"version":1,"questions":[],"summary":"Core flow"}}),
        ),
        ("POST", "interview/confirm", json!({})),
        ("POST", "blueprint/confirm", json!({})),
        ("POST", "task-draft/generate", json!({})),
        ("POST", "task-draft/confirm", json!({"expected_revision":1})),
    ] {
        let response = app
            .clone()
            .oneshot(json_with_token(
                method,
                &format!("/api/product-factory/runs/{id}/{suffix}"),
                body,
                token,
                csrf,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "failed stage {suffix}");
    }
    id
}

async fn seed_handoff_assistant(app: &axum::Router, services: &aionui_app::AppServices, token: &str, csrf: &str) {
    // Reuse the deterministic Team fixture pattern; this executable is registered
    // as available, but the handoff must never launch it.
    let command = std::env::current_exe().unwrap().to_string_lossy().into_owned();
    sqlx::query("UPDATE agent_metadata SET agent_source = 'custom', agent_source_info = ?, command = ?, args = '[]', env = '[]' WHERE agent_id = ?")
        .bind(json!({"binary_name":command}).to_string()).bind(&command).bind("2d23ff1c")
        .execute(services.database.pool()).await.unwrap();
    services.agent_registry.reload_one("2d23ff1c").await.unwrap();
    let response = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            "/api/assistants",
            json!({
                "id":"factory-e2e-assistant", "name":"Factory E2E", "agent_id":"2d23ff1c",
            }),
            token,
            csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn real_team_adapter_handoff_and_replay_create_one_pending_graph_without_runtime() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    seed_handoff_assistant(&app, &services, &token, &csrf).await;
    let workspace = tempfile::tempdir().unwrap();
    let id = confirmed_run(&app, &token, &csrf, workspace.path().to_str().unwrap()).await;
    let mut events = services.event_bus.subscribe();
    let path = format!("/api/product-factory/runs/{id}/handoff");
    let body = json!({"expected_revision":1,"agents":[{"name":"Developer","role":"lead","assistant_id":"factory-e2e-assistant","model":"claude"}]});
    let response = app
        .clone()
        .oneshot(json_with_token("POST", &path, body, &token, &csrf))
        .await
        .unwrap();
    let status = response.status();
    let first = common::body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let response = app
        .clone()
        .oneshot(json_with_token(
            "POST",
            &path,
            json!({"expected_revision":1}),
            &token,
            &csrf,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let second = common::body_json(response).await;
    assert_eq!(first["data"], second["data"]);
    let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM teams), (SELECT COUNT(*) FROM team_tasks WHERE status = 'pending'), (SELECT COUNT(*) FROM conversations)")
        .fetch_one(services.database.pool()).await.unwrap();
    assert_eq!(counts, (1, 4, 1));
    assert_eq!(services.worker_task_manager.active_count(), 0);
    let messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
        .fetch_one(services.database.pool())
        .await
        .unwrap();
    assert_eq!(messages, 0);
    assert_eq!(
        std::fs::read_dir(workspace.path()).unwrap().count(),
        0,
        "handoff must not write workspace files"
    );
    let mut notifications = Vec::new();
    while let Ok(event) = events.try_recv() {
        if event.name == aionui_team::events::TEAM_CREATED_EVENT {
            notifications.push(event);
        }
    }
    assert_eq!(
        notifications.len(),
        1,
        "committed Team is notified once; replay does not re-emit creation"
    );
    assert_eq!(notifications[0].data["team_id"], first["data"]["team_id"]);
    let team_id = first["data"]["team_id"].as_str().unwrap();
    let response = app
        .oneshot(get_with_token(&format!("/api/teams/{team_id}/tasks"), &token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let board = common::body_json(response).await;
    assert_eq!(board["data"].as_array().unwrap().len(), 4);
    services.database.close().await;
}

#[tokio::test]
async fn conversation_snapshot_failure_is_cleaned_up_before_handoff_retry() {
    let (mut app, services) = build_app().await;
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    seed_handoff_assistant(&app, &services, &token, &csrf).await;
    let workspace = tempfile::tempdir().unwrap();
    let id = confirmed_run(&app, &token, &csrf, workspace.path().to_str().unwrap()).await;
    sqlx::query("CREATE TRIGGER fail_factory_snapshot BEFORE INSERT ON conversation_assistant_snapshots BEGIN SELECT RAISE(FAIL, 'injected snapshot failure'); END")
        .execute(services.database.pool()).await.unwrap();
    let path = format!("/api/product-factory/runs/{id}/handoff");
    let body = json!({"expected_revision":1,"agents":[{"name":"Developer","role":"lead","assistant_id":"factory-e2e-assistant","model":"claude"}]});
    let response = app
        .clone()
        .oneshot(json_with_token("POST", &path, body.clone(), &token, &csrf))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM conversations), (SELECT COUNT(*) FROM teams), (SELECT COUNT(*) FROM team_tasks)",
    )
    .fetch_one(services.database.pool())
    .await
    .unwrap();
    assert_eq!(
        counts,
        (0, 0, 0),
        "failed preparation must not leak a persisted conversation"
    );
    let response = app
        .clone()
        .oneshot(get_with_token(&format!("/api/product-factory/runs/{id}"), &token))
        .await
        .unwrap();
    let run = common::body_json(response).await;
    assert_eq!(run["data"]["status"], "task_draft_ready");
    assert!(run["data"]["team_id"].is_null());
    sqlx::query("DROP TRIGGER fail_factory_snapshot")
        .execute(services.database.pool())
        .await
        .unwrap();
    let response = app
        .oneshot(json_with_token("POST", &path, body, &token, &csrf))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(services.worker_task_manager.active_count(), 0);
    services.database.close().await;
}

#[tokio::test]
async fn explicit_repricing_uses_configured_snapshot_and_preserves_known_legacy_and_foreign_rows() {
    let data = tempfile::tempdir().unwrap();
    std::fs::write(data.path().join("model-pricing.json"), r#"{"version":1,"source":"fixture prices only","models":{"fixture-model":{"input_usd_per_million":2,"output_usd_per_million":4,"input_includes_cached_tokens":true,"cached_read_usd_per_million":0.1}}}"#).unwrap();
    let db = aionui_db::init_database_memory().await.unwrap();
    let config = aionui_app::AppConfig {
        data_dir: data.path().into(),
        work_dir: data.path().into(),
        ..Default::default()
    };
    let services = aionui_app::AppServices::from_config(db, &config).await.unwrap();
    let mut app = aionui_app::create_router(&services).await.unwrap();
    let (token, csrf) = setup_and_login(&mut app, &services, "admin", "StrongP@ss1").await;
    seed_handoff_assistant(&app, &services, &token, &csrf).await;
    let workspace = tempfile::tempdir().unwrap();
    let id = confirmed_run(&app, &token, &csrf, workspace.path().to_str().unwrap()).await;
    let response = app.clone().oneshot(json_with_token("POST", &format!("/api/product-factory/runs/{id}/handoff"),
        json!({"expected_revision":1,"agents":[{"name":"Developer","role":"lead","assistant_id":"factory-e2e-assistant","model":"claude"}]}), &token,&csrf)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = common::body_json(response).await;
    let task_id = format!("{}-data-1", body["data"]["team_id"].as_str().unwrap());
    let owner: String = sqlx::query_scalar("SELECT id FROM users WHERE username='admin'")
        .fetch_one(services.database.pool())
        .await
        .unwrap();
    for (name, user, model, cache, cost) in [
        ("measured", owner.as_str(), "fixture-model", Some(80_i64), None),
        ("legacy", owner.as_str(), "fixture-model", None, None),
        ("unpriced", owner.as_str(), "missing-model", Some(80), None),
        ("known", owner.as_str(), "fixture-model", Some(80), Some(0.9)),
        ("foreign", "another-user", "fixture-model", Some(80), None),
    ] {
        sqlx::query("INSERT INTO agent_usage (id,user_id,task_id,model,input_tokens,output_tokens,cost_est,cached_read_tokens,cached_write_tokens,conversation_id,turn_id,created_at,cost_source) VALUES (?,?,?,?,100,50,?,?,?,'fixture-conversation',?,1,?)")
            .bind(name).bind(user).bind(&task_id).bind(model).bind(cost).bind(cache).bind(cache.map(|_|0_i64)).bind(name)
            .bind(cost.map(|_| "provider_report")).execute(services.database.pool()).await.unwrap();
    }
    let path = format!("/api/product-factory/runs/{id}/cost-report/reprice");
    let no_csrf = app
        .clone()
        .oneshot(
            Request::post(&path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);
    let response = app
        .clone()
        .oneshot(json_with_token("POST", &path, json!({}), &token, &csrf))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = common::body_json(response).await;
    assert_eq!(result["data"]["updated_count"], 1);
    assert_eq!(result["data"]["skipped_count"], 2);
    let rows: Vec<aionui_db::models::AgentUsageRow> = sqlx::query_as("SELECT * FROM agent_usage ORDER BY id")
        .fetch_all(services.database.pool())
        .await
        .unwrap();
    let measured = rows.iter().find(|row| row.id == "measured").unwrap();
    assert_eq!(measured.cost_est, Some(0.000248));
    assert_eq!(measured.cost_source.as_deref(), Some("configured_estimate"));
    assert!(
        measured
            .pricing_snapshot
            .as_ref()
            .unwrap()
            .contains("fixture prices only")
    );
    for name in ["legacy", "unpriced", "foreign"] {
        assert_eq!(rows.iter().find(|row| row.id == name).unwrap().cost_est, None);
    }
    assert_eq!(rows.iter().find(|row| row.id == "known").unwrap().cost_est, Some(0.9));
    let again = app
        .oneshot(json_with_token("POST", &path, json!({}), &token, &csrf))
        .await
        .unwrap();
    assert_eq!(common::body_json(again).await["data"]["updated_count"], 0);
    services.database.close().await;
}
