use crate::TeamError;
use aionui_api_types::TeamTaskUsageResponse;

#[derive(Default)]
pub struct TeamBudgetContribution {
    pub usage: Vec<TeamTaskUsageResponse>,
    pub cost_unknown: bool,
}

/// Optional application-owned costs belonging to a Team's full run budget.
/// The domain must not know Product Factory or any sibling service.
#[async_trait::async_trait]
pub trait TeamBudgetContributionPort: Send + Sync {
    async fn get_contribution(&self, user_id: &str, team_id: &str)
    -> Result<Option<TeamBudgetContribution>, TeamError>;
}
