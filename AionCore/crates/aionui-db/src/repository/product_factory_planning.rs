use crate::models::ProductFactoryPlanningRow;
use crate::{DbError, ProductFactoryRunRow};

#[async_trait::async_trait]
pub trait IProductFactoryPlanningRepository: Send + Sync {
    async fn get_by_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ProductFactoryPlanningRow>, DbError>;
    async fn get(&self, user_id: &str, run_id: &str, id: &str) -> Result<Option<ProductFactoryPlanningRow>, DbError>;
    async fn get_by_key(
        &self,
        user_id: &str,
        run_id: &str,
        key: &str,
    ) -> Result<Option<ProductFactoryPlanningRow>, DbError>;
    async fn list(&self, user_id: &str, run_id: &str) -> Result<Vec<ProductFactoryPlanningRow>, DbError>;
    async fn reserve(&self, run: &ProductFactoryRunRow, attempt: &ProductFactoryPlanningRow) -> Result<bool, DbError>;
    async fn claim_preparing(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError>;
    async fn bind_conversation(
        &self,
        user_id: &str,
        id: &str,
        conversation_id: &str,
        snapshot_json: &str,
        now: i64,
    ) -> Result<bool, DbError>;
    async fn mark_running(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError>;
    async fn bind_app_turn(&self, user_id: &str, id: &str, app_turn_id: &str, now: i64) -> Result<bool, DbError>;
    async fn finish(
        &self,
        user_id: &str,
        id: &str,
        state: &str,
        result_json: Option<&str>,
        error_code: Option<&str>,
        now: i64,
    ) -> Result<bool, DbError>;
    async fn cancel(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError>;
    async fn mark_uncertain(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError>;
    async fn apply_candidate(
        &self,
        run: &ProductFactoryRunRow,
        attempt_id: &str,
        expected_plan_revision: i64,
        now: i64,
    ) -> Result<bool, DbError>;
    async fn run_for_team(&self, user_id: &str, team_id: &str) -> Result<Option<String>, DbError>;
}
