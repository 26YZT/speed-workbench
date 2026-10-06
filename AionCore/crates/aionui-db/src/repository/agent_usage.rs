use crate::error::DbError;
use crate::models::{AgentUsageCost, AgentUsageRow};

/// Persistence contract for normalized per-turn agent usage.
#[async_trait::async_trait]
pub trait IAgentUsageRepository: Send + Sync {
    /// Insert a report, or replace the existing report for the same user,
    /// conversation, and turn. Runtime adapters may emit a late correction.
    async fn record_usage(&self, row: &AgentUsageRow) -> Result<(), DbError>;

    /// Return reports for one task, newest first, scoped to the owning user.
    async fn list_by_task(&self, user_id: &str, task_id: &str) -> Result<Vec<AgentUsageRow>, DbError>;

    async fn list_by_conversation(
        &self,
        _user_id: &str,
        _conversation_id: &str,
    ) -> Result<Vec<AgentUsageRow>, DbError> {
        Err(DbError::Init("conversation usage lookup is unsupported".into()))
    }
    async fn list_unassigned_by_team(&self, _user_id: &str, _team_id: &str) -> Result<Vec<AgentUsageRow>, DbError> {
        Err(DbError::Init("team usage lookup is unsupported".into()))
    }

    /// Reserve an unknown usage record before starting an attributed turn.
    /// Never overwrite a report already received for this logical turn.
    async fn ensure_turn_admission(&self, _row: &AgentUsageRow) -> Result<(), DbError> {
        Err(DbError::Init("usage admission persistence is unsupported".into()))
    }

    /// Only the caller that knows dispatch never began may settle an untouched
    /// common admission as zero cost. Never use this for a missing runtime report.
    async fn mark_admission_not_sent(
        &self,
        _user_id: &str,
        _conversation_id: &str,
        _attempt_id: &str,
    ) -> Result<bool, DbError> {
        Err(DbError::Init("unsent admission settlement is unsupported".into()))
    }

    /// Apply an estimate only while this captured record is still unknown and
    /// its ownership, model and token counters have not changed.
    async fn apply_cost_estimate(&self, _snapshot: &AgentUsageRow, _cost: &AgentUsageCost) -> Result<bool, DbError> {
        Ok(false)
    }

    /// Preserve measured counters while invalidating costs whose cumulative
    /// boundary became uncertain. Scope every write to the owning conversation.
    async fn invalidate_billing_attempts(
        &self,
        _user_id: &str,
        _conversation_id: &str,
        _attempt_ids: &[String],
    ) -> Result<(), DbError> {
        Err(DbError::Init("billing invalidation persistence is unsupported".into()))
    }

    /// Explicitly reprice measured unknown records. Historical rows without
    /// cache buckets and existing costs are never changed. Not run at startup.
    async fn backfill_unknown_costs(
        &self,
        resolver: &(dyn for<'row> Fn(&'row AgentUsageRow) -> Option<AgentUsageCost> + Send + Sync),
    ) -> Result<u64, DbError>;
}
