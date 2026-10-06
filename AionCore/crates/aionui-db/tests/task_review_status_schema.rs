use aionui_common::now_ms;
use aionui_db::models::TeamRow;
use aionui_db::{ITeamRepository, SqliteTeamRepository, init_database_memory};

async fn migrated_pool_with_team() -> sqlx::SqlitePool {
    let db = init_database_memory().await.expect("in-memory database");
    let pool = db.pool().clone();
    let repo = SqliteTeamRepository::new(pool.clone());
    let now = now_ms();
    repo.create_team(&TeamRow {
        id: "team-review".to_owned(),
        user_id: "system_default_user".to_owned(),
        name: "Review Team".to_owned(),
        workspace: String::new(),
        workspace_mode: "shared".to_owned(),
        agents: "[]".to_owned(),
        lead_agent_id: None,
        session_mode: None,
        agents_version: "1.0.1".to_owned(),
        created_at: now,
        updated_at: now,
        project_id: None,
        folder_id: None,
    })
    .await
    .expect("parent team");
    pool
}

#[tokio::test]
async fn task_status_accepts_in_review_and_rejects_unknown_values() {
    let pool = migrated_pool_with_team().await;

    sqlx::query(
        "INSERT INTO team_tasks (id, team_id, subject, status, created_at, updated_at)
         VALUES ('task-review', 'team-review', 'Awaiting approval', 'in_review', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("in_review must be a valid persisted task status");

    let status: String = sqlx::query_scalar("SELECT status FROM team_tasks WHERE id = 'task-review'")
        .fetch_one(&pool)
        .await
        .expect("persisted task");
    assert_eq!(status, "in_review");

    let invalid = sqlx::query(
        "INSERT INTO team_tasks (id, team_id, subject, status, created_at, updated_at)
         VALUES ('task-invalid', 'team-review', 'Invalid', 'shipping', 1, 1)",
    )
    .execute(&pool)
    .await;
    assert!(invalid.is_err(), "unknown task statuses must remain rejected");
}

#[tokio::test]
async fn task_status_migration_preserves_team_parent_integrity() {
    let pool = migrated_pool_with_team().await;

    let orphan = sqlx::query(
        "INSERT INTO team_tasks (id, team_id, subject, status, created_at, updated_at)
         VALUES ('task-orphan', 'missing-team', 'Orphan', 'in_review', 1, 1)",
    )
    .execute(&pool)
    .await
    .expect_err("team_tasks parent trigger must survive the table rebuild");

    assert!(
        orphan
            .to_string()
            .contains("team_tasks.team_id must reference teams.id"),
        "unexpected parent-integrity error: {orphan}"
    );
}
