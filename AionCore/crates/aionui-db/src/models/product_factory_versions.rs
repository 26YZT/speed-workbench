use aionui_common::TimestampMs;

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ProductFactoryProductRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub active_version_id: Option<String>,
    pub revision: i64,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ProductFactoryVersionRow {
    pub id: String,
    pub user_id: String,
    pub product_id: String,
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub parent_version_id: Option<String>,
    pub version_no: i64,
    pub state: String,
    pub snapshot_path: Option<String>,
    pub manifest_json: Option<String>,
    pub change_request: Option<String>,
    pub error_code: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
    pub sealed_at: Option<TimestampMs>,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ProductFactoryVersionOperationRow {
    pub id: String,
    pub user_id: String,
    pub source_run_id: String,
    pub version_id: String,
    pub idempotency_key: String,
    pub input_hash: String,
    pub kind: String,
    pub state: String,
    pub error_code: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}
