//! Explicit code lineage. Sealing/selection never changes old business data or
//! usage receipts, and a new iteration remains an unconfirmed independent run.
mod filesystem;
pub(crate) use filesystem::established_root;
mod operations;
pub(crate) mod routes;

use std::path::PathBuf;
use std::sync::Arc;

use aionui_api_types::{
    ProductFactoryProductResponse, ProductFactorySourceManifest, ProductFactorySourceManifestResponse,
    ProductFactoryVersionOperationResponse, ProductFactoryVersionResponse, ProductFactoryVersionsResponse,
};
use aionui_db::{
    IProductFactoryVersionsRepository, ProductFactoryProductRow, ProductFactoryRunRow,
    ProductFactoryVersionOperationRow, ProductFactoryVersionRow,
};

use crate::ProductFactoryService;

#[derive(Debug, thiserror::Error)]
pub enum VersionError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("{0}")]
    Blocked(&'static str),
    #[error("{0}")]
    Unavailable(&'static str),
    #[error("PRODUCT_FACTORY_VERSION_NOT_FOUND")]
    NotFound,
    #[error("version storage is unavailable")]
    Database(#[from] aionui_db::DbError),
}
pub type VersionResult<T> = Result<T, VersionError>;

#[async_trait::async_trait]
pub trait ProductFactoryVersionActivityPort: Send + Sync {
    async fn check_idle(&self, user_id: &str, team_id: &str) -> VersionResult<()>;
}

#[derive(Clone)]
pub(crate) struct Versioning {
    pub repository: Arc<dyn IProductFactoryVersionsRepository>,
    pub snapshot_root: PathBuf,
    pub activity: Option<Arc<dyn ProductFactoryVersionActivityPort>>,
}

impl ProductFactoryService {
    pub fn with_versioning(
        mut self,
        repository: Arc<dyn IProductFactoryVersionsRepository>,
        snapshot_root: PathBuf,
    ) -> Self {
        self.versioning = Some(Versioning {
            repository,
            snapshot_root: established_root(snapshot_root),
            activity: None,
        });
        self
    }
    pub fn with_version_activity_port(mut self, port: Arc<dyn ProductFactoryVersionActivityPort>) -> Self {
        if let Some(versioning) = self.versioning.as_mut() {
            versioning.activity = Some(port);
        }
        self
    }
    async fn ensure_version_runtime_idle(&self, user: &str, run: &ProductFactoryRunRow) -> VersionResult<()> {
        let team = run
            .team_id
            .as_deref()
            .ok_or(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"))?;
        self.versioning()?
            .activity
            .as_ref()
            .ok_or(VersionError::Unavailable("PRODUCT_FACTORY_VERSION_RUNTIME_UNAVAILABLE"))?
            .check_idle(user, team)
            .await
    }

    fn versioning(&self) -> VersionResult<&Versioning> {
        self.versioning
            .as_ref()
            .ok_or(VersionError::Unavailable("PRODUCT_FACTORY_VERSION_NOT_CONFIGURED"))
    }
    async fn version_run(&self, user: &str, id: &str) -> VersionResult<ProductFactoryRunRow> {
        self.repository.get_run(user, id).await?.ok_or(VersionError::NotFound)
    }
    pub async fn get_versions(&self, user: &str, run_id: &str) -> VersionResult<ProductFactoryVersionsResponse> {
        self.version_run(user, run_id).await?;
        let repo = &self.versioning()?.repository;
        let Some(version) = repo.get_by_run(user, run_id).await? else {
            return Ok(ProductFactoryVersionsResponse {
                product: None,
                current_version_id: None,
                versions: vec![],
                operations: vec![],
            });
        };
        let product = repo
            .get_product(user, &version.product_id)
            .await?
            .ok_or(VersionError::NotFound)?;
        Ok(ProductFactoryVersionsResponse {
            product: Some(product_response(product)),
            current_version_id: Some(version.id),
            versions: repo
                .list_versions(user, &version.product_id)
                .await?
                .into_iter()
                .map(version_response)
                .collect::<VersionResult<_>>()?,
            operations: repo
                .list_operations(user, &version.product_id)
                .await?
                .into_iter()
                .map(operation_response)
                .collect(),
        })
    }
    pub async fn get_source_manifest(
        &self,
        user: &str,
        run_id: &str,
    ) -> VersionResult<ProductFactorySourceManifestResponse> {
        let run = self.version_run(user, run_id).await?;
        let path = PathBuf::from(&run.workspace_path);
        let (manifest, excluded_count) = tokio::task::spawn_blocking(move || filesystem::source_manifest(&path))
            .await
            .map_err(|_| VersionError::Unavailable("PRODUCT_FACTORY_VERSION_COPY_FAILED"))??;
        Ok(ProductFactorySourceManifestResponse {
            run_id: run.id,
            plan_revision: run.plan_revision,
            manifest,
            excluded_count,
            warnings: vec![
                "RUNTIME_DATA_NOT_INCLUDED".into(),
                "SOURCE_MANIFEST_REVIEW_REQUIRED".into(),
            ],
        })
    }
    /// Team calls use the same code-version boundary, including ordinary chat
    /// without a current task. Unregistered/ordinary teams keep their behavior.
    pub async fn ensure_team_version_mutable(&self, user: &str, team: &str) -> Result<(), crate::ProductFactoryError> {
        for run in self.repository.list_runs(user).await? {
            if run.team_id.as_deref() == Some(team) {
                self.ensure_version_mutable(user, &run.id).await?;
            }
        }
        Ok(())
    }

    /// Called by Factory mutators after reading their owner-scoped run. Reads
    /// remain available to display retained usage and copy recovery receipts.
    pub(crate) async fn ensure_version_mutable(
        &self,
        user: &str,
        run_id: &str,
    ) -> Result<(), crate::ProductFactoryError> {
        if let Some(versioning) = self.versioning.as_ref()
            && let Some(version) = versioning.repository.get_by_run(user, run_id).await?
            && version.state != "working"
        {
            return Err(crate::ProductFactoryError::ExecutionBlocked(
                "code version is sealed, copying, or failed; select or create an independent working version".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn manifest(version: &ProductFactoryVersionRow) -> VersionResult<ProductFactorySourceManifest> {
    let manifest = version
        .manifest_json
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .ok_or(VersionError::Invalid("PRODUCT_FACTORY_VERSION_MANIFEST_INVALID"))?;
    filesystem::validate_manifest(&manifest)?;
    Ok(manifest)
}
fn product_response(row: ProductFactoryProductRow) -> ProductFactoryProductResponse {
    ProductFactoryProductResponse {
        id: row.id,
        name: row.name,
        active_version_id: row.active_version_id,
        revision: row.revision,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
fn version_response(row: ProductFactoryVersionRow) -> VersionResult<ProductFactoryVersionResponse> {
    let manifest = row
        .manifest_json
        .as_deref()
        .map(|value| {
            serde_json::from_str(value).map_err(|_| VersionError::Invalid("PRODUCT_FACTORY_VERSION_MANIFEST_INVALID"))
        })
        .transpose()?;
    Ok(ProductFactoryVersionResponse {
        id: row.id,
        product_id: row.product_id,
        run_id: row.run_id,
        parent_run_id: row.parent_run_id,
        parent_version_id: row.parent_version_id,
        version_no: row.version_no,
        state: row.state,
        snapshot_path: row.snapshot_path,
        manifest,
        change_request: row.change_request,
        error_code: row.error_code,
        created_at: row.created_at,
        updated_at: row.updated_at,
        sealed_at: row.sealed_at,
    })
}
fn operation_response(row: ProductFactoryVersionOperationRow) -> ProductFactoryVersionOperationResponse {
    ProductFactoryVersionOperationResponse {
        id: row.id,
        kind: row.kind,
        state: row.state,
        version_id: row.version_id,
        idempotency_key: row.idempotency_key,
        error_code: row.error_code,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
