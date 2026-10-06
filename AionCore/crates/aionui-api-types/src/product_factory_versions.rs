use aionui_common::TimestampMs;
use serde::{Deserialize, Serialize};

use crate::ProductFactoryRunResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductFactorySourceFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductFactorySourceManifest {
    pub version: u32,
    pub scope: String,
    pub files: Vec<ProductFactorySourceFile>,
    pub total_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ProductFactorySourceManifestResponse {
    pub run_id: String,
    pub plan_revision: i64,
    pub manifest: ProductFactorySourceManifest,
    pub excluded_count: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryProductResponse {
    pub id: String,
    pub name: String,
    pub active_version_id: Option<String>,
    pub revision: i64,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryVersionResponse {
    pub id: String,
    pub product_id: String,
    pub run_id: String,
    pub parent_run_id: Option<String>,
    pub parent_version_id: Option<String>,
    pub version_no: i64,
    pub state: String,
    pub snapshot_path: Option<String>,
    pub manifest: Option<ProductFactorySourceManifest>,
    pub change_request: Option<String>,
    pub error_code: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
    pub sealed_at: Option<TimestampMs>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryVersionOperationResponse {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub version_id: String,
    pub idempotency_key: String,
    pub error_code: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

#[derive(Debug, Serialize)]
pub struct ProductFactoryVersionsResponse {
    pub product: Option<ProductFactoryProductResponse>,
    pub current_version_id: Option<String>,
    pub versions: Vec<ProductFactoryVersionResponse>,
    pub operations: Vec<ProductFactoryVersionOperationResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProductFactorySnapshotFileRequest {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotProductFactoryVersionRequest {
    pub expected_plan_revision: i64,
    pub idempotency_key: String,
    pub files: Vec<ProductFactorySnapshotFileRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IterateProductFactoryVersionRequest {
    pub source_version_id: String,
    pub expected_product_revision: i64,
    pub idempotency_key: String,
    pub change_request: String,
    pub budget_usd: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivateProductFactoryVersionRequest {
    pub version_id: String,
    pub expected_product_revision: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductFactoryVersionMutationResponse {
    pub product: ProductFactoryProductResponse,
    pub version: ProductFactoryVersionResponse,
    pub operation: ProductFactoryVersionOperationResponse,
}

#[derive(Debug, Serialize)]
pub struct ProductFactoryVersionIterationResponse {
    pub product: ProductFactoryProductResponse,
    pub version: ProductFactoryVersionResponse,
    pub operation: ProductFactoryVersionOperationResponse,
    pub run: ProductFactoryRunResponse,
}

#[derive(Debug, Serialize)]
pub struct ProductFactoryVersionActivationResponse {
    pub product: ProductFactoryProductResponse,
    pub version: ProductFactoryVersionResponse,
    pub workspace_path: String,
    pub scope: String,
}
