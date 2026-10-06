use super::product_factory_planning::IProductFactoryPlanningRepository;
use crate::models::ProductFactoryPlanningRow;
use crate::{DbError, ProductFactoryRunRow};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SqliteProductFactoryPlanningRepository {
    pool: SqlitePool,
}
impl SqliteProductFactoryPlanningRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl IProductFactoryPlanningRepository for SqliteProductFactoryPlanningRepository {
    async fn get_by_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ProductFactoryPlanningRow>, DbError> {
        Ok(
            sqlx::query_as("SELECT * FROM product_factory_planning WHERE user_id=? AND conversation_id=?")
                .bind(user_id)
                .bind(conversation_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }
    async fn get(&self, user_id: &str, run_id: &str, id: &str) -> Result<Option<ProductFactoryPlanningRow>, DbError> {
        Ok(sqlx::query_as("SELECT attempt.* FROM product_factory_planning attempt JOIN product_factory_runs run ON run.id=attempt.run_id AND run.user_id=attempt.user_id WHERE attempt.user_id=? AND attempt.run_id=? AND attempt.id=?")
            .bind(user_id).bind(run_id).bind(id).fetch_optional(&self.pool).await?)
    }
    async fn get_by_key(
        &self,
        user_id: &str,
        run_id: &str,
        key: &str,
    ) -> Result<Option<ProductFactoryPlanningRow>, DbError> {
        Ok(
            sqlx::query_as("SELECT * FROM product_factory_planning WHERE user_id=? AND run_id=? AND idempotency_key=?")
                .bind(user_id)
                .bind(run_id)
                .bind(key)
                .fetch_optional(&self.pool)
                .await?,
        )
    }
    async fn list(&self, user_id: &str, run_id: &str) -> Result<Vec<ProductFactoryPlanningRow>, DbError> {
        Ok(sqlx::query_as(
            "SELECT * FROM product_factory_planning WHERE user_id=? AND run_id=? ORDER BY created_at DESC,id DESC",
        )
        .bind(user_id)
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?)
    }
    async fn reserve(&self, run: &ProductFactoryRunRow, row: &ProductFactoryPlanningRow) -> Result<bool, DbError> {
        if run.user_id != row.user_id || run.id != row.run_id || run.plan_revision != row.input_plan_revision {
            return Err(DbError::Conflict(
                "planning snapshot ownership or revision mismatch".into(),
            ));
        }
        let result = sqlx::query("INSERT INTO product_factory_planning (id,user_id,run_id,phase,input_plan_revision,input_hash,idempotency_key,assistant_id,model,state,created_at,updated_at) SELECT ?,?,?,?,?,?,?,?,?,'reserved',?,? WHERE EXISTS(SELECT 1 FROM product_factory_runs WHERE id=? AND user_id=? AND plan_revision=? AND team_id IS NULL) ON CONFLICT DO NOTHING")
            .bind(&row.id).bind(&row.user_id).bind(&row.run_id).bind(&row.phase).bind(row.input_plan_revision)
            .bind(&row.input_hash).bind(&row.idempotency_key).bind(&row.assistant_id).bind(&row.model)
            .bind(row.created_at).bind(row.updated_at).bind(&run.id).bind(&run.user_id).bind(run.plan_revision)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }
    async fn claim_preparing(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError> {
        Ok(sqlx::query("UPDATE product_factory_planning SET state='preparing',updated_at=? WHERE user_id=? AND id=? AND state='reserved'")
            .bind(now).bind(user_id).bind(id).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn bind_conversation(
        &self,
        user_id: &str,
        id: &str,
        conversation_id: &str,
        snapshot_json: &str,
        now: i64,
    ) -> Result<bool, DbError> {
        Ok(sqlx::query("UPDATE product_factory_planning SET conversation_id=?,assistant_snapshot_json=?,updated_at=? WHERE user_id=? AND id=? AND state IN ('preparing','cancelled') AND conversation_id IS NULL")
            .bind(conversation_id).bind(snapshot_json).bind(now).bind(user_id).bind(id).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn mark_running(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError> {
        Ok(sqlx::query("UPDATE product_factory_planning SET state='running',started_at=?,updated_at=? WHERE user_id=? AND id=? AND state='preparing' AND conversation_id IS NOT NULL")
            .bind(now).bind(now).bind(user_id).bind(id).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn bind_app_turn(&self, user_id: &str, id: &str, app_turn_id: &str, now: i64) -> Result<bool, DbError> {
        Ok(sqlx::query("UPDATE product_factory_planning SET app_turn_id=?,updated_at=? WHERE user_id=? AND id=? AND app_turn_id IS NULL AND state IN ('running','cancelled','uncertain')")
            .bind(app_turn_id).bind(now).bind(user_id).bind(id).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn finish(
        &self,
        user_id: &str,
        id: &str,
        state: &str,
        result_json: Option<&str>,
        error_code: Option<&str>,
        now: i64,
    ) -> Result<bool, DbError> {
        if !matches!(state, "candidate_ready" | "failed" | "uncertain" | "cancelled") {
            return Err(DbError::Conflict("invalid planning terminal state".into()));
        }
        Ok(sqlx::query("UPDATE product_factory_planning SET state=?,result_json=?,error_code=?,updated_at=? WHERE user_id=? AND id=? AND (state IN ('reserved','preparing','running') OR (state='cancelled' AND ?='cancelled'))")
            .bind(state).bind(result_json).bind(error_code).bind(now).bind(user_id).bind(id).bind(state).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn cancel(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError> {
        Ok(sqlx::query("UPDATE product_factory_planning SET state='cancelled',updated_at=? WHERE user_id=? AND id=? AND state IN ('reserved','preparing','running','candidate_ready','uncertain')")
            .bind(now).bind(user_id).bind(id).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn mark_uncertain(&self, user_id: &str, id: &str, now: i64) -> Result<bool, DbError> {
        Ok(sqlx::query("UPDATE product_factory_planning SET state='uncertain',error_code='PLANNING_RESTART_UNCERTAIN',updated_at=? WHERE user_id=? AND id=? AND state IN ('reserved','preparing','running')")
            .bind(now).bind(user_id).bind(id).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn apply_candidate(
        &self,
        run: &ProductFactoryRunRow,
        attempt_id: &str,
        expected_plan_revision: i64,
        now: i64,
    ) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query("UPDATE product_factory_runs SET interview_json=?,blueprint_json=?,task_draft_json=?,status=?,updated_at=?,plan_revision=plan_revision+1 WHERE id=? AND user_id=? AND plan_revision=? AND team_id IS NULL AND EXISTS(SELECT 1 FROM product_factory_planning WHERE id=? AND run_id=? AND user_id=? AND state='candidate_ready' AND input_plan_revision=?)")
            .bind(&run.interview_json).bind(&run.blueprint_json).bind(&run.task_draft_json).bind(&run.status).bind(now)
            .bind(&run.id).bind(&run.user_id).bind(expected_plan_revision).bind(attempt_id).bind(&run.id).bind(&run.user_id).bind(expected_plan_revision)
            .execute(&mut *tx).await?;
        if result.rows_affected() != 1 {
            tx.rollback().await?;
            return Ok(false);
        }
        let applied = sqlx::query("UPDATE product_factory_planning SET state='applied',updated_at=? WHERE id=? AND user_id=? AND state='candidate_ready'")
            .bind(now).bind(attempt_id).bind(&run.user_id).execute(&mut *tx).await?;
        if applied.rows_affected() != 1 {
            tx.rollback().await?;
            return Ok(false);
        }
        tx.commit().await?;
        Ok(true)
    }
    async fn run_for_team(&self, user_id: &str, team_id: &str) -> Result<Option<String>, DbError> {
        Ok(
            sqlx::query_scalar("SELECT id FROM product_factory_runs WHERE user_id=? AND team_id=?")
                .bind(user_id)
                .bind(team_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }
}
