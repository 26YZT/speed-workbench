use crate::error::DbError;
use crate::models::{ProductFactoryExecutionRow, ProductFactoryRunRow, TeamRow, TeamTaskRow};

/// Persistence contract for owner-scoped Product Factory workflow runs.
#[async_trait::async_trait]
pub trait IProductFactoryRepository: Send + Sync {
    async fn create_run(&self, run: &ProductFactoryRunRow) -> Result<(), DbError>;

    async fn get_run(&self, user_id: &str, run_id: &str) -> Result<Option<ProductFactoryRunRow>, DbError>;

    async fn list_runs(&self, user_id: &str) -> Result<Vec<ProductFactoryRunRow>, DbError>;

    async fn update_run(&self, run: &ProductFactoryRunRow) -> Result<(), DbError>;

    async fn update_task_draft_if_current(
        &self,
        run: &ProductFactoryRunRow,
        expected_revision: Option<u64>,
        expected_task_draft_json: Option<&str>,
        expected_status: &str,
    ) -> Result<(), DbError>;

    async fn delete_run(&self, user_id: &str, run_id: &str) -> Result<(), DbError>;

    /// Commit one prepared Team, task graph, budget and run binding atomically.
    /// The supplied run is the exact owner-scoped snapshot validated by the service.
    async fn complete_handoff(
        &self,
        run: &ProductFactoryRunRow,
        team: &TeamRow,
        tasks: &[TeamTaskRow],
    ) -> Result<(), DbError>;

    async fn get_handoff_team(&self, user_id: &str, team_id: &str) -> Result<Option<TeamRow>, DbError>;

    async fn get_handoff_task(
        &self,
        user_id: &str,
        team_id: &str,
        task_id: &str,
    ) -> Result<Option<TeamTaskRow>, DbError>;

    async fn get_execution(&self, user_id: &str, run_id: &str) -> Result<Option<ProductFactoryExecutionRow>, DbError>;

    /// Atomically reserve the only dispatch attempt against validated snapshots.
    /// False means an existing receipt must be read back, not dispatched again.
    async fn reserve_execution(
        &self,
        run: &ProductFactoryRunRow,
        team: &TeamRow,
        task: &TeamTaskRow,
        now: i64,
    ) -> Result<bool, DbError>;

    async fn finish_execution(
        &self,
        user_id: &str,
        run_id: &str,
        state: &str,
        message_id: Option<&str>,
        team_run_id: Option<&str>,
        now: i64,
    ) -> Result<(), DbError>;

    /// Advance the single current execution receipt to the next task after the
    /// previous task has been human-approved. The compare-and-swap predicate
    /// prevents two browser windows from dispatching the same next task.
    async fn advance_execution(
        &self,
        user_id: &str,
        run_id: &str,
        team_id: &str,
        previous_task_id: &str,
        next_task: &TeamTaskRow,
        now: i64,
    ) -> Result<bool, DbError>;

    /// Reserve a deliberate user-requested retry of the same task after review
    /// feedback. A pending receipt is never retried automatically.
    async fn retry_execution(
        &self,
        user_id: &str,
        run_id: &str,
        team_id: &str,
        task_id: &str,
        now: i64,
    ) -> Result<bool, DbError>;

    /// Move an execution-owned Product Factory run through its runtime states.
    /// Generic status writes remain blocked by the service and cannot call this.
    async fn set_execution_run_status(
        &self,
        user_id: &str,
        run_id: &str,
        expected_status: &str,
        next_status: &str,
        now: i64,
    ) -> Result<bool, DbError>;

    /// Read only this owner's conversations minted by one failed preparation.
    async fn list_prepared_conversation_ids(&self, user_id: &str, preparation_id: &str)
    -> Result<Vec<String>, DbError>;
}
