use sqlx::SqlitePool;

use crate::error::DbError;
use crate::models::{AgentUsageCost, AgentUsageRow};
use crate::repository::agent_usage::IAgentUsageRepository;

/// SQLite-backed usage ledger.
#[derive(Clone, Debug)]
pub struct SqliteAgentUsageRepository {
    pool: SqlitePool,
}

impl SqliteAgentUsageRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl IAgentUsageRepository for SqliteAgentUsageRepository {
    async fn record_usage(&self, row: &AgentUsageRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO agent_usage \
                (id, user_id, task_id, agent_id, model, input_tokens, output_tokens, cost_est, \
                 conversation_id, turn_id, created_at, cached_read_tokens, cached_write_tokens, \
                 cost_source, pricing_snapshot, cost_unknown_reason, attempt_id, team_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(user_id, conversation_id, turn_id, attempt_id) DO UPDATE SET \
                task_id = excluded.task_id, \
                team_id = COALESCE(agent_usage.team_id, excluded.team_id), \
                agent_id = excluded.agent_id, \
                model = excluded.model, \
                input_tokens = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' OR excluded.cost_unknown_reason GLOB 'billing_*' THEN MAX(agent_usage.input_tokens, excluded.input_tokens) ELSE excluded.input_tokens END, \
                output_tokens = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' OR excluded.cost_unknown_reason GLOB 'billing_*' THEN MAX(agent_usage.output_tokens, excluded.output_tokens) ELSE excluded.output_tokens END, \
                cost_est = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' THEN NULL ELSE excluded.cost_est END, \
                cached_read_tokens = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' OR excluded.cost_unknown_reason GLOB 'billing_*' THEN CASE WHEN agent_usage.cached_read_tokens IS NULL THEN excluded.cached_read_tokens WHEN excluded.cached_read_tokens IS NULL THEN agent_usage.cached_read_tokens ELSE MAX(agent_usage.cached_read_tokens, excluded.cached_read_tokens) END ELSE excluded.cached_read_tokens END, \
                cached_write_tokens = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' OR excluded.cost_unknown_reason GLOB 'billing_*' THEN CASE WHEN agent_usage.cached_write_tokens IS NULL THEN excluded.cached_write_tokens WHEN excluded.cached_write_tokens IS NULL THEN agent_usage.cached_write_tokens ELSE MAX(agent_usage.cached_write_tokens, excluded.cached_write_tokens) END ELSE excluded.cached_write_tokens END, \
                cost_source = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' THEN NULL ELSE excluded.cost_source END, \
                pricing_snapshot = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' THEN NULL ELSE excluded.pricing_snapshot END, \
                cost_unknown_reason = CASE WHEN agent_usage.cost_unknown_reason GLOB 'billing_*' THEN agent_usage.cost_unknown_reason ELSE excluded.cost_unknown_reason END, \
                created_at = excluded.created_at",
        )
        .bind(&row.id)
        .bind(&row.user_id)
        .bind(&row.task_id)
        .bind(&row.agent_id)
        .bind(&row.model)
        .bind(row.input_tokens)
        .bind(row.output_tokens)
        .bind(row.cost_est)
        .bind(&row.conversation_id)
        .bind(&row.turn_id)
        .bind(row.created_at)
        .bind(row.cached_read_tokens)
        .bind(row.cached_write_tokens)
        .bind(&row.cost_source)
        .bind(&row.pricing_snapshot)
        .bind(&row.cost_unknown_reason)
        .bind(&row.attempt_id)
        .bind(&row.team_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_by_task(&self, user_id: &str, task_id: &str) -> Result<Vec<AgentUsageRow>, DbError> {
        Ok(sqlx::query_as::<_, AgentUsageRow>(
            "SELECT * FROM agent_usage \
             WHERE user_id = ? AND task_id = ? \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(user_id)
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn list_by_conversation(&self, user_id: &str, conversation_id: &str) -> Result<Vec<AgentUsageRow>, DbError> {
        Ok(sqlx::query_as::<_, AgentUsageRow>(
            "SELECT * FROM agent_usage WHERE user_id=? AND conversation_id=? ORDER BY created_at DESC,id DESC",
        )
        .bind(user_id)
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?)
    }
    async fn list_unassigned_by_team(&self, user_id: &str, team_id: &str) -> Result<Vec<AgentUsageRow>, DbError> {
        Ok(sqlx::query_as::<_,AgentUsageRow>("SELECT * FROM agent_usage WHERE user_id=? AND team_id=? AND task_id IS NULL ORDER BY created_at DESC,id DESC")
            .bind(user_id).bind(team_id).fetch_all(&self.pool).await?)
    }

    async fn ensure_turn_admission(&self, row: &AgentUsageRow) -> Result<(), DbError> {
        sqlx::query("INSERT INTO agent_usage (id,user_id,task_id,agent_id,model,input_tokens,output_tokens,cost_est,cached_read_tokens,cached_write_tokens,cost_unknown_reason,conversation_id,turn_id,created_at,attempt_id,team_id) VALUES (?,?,?,?,?,0,0,NULL,NULL,NULL,'usage_not_reported',?,?,?,?,?) ON CONFLICT(user_id,conversation_id,turn_id,attempt_id) DO NOTHING")
            .bind(&row.id).bind(&row.user_id).bind(&row.task_id).bind(&row.agent_id).bind(&row.model)
            .bind(&row.conversation_id).bind(&row.turn_id).bind(row.created_at).bind(&row.attempt_id).bind(&row.team_id).execute(&self.pool).await?;
        Ok(())
    }

    async fn mark_admission_not_sent(
        &self,
        user_id: &str,
        conversation_id: &str,
        attempt_id: &str,
    ) -> Result<bool, DbError> {
        let result=sqlx::query("UPDATE agent_usage SET cost_est=0,cost_source='not_sent',cached_read_tokens=0,cached_write_tokens=0,cost_unknown_reason=NULL WHERE user_id=? AND conversation_id=? AND attempt_id=? AND cost_unknown_reason='usage_not_reported' AND cost_est IS NULL AND input_tokens=0 AND output_tokens=0 AND cached_read_tokens IS NULL AND cached_write_tokens IS NULL AND model IS NULL AND agent_id IS NULL AND (SELECT COUNT(*) FROM agent_usage WHERE user_id=? AND conversation_id=? AND attempt_id=?)=1")
            .bind(user_id).bind(conversation_id).bind(attempt_id).bind(user_id).bind(conversation_id).bind(attempt_id).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    async fn apply_cost_estimate(&self, row: &AgentUsageRow, cost: &AgentUsageCost) -> Result<bool, DbError> {
        if row
            .cost_unknown_reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("billing_"))
            || row.cost_est.is_some()
            || row.cached_read_tokens.is_none()
            || row.cached_write_tokens.is_none()
            || !cost.cost_usd.is_finite()
            || cost.cost_usd < 0.0
            || cost.source.trim().is_empty()
            || serde_json::from_str::<serde_json::Value>(&cost.pricing_snapshot).is_err()
        {
            return Ok(false);
        }
        let result = sqlx::query("UPDATE agent_usage SET cost_est=?,cost_source=?,pricing_snapshot=?,cost_unknown_reason=NULL WHERE id=? AND user_id=? AND attempt_id=? AND task_id IS ? AND model IS ? AND cost_est IS NULL AND input_tokens=? AND output_tokens=? AND cached_read_tokens=? AND cached_write_tokens=? AND cost_unknown_reason IS ? AND COALESCE(cost_unknown_reason,'') NOT GLOB 'billing_*'")
            .bind(cost.cost_usd).bind(&cost.source).bind(&cost.pricing_snapshot)
            .bind(&row.id).bind(&row.user_id).bind(&row.attempt_id).bind(&row.task_id).bind(&row.model)
            .bind(row.input_tokens).bind(row.output_tokens).bind(row.cached_read_tokens).bind(row.cached_write_tokens).bind(&row.cost_unknown_reason)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    async fn invalidate_billing_attempts(
        &self,
        user_id: &str,
        conversation_id: &str,
        attempt_ids: &[String],
    ) -> Result<(), DbError> {
        let mut transaction = self.pool.begin().await?;
        for id in attempt_ids {
            sqlx::query("UPDATE agent_usage SET cost_est=NULL,cost_source=NULL,pricing_snapshot=NULL,cost_unknown_reason='billing_attribution_uncertain' WHERE user_id=? AND conversation_id=? AND attempt_id=? AND cost_unknown_reason IS NOT 'billing_attribution_uncertain'")
                .bind(user_id).bind(conversation_id).bind(id).execute(&mut *transaction).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn backfill_unknown_costs(
        &self,
        resolver: &(dyn for<'row> Fn(&'row AgentUsageRow) -> Option<AgentUsageCost> + Send + Sync),
    ) -> Result<u64, DbError> {
        let rows = sqlx::query_as::<_, AgentUsageRow>(
            "SELECT * FROM agent_usage WHERE cost_est IS NULL AND cached_read_tokens IS NOT NULL AND cached_write_tokens IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut updated = 0_u64;
        for row in rows {
            let Some(cost) = resolver(&row) else {
                continue;
            };
            updated += u64::from(self.apply_cost_estimate(&row, &cost).await?);
        }
        Ok(updated)
    }
}
