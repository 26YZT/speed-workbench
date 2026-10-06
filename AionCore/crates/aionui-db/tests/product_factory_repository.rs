use aionui_db::models::{TeamRow, TeamTaskRow};
use aionui_db::{
    DbError, IProductFactoryRepository, ITeamRepository, ProductFactoryRunRow, SqliteProductFactoryRepository,
    SqliteTeamRepository, init_database_memory,
};

fn handoff_rows() -> (ProductFactoryRunRow, TeamRow, Vec<TeamTaskRow>) {
    let mut run = run("handoff-run", "owner-a", "Product", 100);
    run.status = "task_draft_ready".into();
    run.task_draft_json = Some(r#"{"confirmed":true,"revision":2}"#.into());
    let team = TeamRow {
        id: "factory-handoff-run-r2".into(),
        user_id: run.user_id.clone(),
        name: run.name.clone(),
        workspace: run.workspace_path.clone(),
        workspace_mode: "shared".into(),
        agents: "[]".into(),
        lead_agent_id: None,
        session_mode: None,
        agents_version: "1.0.1".into(),
        created_at: 200,
        updated_at: 200,
        project_id: None,
        folder_id: None,
    };
    let tasks = vec![TeamTaskRow {
        id: "factory-task-1".into(),
        team_id: team.id.clone(),
        subject: "Build".into(),
        description: Some("Build the product".into()),
        status: "pending".into(),
        owner: None,
        blocked_by: "[]".into(),
        blocks: "[]".into(),
        metadata: None,
        created_at: 200,
        updated_at: 200,
    }];
    (run, team, tasks)
}

#[tokio::test]
async fn handoff_commits_team_graph_budget_and_run_binding_together() {
    let (db, repo) = setup().await;
    let (run, team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    repo.complete_handoff(&run, &team, &tasks).await.unwrap();
    let stored = repo.get_run(&run.user_id, &run.id).await.unwrap().unwrap();
    assert_eq!(stored.status, "handed_off");
    assert_eq!(stored.team_id.as_deref(), Some(team.id.as_str()));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM team_tasks WHERE team_id = ?")
        .bind(&team.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let budget: f64 = sqlx::query_scalar("SELECT limit_usd FROM team_budgets WHERE team_id = ?")
        .bind(&team.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(budget, 10.0);
}

#[tokio::test]
async fn execution_reservation_is_owner_scoped_and_cannot_be_replayed() {
    let (_db, repo) = setup().await;
    let (run, team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    repo.complete_handoff(&run, &team, &tasks).await.unwrap();
    let stored = repo.get_run(&run.user_id, &run.id).await.unwrap().unwrap();
    assert!(repo.reserve_execution(&stored, &team, &tasks[0], 300).await.unwrap());
    assert!(!repo.reserve_execution(&stored, &team, &tasks[0], 301).await.unwrap());
    let receipt = repo.get_execution("owner-a", &run.id).await.unwrap().unwrap();
    assert_eq!(receipt.state, "pending");
    assert_eq!(receipt.task_id, "factory-task-1");
    assert!(repo.get_execution("owner-b", &run.id).await.unwrap().is_none());
    // A pending dispatch is not proof of a running Agent.
    assert_eq!(
        repo.get_run("owner-a", &run.id).await.unwrap().unwrap().status,
        "handed_off"
    );
}

#[tokio::test]
async fn execution_reservation_rejects_a_changed_or_already_started_task() {
    let (db, repo) = setup().await;
    let (run, team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    repo.complete_handoff(&run, &team, &tasks).await.unwrap();
    let stored = repo.get_run(&run.user_id, &run.id).await.unwrap().unwrap();
    sqlx::query("UPDATE team_tasks SET status = 'in_progress' WHERE id = ?")
        .bind(&tasks[0].id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        repo.reserve_execution(&stored, &team, &tasks[0], 300).await,
        Err(DbError::Conflict(_))
    ));
    assert!(repo.get_execution("owner-a", &run.id).await.unwrap().is_none());
}

#[tokio::test]
async fn execution_receipt_is_durable_and_only_enqueued_teams_are_restored() {
    let (db, repo) = setup().await;
    let (run, team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    repo.complete_handoff(&run, &team, &tasks).await.unwrap();
    let stored = repo.get_run(&run.user_id, &run.id).await.unwrap().unwrap();
    repo.reserve_execution(&stored, &team, &tasks[0], 300).await.unwrap();
    let teams = SqliteTeamRepository::new(db.pool().clone());
    assert!(teams.list_teams_for_restore().await.unwrap().is_empty());
    repo.finish_execution("owner-a", &run.id, "enqueued", Some("mail-1"), Some("turn-1"), 301)
        .await
        .unwrap();
    let reopened = SqliteProductFactoryRepository::new(db.pool().clone());
    let receipt = reopened.get_execution("owner-a", &run.id).await.unwrap().unwrap();
    assert_eq!(receipt.message_id.as_deref(), Some("mail-1"));
    assert_eq!(teams.list_teams_for_restore().await.unwrap().len(), 1);
    assert!(matches!(
        repo.finish_execution("owner-a", &run.id, "uncertain", None, None, 302)
            .await,
        Err(DbError::Conflict(_))
    ));
}

#[tokio::test]
async fn failed_task_insert_rolls_back_team_budget_and_run_binding() {
    let (db, repo) = setup().await;
    let (run, team, mut tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    tasks[0].status = "invalid-status".into();
    assert!(matches!(
        repo.complete_handoff(&run, &team, &tasks).await,
        Err(DbError::Query(_))
    ));
    assert_eq!(repo.get_run(&run.user_id, &run.id).await.unwrap().unwrap(), run);
    for table in ["teams", "team_tasks", "team_budgets"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "orphan rows in {table}");
    }
}

#[tokio::test]
async fn stale_snapshot_cannot_commit_a_handoff() {
    let (_db, repo) = setup().await;
    let (run, team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    let mut newer = run.clone();
    newer.task_draft_json = Some(r#"{"confirmed":true,"revision":3}"#.into());
    newer.plan_revision += 1;
    repo.update_run(&newer).await.unwrap();
    assert!(matches!(
        repo.complete_handoff(&run, &team, &tasks).await,
        Err(DbError::Conflict(_))
    ));
    assert!(
        repo.get_run(&run.user_id, &run.id)
            .await
            .unwrap()
            .unwrap()
            .team_id
            .is_none()
    );
}

#[tokio::test]
async fn another_owner_cannot_bind_a_team_or_import_tasks() {
    let (_db, repo) = setup().await;
    let (run, mut team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    team.user_id = "owner-b".into();
    assert!(matches!(
        repo.complete_handoff(&run, &team, &tasks).await,
        Err(DbError::Conflict(_))
    ));
    assert_eq!(repo.get_run(&run.user_id, &run.id).await.unwrap().unwrap(), run);
}

#[tokio::test]
async fn a_second_commit_cannot_create_another_graph() {
    let (db, repo) = setup().await;
    let (run, team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    repo.complete_handoff(&run, &team, &tasks).await.unwrap();
    assert!(matches!(
        repo.complete_handoff(&run, &team, &tasks).await,
        Err(DbError::Conflict(_))
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM teams")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn an_old_workflow_save_cannot_overwrite_a_committed_handoff() {
    let (_db, repo) = setup().await;
    let (run, team, tasks) = handoff_rows();
    repo.create_run(&run).await.unwrap();
    repo.complete_handoff(&run, &team, &tasks).await.unwrap();
    assert!(matches!(repo.update_run(&run).await, Err(DbError::Conflict(_))));
    let stored = repo.get_run(&run.user_id, &run.id).await.unwrap().unwrap();
    assert_eq!(stored.team_id.as_deref(), Some(team.id.as_str()));
    assert_eq!(stored.status, "handed_off");
}

#[tokio::test]
async fn preparation_cleanup_lookup_is_owner_and_attempt_scoped() {
    let (db, repo) = setup().await;
    for user in ["owner-a", "owner-b"] {
        sqlx::query("INSERT INTO users (id, username, password_hash, created_at, updated_at) VALUES (?, ?, '', 0, 0)")
            .bind(user)
            .bind(user)
            .execute(db.pool())
            .await
            .unwrap();
    }
    for (id, user, marker) in [
        ("ours", "owner-a", "attempt-a"),
        ("other-attempt", "owner-a", "attempt-b"),
        ("foreign", "owner-b", "attempt-a"),
    ] {
        sqlx::query("INSERT INTO conversations (id, user_id, name, type, extra, created_at, updated_at) VALUES (?, ?, 'Member', 'acp', ?, 0, 0)")
            .bind(id).bind(user).bind(serde_json::json!({"product_factory_preparation_id":marker}).to_string())
            .execute(db.pool()).await.unwrap();
    }
    assert_eq!(
        repo.list_prepared_conversation_ids("owner-a", "attempt-a")
            .await
            .unwrap(),
        vec!["ours"]
    );
    assert!(
        repo.list_prepared_conversation_ids("owner-a", "unknown")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn handoff_team_is_not_auto_restored_as_a_runtime_on_server_restart() {
    let (db, factory) = setup().await;
    let (run, team, tasks) = handoff_rows();
    factory.create_run(&run).await.unwrap();
    factory.complete_handoff(&run, &team, &tasks).await.unwrap();
    let teams = SqliteTeamRepository::new(db.pool().clone());
    let mut ordinary = team.clone();
    ordinary.id = "ordinary-team".into();
    teams.create_team(&ordinary).await.unwrap();
    let restored = teams.list_teams_for_restore().await.unwrap();
    assert_eq!(
        restored.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        vec!["ordinary-team"]
    );
    // A later explicit execution stage may transition the run to `running`;
    // only then does the normal Team startup restore apply.
    sqlx::query("UPDATE product_factory_runs SET status = 'running' WHERE id = ?")
        .bind(&run.id)
        .execute(db.pool())
        .await
        .unwrap();
    let restored = teams.list_teams_for_restore().await.unwrap();
    assert_eq!(restored.len(), 2);
}

fn run(id: &str, user_id: &str, name: &str, updated_at: i64) -> ProductFactoryRunRow {
    ProductFactoryRunRow {
        id: id.to_owned(),
        plan_revision: 1,
        user_id: user_id.to_owned(),
        name: name.to_owned(),
        idea: format!("Idea for {name}"),
        target_user: String::new(),
        problem: String::new(),
        expected_output: String::new(),
        workspace_path: format!("/tmp/{id}"),
        budget_usd: Some(10.0),
        status: "draft".to_owned(),
        interview_json: None,
        blueprint_json: None,
        task_draft_json: None,
        team_id: None,
        created_at: updated_at,
        updated_at,
    }
}

async fn setup() -> (aionui_db::Database, SqliteProductFactoryRepository) {
    let db = init_database_memory().await.expect("in-memory database");
    let repo = SqliteProductFactoryRepository::new(db.pool().clone());
    (db, repo)
}

#[tokio::test]
async fn create_get_and_list_runs_are_owner_scoped_and_recent_first() {
    let (_db, repo) = setup().await;
    repo.create_run(&run("run-old", "owner-a", "Old", 100)).await.unwrap();
    repo.create_run(&run("run-new", "owner-a", "New", 200)).await.unwrap();
    repo.create_run(&run("run-private", "owner-b", "Private", 300))
        .await
        .unwrap();

    assert_eq!(repo.get_run("owner-a", "run-new").await.unwrap().unwrap().name, "New");
    assert!(repo.get_run("owner-b", "run-new").await.unwrap().is_none());
    assert_eq!(
        repo.list_runs("owner-a")
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec!["run-new", "run-old"]
    );
}

#[tokio::test]
async fn updating_a_run_persists_workflow_artifacts_without_changing_owner() {
    let (_db, repo) = setup().await;
    let original = run("run-1", "owner-a", "Original", 100);
    repo.create_run(&original).await.unwrap();

    let mut updated = original.clone();
    updated.name = "Updated".to_owned();
    updated.status = "blueprint_ready".to_owned();
    updated.interview_json = Some(r#"[{"question":"Who?","answer":"PMs"}]"#.to_owned());
    updated.blueprint_json = Some(r#"{"goals":["Faster research"]}"#.to_owned());
    updated.task_draft_json = Some(r#"[{"title":"Build UI"}]"#.to_owned());
    updated.updated_at = 200;
    updated.plan_revision += 1;
    repo.update_run(&updated).await.unwrap();

    let stored = repo.get_run("owner-a", "run-1").await.unwrap().unwrap();
    assert_eq!(stored.name, "Updated");
    assert_eq!(stored.status, "blueprint_ready");
    assert_eq!(stored.interview_json, updated.interview_json);
    assert_eq!(stored.blueprint_json, updated.blueprint_json);
    assert_eq!(stored.task_draft_json, updated.task_draft_json);
    assert_eq!(stored.user_id, "owner-a");
}

#[tokio::test]
async fn another_owner_cannot_update_or_delete_a_run() {
    let (_db, repo) = setup().await;
    let original = run("run-1", "owner-a", "Original", 100);
    repo.create_run(&original).await.unwrap();

    let mut takeover = original.clone();
    takeover.user_id = "owner-b".to_owned();
    takeover.name = "Taken over".to_owned();
    assert!(matches!(repo.update_run(&takeover).await, Err(DbError::NotFound(id)) if id == "run-1"));
    assert!(matches!(repo.delete_run("owner-b", "run-1").await, Err(DbError::NotFound(id)) if id == "run-1"));

    assert_eq!(
        repo.get_run("owner-a", "run-1").await.unwrap().unwrap().name,
        "Original"
    );
}

#[tokio::test]
async fn deleting_an_owned_run_removes_it() {
    let (_db, repo) = setup().await;
    repo.create_run(&run("run-1", "owner-a", "Temporary", 100))
        .await
        .unwrap();

    repo.delete_run("owner-a", "run-1").await.unwrap();

    assert!(repo.get_run("owner-a", "run-1").await.unwrap().is_none());
}
