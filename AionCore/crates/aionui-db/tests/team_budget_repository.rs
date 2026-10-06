use aionui_db::{DbError, ITeamBudgetRepository, SqliteTeamBudgetRepository, init_database_memory};

async fn setup() -> (aionui_db::Database, SqliteTeamBudgetRepository) {
    let db = init_database_memory().await.expect("in-memory database");
    let repo = SqliteTeamBudgetRepository::new(db.pool().clone());
    (db, repo)
}

#[tokio::test]
async fn set_get_and_clear_budget_are_owner_scoped() {
    let (_db, repo) = setup().await;

    repo.set_limit_usd("owner-a", "team-a", Some(1.25), 100)
        .await
        .expect("owner A can set its team budget");

    assert_eq!(repo.get_limit_usd("owner-a", "team-a").await.unwrap(), Some(1.25));
    assert_eq!(repo.get_limit_usd("owner-b", "team-a").await.unwrap(), None);

    repo.set_limit_usd("owner-a", "team-a", None, 200)
        .await
        .expect("owner A can clear its team budget");
    assert_eq!(repo.get_limit_usd("owner-a", "team-a").await.unwrap(), None);
}

#[tokio::test]
async fn updating_a_budget_cannot_take_over_another_users_team_row() {
    let (_db, repo) = setup().await;

    repo.set_limit_usd("owner-a", "team-a", Some(2.0), 100)
        .await
        .expect("owner A can set its team budget");

    let error = repo
        .set_limit_usd("owner-b", "team-a", Some(3.0), 200)
        .await
        .expect_err("owner B must not update owner A's team row");
    assert!(matches!(error, DbError::NotFound(team_id) if team_id == "team-a"));
    assert_eq!(repo.get_limit_usd("owner-a", "team-a").await.unwrap(), Some(2.0));
    assert_eq!(repo.get_limit_usd("owner-b", "team-a").await.unwrap(), None);
}
