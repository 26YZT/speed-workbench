use sqlx::SqlitePool;

use crate::DbError;
use crate::models::{ProductFactoryProductRow, ProductFactoryVersionOperationRow, ProductFactoryVersionRow};
use crate::repository::product_factory_versions::{
    IProductFactoryVersionsRepository, IterationVersionReservation, SnapshotVersionReservation,
};

#[derive(Clone)]
pub struct SqliteProductFactoryVersionsRepository {
    pool: SqlitePool,
}
impl SqliteProductFactoryVersionsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl IProductFactoryVersionsRepository for SqliteProductFactoryVersionsRepository {
    async fn get_product(&self, user: &str, id: &str) -> Result<Option<ProductFactoryProductRow>, DbError> {
        Ok(
            sqlx::query_as("SELECT * FROM product_factory_products WHERE user_id=? AND id=?")
                .bind(user)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }
    async fn get_by_run(&self, user: &str, run: &str) -> Result<Option<ProductFactoryVersionRow>, DbError> {
        Ok(sqlx::query_as("SELECT v.* FROM product_factory_versions v JOIN product_factory_products p ON p.id=v.product_id AND p.user_id=v.user_id WHERE v.user_id=? AND v.run_id=?").bind(user).bind(run).fetch_optional(&self.pool).await?)
    }
    async fn get_version(&self, user: &str, id: &str) -> Result<Option<ProductFactoryVersionRow>, DbError> {
        Ok(sqlx::query_as("SELECT v.* FROM product_factory_versions v JOIN product_factory_products p ON p.id=v.product_id AND p.user_id=v.user_id WHERE v.user_id=? AND v.id=?").bind(user).bind(id).fetch_optional(&self.pool).await?)
    }
    async fn list_versions(&self, user: &str, product: &str) -> Result<Vec<ProductFactoryVersionRow>, DbError> {
        Ok(sqlx::query_as(
            "SELECT * FROM product_factory_versions WHERE user_id=? AND product_id=? ORDER BY version_no DESC",
        )
        .bind(user)
        .bind(product)
        .fetch_all(&self.pool)
        .await?)
    }
    async fn list_operations(
        &self,
        user: &str,
        product: &str,
    ) -> Result<Vec<ProductFactoryVersionOperationRow>, DbError> {
        Ok(sqlx::query_as("SELECT o.* FROM product_factory_version_operations o JOIN product_factory_versions v ON v.id=o.version_id AND v.user_id=o.user_id WHERE o.user_id=? AND v.product_id=? ORDER BY o.created_at DESC,o.id DESC").bind(user).bind(product).fetch_all(&self.pool).await?)
    }
    async fn get_operation_by_key(
        &self,
        user: &str,
        run: &str,
        key: &str,
    ) -> Result<Option<ProductFactoryVersionOperationRow>, DbError> {
        Ok(sqlx::query_as("SELECT * FROM product_factory_version_operations WHERE user_id=? AND source_run_id=? AND idempotency_key=?").bind(user).bind(run).bind(key).fetch_optional(&self.pool).await?)
    }
    async fn reserve_snapshot(&self, r: SnapshotVersionReservation<'_>) -> Result<bool, DbError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let existing:Option<ProductFactoryVersionOperationRow>=sqlx::query_as("SELECT * FROM product_factory_version_operations WHERE user_id=? AND source_run_id=? AND idempotency_key=?")
            .bind(&r.run.user_id).bind(&r.run.id).bind(&r.operation.idempotency_key).fetch_optional(&mut *tx).await?;
        if let Some(existing) = existing {
            if existing.input_hash != r.operation.input_hash || existing.kind != "snapshot" {
                return Err(DbError::Conflict("VERSION_IDEMPOTENCY_CONFLICT".into()));
            }
            return Ok(false);
        }
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_factory_runs WHERE user_id=? AND id=? AND plan_revision=? AND status='completed' AND team_id IS NOT NULL AND EXISTS(SELECT 1 FROM product_factory_execution e WHERE e.user_id=product_factory_runs.user_id AND e.run_id=product_factory_runs.id))")
            .bind(&r.run.user_id).bind(&r.run.id).bind(r.run.plan_revision).fetch_one(&mut *tx).await?;
        if !valid || r.completed_task_ids.is_empty() {
            return Err(DbError::Conflict("VERSION_SNAPSHOT_INCOMPLETE".into()));
        }
        for id in r.completed_task_ids {
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM team_tasks t JOIN teams p ON p.id=t.team_id WHERE p.user_id=? AND t.team_id=? AND t.id=? AND t.status='completed')")
                .bind(&r.run.user_id).bind(&r.run.team_id).bind(id).fetch_one(&mut *tx).await?;
            if !valid {
                return Err(DbError::Conflict("VERSION_SNAPSHOT_INCOMPLETE".into()));
            }
        }
        let current: Option<ProductFactoryVersionRow> =
            sqlx::query_as("SELECT * FROM product_factory_versions WHERE user_id=? AND run_id=?")
                .bind(&r.run.user_id)
                .bind(&r.run.id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(current) = current {
            if current.id != r.version_id || !matches!(current.state.as_str(), "working" | "failed" | "copying") {
                return Err(DbError::Conflict("VERSION_ALREADY_SEALED_OR_COPYING".into()));
            }
            sqlx::query("UPDATE product_factory_version_operations SET state='failed',error_code='PRODUCT_FACTORY_VERSION_COPY_SUPERSEDED',updated_at=? WHERE user_id=? AND version_id=? AND state IN ('reserved','copying')")
                .bind(r.operation.created_at).bind(&r.run.user_id).bind(r.version_id).execute(&mut *tx).await?;
            sqlx::query("UPDATE product_factory_versions SET state='copying',snapshot_path=?,manifest_json=?,error_code=NULL,updated_at=? WHERE user_id=? AND id=?")
                .bind(r.snapshot_path).bind(r.manifest_json).bind(r.operation.created_at).bind(&r.run.user_id).bind(r.version_id).execute(&mut *tx).await?;
            sqlx::query(
                "UPDATE product_factory_products SET revision=revision+1,updated_at=? WHERE user_id=? AND id=?",
            )
            .bind(r.operation.created_at)
            .bind(&r.run.user_id)
            .bind(&current.product_id)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query("INSERT INTO product_factory_products (id,user_id,name,revision,created_at,updated_at) VALUES (?,?,?,1,?,?)")
                .bind(r.product_id).bind(&r.run.user_id).bind(&r.run.name).bind(r.operation.created_at).bind(r.operation.created_at).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO product_factory_versions (id,user_id,product_id,run_id,version_no,state,snapshot_path,manifest_json,created_at,updated_at) VALUES (?,?,?,?,1,'copying',?,?,?,?)")
                .bind(r.version_id).bind(&r.run.user_id).bind(r.product_id).bind(&r.run.id).bind(r.snapshot_path).bind(r.manifest_json).bind(r.operation.created_at).bind(r.operation.created_at).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO product_factory_version_operations (id,user_id,source_run_id,version_id,idempotency_key,input_hash,kind,state,created_at,updated_at,input_plan_revision,task_ids_json) VALUES (?,?,?,?,?,?,'snapshot','reserved',?,?,?,?)")
            .bind(&r.operation.id).bind(&r.run.user_id).bind(&r.run.id).bind(r.version_id).bind(&r.operation.idempotency_key).bind(&r.operation.input_hash)
            .bind(r.operation.created_at).bind(r.operation.created_at).bind(r.run.plan_revision).bind(serde_json::to_string(r.completed_task_ids).map_err(|_|DbError::Init("version task serialization failed".into()))?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    async fn reserve_iteration(&self, r: IterationVersionReservation<'_>) -> Result<bool, DbError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let existing:Option<ProductFactoryVersionOperationRow>=sqlx::query_as("SELECT * FROM product_factory_version_operations WHERE user_id=? AND source_run_id=? AND idempotency_key=?")
            .bind(&r.run.user_id).bind(r.source_run_id).bind(&r.operation.idempotency_key).fetch_optional(&mut *tx).await?;
        if let Some(existing) = existing {
            if existing.input_hash != r.operation.input_hash || existing.kind != "iterate" {
                return Err(DbError::Conflict("VERSION_IDEMPOTENCY_CONFLICT".into()));
            }
            return Ok(false);
        }
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_factory_versions v JOIN product_factory_products p ON p.id=v.product_id AND p.user_id=v.user_id WHERE v.user_id=? AND v.id=? AND v.product_id=? AND v.state='sealed' AND v.snapshot_path IS NOT NULL AND v.manifest_json IS NOT NULL AND p.revision=?)")
            .bind(&r.run.user_id).bind(&r.source.id).bind(&r.source.product_id).bind(r.expected_product_revision).fetch_one(&mut *tx).await?;
        let owned_source: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM product_factory_versions WHERE user_id=? AND run_id=? AND product_id=?)",
        )
        .bind(&r.run.user_id)
        .bind(r.source_run_id)
        .bind(&r.source.product_id)
        .fetch_one(&mut *tx)
        .await?;
        if !valid || !owned_source {
            return Err(DbError::Conflict("VERSION_REVISION_CONFLICT".into()));
        }
        let number: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(version_no),0)+1 FROM product_factory_versions WHERE product_id=?")
                .bind(&r.source.product_id)
                .fetch_one(&mut *tx)
                .await?;
        sqlx::query("INSERT INTO product_factory_runs (id,user_id,name,idea,target_user,problem,expected_output,workspace_path,budget_usd,status,created_at,updated_at,plan_revision) VALUES (?,?,?,?,?,?,?,?,?,'draft',?,?,1)")
            .bind(&r.run.id).bind(&r.run.user_id).bind(&r.run.name).bind(&r.run.idea).bind(&r.run.target_user).bind(&r.run.problem).bind(&r.run.expected_output).bind(&r.run.workspace_path).bind(r.run.budget_usd).bind(r.run.created_at).bind(r.run.updated_at).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO product_factory_versions (id,user_id,product_id,run_id,parent_run_id,parent_version_id,version_no,state,manifest_json,change_request,created_at,updated_at) VALUES (?,?,?,?,?,?,?,'copying',?,?,?,?)")
            .bind(r.version_id).bind(&r.run.user_id).bind(&r.source.product_id).bind(&r.run.id).bind(&r.source.run_id).bind(&r.source.id).bind(number).bind(&r.source.manifest_json).bind(r.change_request).bind(r.run.created_at).bind(r.run.updated_at).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO product_factory_version_operations (id,user_id,source_run_id,version_id,idempotency_key,input_hash,kind,state,created_at,updated_at) VALUES (?,?,?,?,?,?,'iterate','reserved',?,?)")
            .bind(&r.operation.id).bind(&r.run.user_id).bind(r.source_run_id).bind(r.version_id).bind(&r.operation.idempotency_key).bind(&r.operation.input_hash).bind(r.run.created_at).bind(r.run.updated_at).execute(&mut *tx).await?;
        sqlx::query("UPDATE product_factory_products SET revision=revision+1,updated_at=? WHERE user_id=? AND id=? AND revision=?")
            .bind(r.run.created_at).bind(&r.run.user_id).bind(&r.source.product_id).bind(r.expected_product_revision).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    async fn claim_copy(&self, user: &str, id: &str, now: i64) -> Result<bool, DbError> {
        Ok(sqlx::query("UPDATE product_factory_version_operations SET state='copying',updated_at=? WHERE user_id=? AND id=? AND state='reserved'").bind(now).bind(user).bind(id).execute(&self.pool).await?.rows_affected()==1)
    }
    async fn finish_copy(
        &self,
        user: &str,
        id: &str,
        success: bool,
        error: Option<&str>,
        now: i64,
    ) -> Result<bool, DbError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let operation: Option<ProductFactoryVersionOperationRow> = sqlx::query_as(
            "SELECT * FROM product_factory_version_operations WHERE user_id=? AND id=? AND state='copying'",
        )
        .bind(user)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(operation) = operation else { return Ok(false) };
        if success && operation.kind == "snapshot" {
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_factory_runs r JOIN product_factory_version_operations o ON o.source_run_id=r.id AND o.user_id=r.user_id WHERE o.id=? AND r.user_id=? AND r.status='completed' AND r.plan_revision=o.input_plan_revision)")
                .bind(id).bind(user).fetch_one(&mut *tx).await?;
            let task_json: Option<String> = sqlx::query_scalar(
                "SELECT task_ids_json FROM product_factory_version_operations WHERE id=? AND user_id=?",
            )
            .bind(id)
            .bind(user)
            .fetch_one(&mut *tx)
            .await?;
            let task_ids: Vec<String> = task_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .ok_or_else(|| DbError::Conflict("VERSION_SNAPSHOT_INCOMPLETE".into()))?;
            if !valid || task_ids.is_empty() {
                return Err(DbError::Conflict("VERSION_SNAPSHOT_INCOMPLETE".into()));
            }
            for task in task_ids {
                let completed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM team_tasks t JOIN teams p ON p.id=t.team_id JOIN product_factory_runs r ON r.team_id=p.id WHERE r.id=? AND r.user_id=? AND p.user_id=? AND t.id=? AND t.status='completed')")
                    .bind(&operation.source_run_id).bind(user).bind(user).bind(task).fetch_one(&mut *tx).await?;
                if !completed {
                    return Err(DbError::Conflict("VERSION_SNAPSHOT_INCOMPLETE".into()));
                }
            }
        }
        let next = if success {
            if operation.kind == "snapshot" {
                "sealed"
            } else {
                "working"
            }
        } else {
            "failed"
        };
        let changed=sqlx::query("UPDATE product_factory_versions SET state=?,error_code=?,sealed_at=CASE WHEN ?='sealed' THEN ? ELSE sealed_at END,updated_at=? WHERE user_id=? AND id=? AND state='copying'")
            .bind(next).bind(error).bind(next).bind(now).bind(now).bind(user).bind(&operation.version_id).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(DbError::Conflict("VERSION_COPY_STATE_CONFLICT".into()));
        }
        sqlx::query("UPDATE product_factory_version_operations SET state=?,error_code=?,updated_at=? WHERE user_id=? AND id=? AND state='copying'")
            .bind(if success {"complete"} else {"failed"}).bind(error).bind(now).bind(user).bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE product_factory_products SET revision=revision+1,updated_at=? WHERE user_id=? AND id=(SELECT product_id FROM product_factory_versions WHERE user_id=? AND id=?)")
            .bind(now).bind(user).bind(user).bind(&operation.version_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    async fn activate(
        &self,
        user: &str,
        product: &str,
        version: &str,
        revision: i64,
        now: i64,
    ) -> Result<ProductFactoryProductRow, DbError> {
        let mut transaction = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let result=sqlx::query("UPDATE product_factory_products SET active_version_id=?,revision=revision+1,updated_at=? WHERE user_id=? AND id=? AND revision=? AND EXISTS(SELECT 1 FROM product_factory_versions WHERE user_id=? AND id=? AND product_id=? AND state='sealed')")
            .bind(version).bind(now).bind(user).bind(product).bind(revision).bind(user).bind(version).bind(product).execute(&mut *transaction).await?;
        if result.rows_affected() != 1 {
            return Err(DbError::Conflict("VERSION_REVISION_CONFLICT".into()));
        }
        let product = sqlx::query_as("SELECT * FROM product_factory_products WHERE user_id=? AND id=?")
            .bind(user)
            .bind(product)
            .fetch_one(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(product)
    }
}
