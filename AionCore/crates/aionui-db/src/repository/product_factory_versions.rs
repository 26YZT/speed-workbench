use crate::DbError;
use crate::models::{
    ProductFactoryProductRow, ProductFactoryRunRow, ProductFactoryVersionOperationRow, ProductFactoryVersionRow,
};

pub struct SnapshotVersionReservation<'a> {
    pub run: &'a ProductFactoryRunRow,
    pub product_id: &'a str,
    pub version_id: &'a str,
    pub operation: &'a ProductFactoryVersionOperationRow,
    pub manifest_json: &'a str,
    pub snapshot_path: &'a str,
    pub completed_task_ids: &'a [String],
}

pub struct IterationVersionReservation<'a> {
    pub source: &'a ProductFactoryVersionRow,
    pub source_run_id: &'a str,
    pub expected_product_revision: i64,
    pub run: &'a ProductFactoryRunRow,
    pub version_id: &'a str,
    pub operation: &'a ProductFactoryVersionOperationRow,
    pub change_request: &'a str,
}

#[async_trait::async_trait]
pub trait IProductFactoryVersionsRepository: Send + Sync {
    async fn get_product(&self, user_id: &str, id: &str) -> Result<Option<ProductFactoryProductRow>, DbError>;
    async fn get_by_run(&self, user_id: &str, run_id: &str) -> Result<Option<ProductFactoryVersionRow>, DbError>;
    async fn get_version(&self, user_id: &str, id: &str) -> Result<Option<ProductFactoryVersionRow>, DbError>;
    async fn list_versions(&self, user_id: &str, product_id: &str) -> Result<Vec<ProductFactoryVersionRow>, DbError>;
    async fn list_operations(
        &self,
        user_id: &str,
        product_id: &str,
    ) -> Result<Vec<ProductFactoryVersionOperationRow>, DbError>;
    async fn get_operation_by_key(
        &self,
        user_id: &str,
        source_run_id: &str,
        key: &str,
    ) -> Result<Option<ProductFactoryVersionOperationRow>, DbError>;
    /// All reservations use a write transaction. False means read the existing
    /// operation back, never allocate/copy a second version for this key.
    async fn reserve_snapshot(&self, reservation: SnapshotVersionReservation<'_>) -> Result<bool, DbError>;
    async fn reserve_iteration(&self, reservation: IterationVersionReservation<'_>) -> Result<bool, DbError>;
    async fn claim_copy(&self, user_id: &str, operation_id: &str, now: i64) -> Result<bool, DbError>;
    async fn finish_copy(
        &self,
        user_id: &str,
        operation_id: &str,
        success: bool,
        error_code: Option<&str>,
        now: i64,
    ) -> Result<bool, DbError>;
    async fn activate(
        &self,
        user_id: &str,
        product_id: &str,
        version_id: &str,
        expected_revision: i64,
        now: i64,
    ) -> Result<ProductFactoryProductRow, DbError>;
}
