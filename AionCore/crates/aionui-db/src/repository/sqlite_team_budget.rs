use sqlx::SqlitePool;

use crate::error::DbError;
use crate::repository::team_budget::ITeamBudgetRepository;

/// SQLite-backed per-team budget storage.
#[derive(Clone, Debug)]
pub struct SqliteTeamBudgetRepository {
    pool: SqlitePool,
}

impl SqliteTeamBudgetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ITeamBudgetRepository for SqliteTeamBudgetRepository {
    async fn get_limit_usd(&self, user_id: &str, team_id: &str) -> Result<Option<f64>, DbError> {
        Ok(
            sqlx::query_scalar("SELECT limit_usd FROM team_budgets WHERE user_id = ? AND team_id = ?")
                .bind(user_id)
                .bind(team_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    async fn set_limit_usd(
        &self,
        user_id: &str,
        team_id: &str,
        limit_usd: Option<f64>,
        updated_at: i64,
    ) -> Result<(), DbError> {
        match limit_usd {
            Some(limit_usd) => {
                let result = sqlx::query(
                    "INSERT INTO team_budgets (team_id, user_id, limit_usd, updated_at) \
                     VALUES (?, ?, ?, ?) \
                     ON CONFLICT(team_id) DO UPDATE SET \
                         limit_usd = excluded.limit_usd, updated_at = excluded.updated_at \
                     WHERE team_budgets.user_id = excluded.user_id",
                )
                .bind(team_id)
                .bind(user_id)
                .bind(limit_usd)
                .bind(updated_at)
                .execute(&self.pool)
                .await?;
                if result.rows_affected() == 0 {
                    return Err(DbError::NotFound(team_id.to_owned()));
                }
            }
            None => {
                sqlx::query("DELETE FROM team_budgets WHERE user_id = ? AND team_id = ?")
                    .bind(user_id)
                    .bind(team_id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }
}
