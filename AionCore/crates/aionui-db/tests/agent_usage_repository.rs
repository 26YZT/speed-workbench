use aionui_db::models::AgentUsageRow;
use aionui_db::{IAgentUsageRepository, SqliteAgentUsageRepository, init_database_memory};

#[tokio::test]
async fn records_and_lists_usage_scoped_to_the_owner_and_task() {
    let database = init_database_memory().await.unwrap();
    let repository = SqliteAgentUsageRepository::new(database.pool().clone());

    repository
        .record_usage(&AgentUsageRow {
            id: "usage-1".into(),
            user_id: "system_default_user".into(),
            task_id: Some("task-1".into()),
            agent_id: Some("agent-1".into()),
            model: Some("model-a".into()),
            input_tokens: 120,
            output_tokens: 30,
            cost_est: Some(0.0042),
            cached_read_tokens: None,
            cached_write_tokens: None,
            cost_source: None,
            pricing_snapshot: None,
            cost_unknown_reason: None,
            conversation_id: "conversation-1".into(),
            turn_id: "turn-1".into(),
            created_at: 10,
            attempt_id: "legacy".into(),
            team_id: None,
        })
        .await
        .unwrap();

    let rows = repository.list_by_task("system_default_user", "task-1").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].input_tokens, 120);
    assert_eq!(rows[0].output_tokens, 30);
    assert_eq!(rows[0].cost_est, Some(0.0042));

    assert!(
        repository
            .list_by_task("other-user", "task-1")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repository
            .list_by_task("system_default_user", "other-task")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn repeated_turn_usage_is_idempotently_updated() {
    let database = init_database_memory().await.unwrap();
    let repository = SqliteAgentUsageRepository::new(database.pool().clone());
    let mut row = AgentUsageRow {
        id: "usage-first".into(),
        user_id: "system_default_user".into(),
        task_id: Some("task-1".into()),
        agent_id: Some("agent-1".into()),
        model: Some("model-a".into()),
        input_tokens: 10,
        output_tokens: 5,
        cost_est: Some(0.01),
        cached_read_tokens: None,
        cached_write_tokens: None,
        cost_source: None,
        pricing_snapshot: None,
        cost_unknown_reason: None,
        conversation_id: "conversation-1".into(),
        turn_id: "turn-1".into(),
        created_at: 10,
        attempt_id: "legacy".into(),
        team_id: None,
    };
    repository.record_usage(&row).await.unwrap();

    row.input_tokens = 20;
    row.output_tokens = 7;
    row.cost_est = Some(0.02);
    row.created_at = 20;
    repository.record_usage(&row).await.unwrap();

    let rows = repository.list_by_task("system_default_user", "task-1").await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!((rows[0].input_tokens, rows[0].output_tokens), (20, 7));
    assert_eq!(rows[0].cost_est, Some(0.02));
}

#[tokio::test]
async fn migration_preserves_legacy_unknown_buckets_and_known_amounts() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::raw_sql(include_str!("../migrations/045_agent_usage.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO agent_usage (id,user_id,input_tokens,output_tokens,cost_est,conversation_id,turn_id,created_at) VALUES ('old','u',100,50,NULL,'c','t',1), ('known','u',100,50,0.2,'c2','t2',1)").execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("../migrations/049_agent_usage_provenance.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE teams(id TEXT,user_id TEXT); CREATE TABLE team_tasks(id TEXT,team_id TEXT);")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/050_agent_usage_attempts.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let old = sqlx::query_as::<_, AgentUsageRow>("SELECT * FROM agent_usage WHERE id='old'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(old.cached_read_tokens, None);
    assert_eq!(old.cached_write_tokens, None);
    assert_eq!(old.cost_est, None);
    assert_eq!(
        old.cost_unknown_reason.as_deref(),
        Some("legacy_cache_breakdown_missing")
    );
    let known = sqlx::query_as::<_, AgentUsageRow>("SELECT * FROM agent_usage WHERE id='known'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(known.cost_est, Some(0.2));
    assert_eq!(known.cost_source.as_deref(), Some("legacy_unspecified"));
}
