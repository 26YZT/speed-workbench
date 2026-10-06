use crate::error::DbError;

/// Persistence contract for an optional per-team USD budget.
#[async_trait::async_trait]
pub trait ITeamBudgetRepository: Send + Sync {
    async fn get_limit_usd(&self, user_id: &str, team_id: &str) -> Result<Option<f64>, DbError>;

    async fn set_limit_usd(
        &self,
        user_id: &str,
        team_id: &str,
        limit_usd: Option<f64>,
        updated_at: i64,
    ) -> Result<(), DbError>;
}
