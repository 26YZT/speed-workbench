use std::path::PathBuf;

use aionui_api_types::{
    ActivateProductFactoryVersionRequest, IterateProductFactoryVersionRequest, ProductFactoryVersionActivationResponse,
    ProductFactoryVersionIterationResponse, ProductFactoryVersionMutationResponse,
    SnapshotProductFactoryVersionRequest,
};
use aionui_common::{generate_id, now_ms};
use aionui_db::{
    IterationVersionReservation, ProductFactoryRunRow, ProductFactoryVersionOperationRow, ProductFactoryVersionRow,
    SnapshotVersionReservation,
};
use sha2::{Digest, Sha256};

use super::{
    VersionError, VersionResult, filesystem, manifest, operation_response, product_response, version_response,
};
use crate::ProductFactoryService;
use crate::service::{parse_task_draft, to_response};

struct CopyFiles {
    source: PathBuf,
    target_root: PathBuf,
    relative: String,
    manifest: aionui_api_types::ProductFactorySourceManifest,
    sealed: bool,
    source_run_id: String,
}

fn key(value: &str) -> VersionResult<()> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_REQUEST"));
    }
    Ok(())
}
fn input_hash(value: serde_json::Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
fn storage(error: aionui_db::DbError) -> VersionError {
    match error {
        aionui_db::DbError::NotFound(_) => VersionError::NotFound,
        aionui_db::DbError::Conflict(code) => match code.as_str() {
            "VERSION_IDEMPOTENCY_CONFLICT" => VersionError::Conflict("PRODUCT_FACTORY_VERSION_IDEMPOTENCY_CONFLICT"),
            "VERSION_SNAPSHOT_INCOMPLETE" => VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"),
            "VERSION_ALREADY_SEALED_OR_COPYING" => VersionError::Blocked("PRODUCT_FACTORY_VERSION_ALREADY_SEALED"),
            _ => VersionError::Conflict("PRODUCT_FACTORY_VERSION_REVISION_CONFLICT"),
        },
        other => VersionError::Database(other),
    }
}

impl ProductFactoryService {
    async fn operation_result(
        &self,
        user: &str,
        run: &str,
        key: &str,
    ) -> VersionResult<ProductFactoryVersionMutationResponse> {
        let repo = &self.versioning()?.repository;
        let operation = repo
            .get_operation_by_key(user, run, key)
            .await?
            .ok_or(VersionError::NotFound)?;
        let version = repo
            .get_version(user, &operation.version_id)
            .await?
            .ok_or(VersionError::NotFound)?;
        let product = repo
            .get_product(user, &version.product_id)
            .await?
            .ok_or(VersionError::NotFound)?;
        Ok(ProductFactoryVersionMutationResponse {
            product: product_response(product),
            version: version_response(version)?,
            operation: operation_response(operation),
        })
    }
    async fn replay_operation(
        &self,
        user: &str,
        run: &str,
        key: &str,
        kind: &str,
        hash: &str,
    ) -> VersionResult<Option<ProductFactoryVersionMutationResponse>> {
        let Some(operation) = self
            .versioning()?
            .repository
            .get_operation_by_key(user, run, key)
            .await?
        else {
            return Ok(None);
        };
        if operation.kind != kind || operation.input_hash != hash {
            return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_IDEMPOTENCY_CONFLICT"));
        }
        Ok(Some(self.operation_result(user, run, key).await?))
    }
    async fn snapshot_completion(&self, user: &str, run: &ProductFactoryRunRow) -> VersionResult<Vec<String>> {
        if run.status != "completed" {
            return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"));
        }
        let draft =
            parse_task_draft(run).map_err(|_| VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"))?;
        if !draft.confirmed || draft.tasks.is_empty() {
            return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"));
        }
        let team = run
            .team_id
            .as_deref()
            .ok_or(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"))?;
        let owned = self
            .repository
            .get_handoff_team(user, team)
            .await?
            .ok_or(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"))?;
        if owned.user_id != user || self.repository.get_execution(user, &run.id).await?.is_none() {
            return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"));
        }
        let mut task_ids = Vec::new();
        for task in draft.tasks {
            let id = format!("{team}-{}", task.id);
            let row = self
                .repository
                .get_handoff_task(user, team, &id)
                .await?
                .ok_or(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"))?;
            if row.status != "completed" {
                return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"));
            }
            task_ids.push(id);
        }
        Ok(task_ids)
    }
    pub async fn snapshot_version(
        &self,
        user: &str,
        run_id: &str,
        mut request: SnapshotProductFactoryVersionRequest,
    ) -> VersionResult<ProductFactoryVersionMutationResponse> {
        key(&request.idempotency_key)?;
        if request.expected_plan_revision < 1 {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_REQUEST"));
        }
        request.files.sort_by(|a, b| a.path.cmp(&b.path));
        let hash = input_hash(
            serde_json::json!({"kind":"snapshot","expected_plan_revision":request.expected_plan_revision,"files":request.files}),
        );
        self.version_run(user, run_id).await?;
        if let Some(result) = self
            .replay_operation(user, run_id, &request.idempotency_key, "snapshot", &hash)
            .await?
        {
            return Ok(result);
        }
        let run = self.version_run(user, run_id).await?;
        if run.plan_revision != request.expected_plan_revision {
            return Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_REVISION_CONFLICT"));
        }
        let task_ids = self.snapshot_completion(user, &run).await?;
        self.ensure_version_runtime_idle(user, &run).await?;
        let versioning = self.versioning()?.clone();
        let existing = versioning.repository.get_by_run(user, run_id).await?;
        if existing.as_ref().is_some_and(|version| version.state == "sealed") {
            return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_ALREADY_SEALED"));
        }
        let product_id = existing
            .as_ref()
            .map(|row| row.product_id.clone())
            .unwrap_or_else(generate_id);
        let version_id = existing.as_ref().map(|row| row.id.clone()).unwrap_or_else(generate_id);
        let operation_id = generate_id();
        let relative = format!("{product_id}/{version_id}/{operation_id}");
        let path = PathBuf::from(&run.workspace_path);
        let files = request.files;
        let selected = tokio::task::spawn_blocking(move || filesystem::selected_manifest(&path, &files))
            .await
            .map_err(|_| VersionError::Unavailable("PRODUCT_FACTORY_VERSION_COPY_FAILED"))??;
        let snapshot_path = versioning.snapshot_root.join(&relative).to_string_lossy().into_owned();
        let manifest_json = serde_json::to_string(&selected)
            .map_err(|_| VersionError::Invalid("PRODUCT_FACTORY_VERSION_MANIFEST_INVALID"))?;
        let now = now_ms();
        let operation = ProductFactoryVersionOperationRow {
            id: operation_id.clone(),
            user_id: user.into(),
            source_run_id: run_id.into(),
            version_id: version_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            input_hash: hash,
            kind: "snapshot".into(),
            state: "reserved".into(),
            error_code: None,
            created_at: now,
            updated_at: now,
        };
        if !versioning
            .repository
            .reserve_snapshot(SnapshotVersionReservation {
                run: &run,
                product_id: &product_id,
                version_id: &version_id,
                operation: &operation,
                manifest_json: &manifest_json,
                snapshot_path: &snapshot_path,
                completed_task_ids: &task_ids,
            })
            .await
            .map_err(storage)?
        {
            return self.operation_result(user, run_id, &request.idempotency_key).await;
        }
        self.copy_version(
            user,
            &operation_id,
            CopyFiles {
                source: PathBuf::from(&run.workspace_path),
                target_root: versioning.snapshot_root.clone(),
                relative,
                manifest: selected,
                sealed: true,
                source_run_id: run_id.to_owned(),
            },
        )
        .await?;
        self.operation_result(user, run_id, &request.idempotency_key).await
    }
    async fn copy_version(&self, user: &str, id: &str, copy: CopyFiles) -> VersionResult<()> {
        let CopyFiles {
            source,
            target_root,
            relative,
            manifest,
            sealed,
            source_run_id,
        } = copy;
        let repo = &self.versioning()?.repository;
        if !repo.claim_copy(user, id, now_ms()).await? {
            return Ok(());
        }
        tracing::info!(operation_id = id, sealed, "code version copy started");
        let result = tokio::task::spawn_blocking(move || {
            let path = filesystem::copy_manifest(&source, &target_root, &relative, &manifest, sealed)?;
            filesystem::verify_exact_manifest(&path, &manifest)?;
            Ok::<_, VersionError>(())
        })
        .await
        .map_err(|_| VersionError::Unavailable("PRODUCT_FACTORY_VERSION_COPY_FAILED"))
        .and_then(|value| value);
        match result {
            Ok(()) if sealed => {
                let run = self.version_run(user, &source_run_id).await?;
                if let Err(error) = self.ensure_version_runtime_idle(user, &run).await {
                    let _ = repo
                        .finish_copy(user, id, false, Some("PRODUCT_FACTORY_VERSION_RUNTIME_BUSY"), now_ms())
                        .await;
                    return Err(error);
                }
                self.commit_version_copy(user, id).await
            }
            Ok(()) => self.commit_version_copy(user, id).await,
            Err(error) => {
                let code = match &error {
                    VersionError::Invalid(code)
                    | VersionError::Conflict(code)
                    | VersionError::Blocked(code)
                    | VersionError::Unavailable(code) => *code,
                    _ => "PRODUCT_FACTORY_VERSION_COPY_FAILED",
                };
                let _ = repo.finish_copy(user, id, false, Some(code), now_ms()).await;
                tracing::warn!(
                    operation_id = id,
                    error_code = code,
                    "code version copy failed; old files retained"
                );
                Err(error)
            }
        }
    }
    async fn commit_version_copy(&self, user: &str, id: &str) -> VersionResult<()> {
        let repo = &self.versioning()?.repository;
        match repo.finish_copy(user, id, true, None, now_ms()).await {
            Ok(true) => {
                tracing::info!(operation_id = id, "code version copy committed");
                Ok(())
            }
            Ok(false) => Err(VersionError::Conflict("PRODUCT_FACTORY_VERSION_COPY_SUPERSEDED")),
            Err(error) => {
                let _ = repo
                    .finish_copy(
                        user,
                        id,
                        false,
                        Some("PRODUCT_FACTORY_VERSION_SNAPSHOT_INCOMPLETE"),
                        now_ms(),
                    )
                    .await;
                Err(storage(error))
            }
        }
    }
    fn sealed_path(&self, version: &ProductFactoryVersionRow) -> VersionResult<PathBuf> {
        if version.state != "sealed" {
            return Err(VersionError::Blocked("PRODUCT_FACTORY_VERSION_NOT_SEALED"));
        }
        let path = PathBuf::from(
            version
                .snapshot_path
                .as_deref()
                .ok_or(VersionError::Blocked("PRODUCT_FACTORY_VERSION_NOT_SEALED"))?,
        );
        let expected_root = self
            .versioning()?
            .snapshot_root
            .join(&version.product_id)
            .join(&version.id);
        if !path.starts_with(&expected_root)
            || path
                .strip_prefix(&expected_root)
                .ok()
                .and_then(|value| value.to_str())
                .is_none_or(|relative| {
                    relative.is_empty() || relative.contains('/') || filesystem::validate_relative(relative).is_err()
                })
        {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_PATH"));
        }
        Ok(path)
    }
    pub async fn iterate_version(
        &self,
        user: &str,
        run_id: &str,
        request: IterateProductFactoryVersionRequest,
    ) -> VersionResult<ProductFactoryVersionIterationResponse> {
        key(&request.idempotency_key)?;
        let change = request.change_request.trim();
        if change.is_empty() || change.len() > 8192 || request.expected_product_revision < 1 {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_REQUEST"));
        }
        let budget = request
            .budget_usd
            .map(|value| {
                let normalized = (value * 100.0).round() / 100.0;
                if !value.is_finite() || value <= 0.0 || !normalized.is_finite() || normalized <= 0.0 {
                    return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_REQUEST"));
                }
                Ok(normalized)
            })
            .transpose()?;
        let hash = input_hash(
            serde_json::json!({"kind":"iterate","source_version_id":request.source_version_id,"expected_product_revision":request.expected_product_revision,"change_request":change,"budget_usd":budget}),
        );
        self.version_run(user, run_id).await?;
        if let Some(result) = self
            .replay_operation(user, run_id, &request.idempotency_key, "iterate", &hash)
            .await?
        {
            let run = self.version_run(user, &result.version.run_id).await?;
            return Ok(ProductFactoryVersionIterationResponse {
                product: result.product,
                version: result.version,
                operation: result.operation,
                run: to_response(run),
            });
        }
        let versioning = self.versioning()?.clone();
        let current = versioning
            .repository
            .get_by_run(user, run_id)
            .await?
            .ok_or(VersionError::NotFound)?;
        let source = versioning
            .repository
            .get_version(user, &request.source_version_id)
            .await?
            .filter(|row| row.product_id == current.product_id)
            .ok_or(VersionError::NotFound)?;
        let source_path = self.sealed_path(&source)?;
        let selected = manifest(&source)?;
        let source_check = source_path.clone();
        let manifest_check = selected.clone();
        tokio::task::spawn_blocking(move || filesystem::verify_exact_manifest(&source_check, &manifest_check))
            .await
            .map_err(|_| VersionError::Unavailable("PRODUCT_FACTORY_VERSION_COPY_FAILED"))??;
        let old = self.version_run(user, &source.run_id).await?;
        let managed = self
            .managed_workspace_root
            .as_ref()
            .ok_or(VersionError::Unavailable("PRODUCT_FACTORY_VERSION_NOT_CONFIGURED"))?
            .clone();
        let id = generate_id();
        let now = now_ms();
        let version_id = generate_id();
        let operation_id = generate_id();
        let idea = format!("{}\n\n变更要求：{}", old.idea, change);
        if idea.len() > 32768 {
            return Err(VersionError::Invalid("PRODUCT_FACTORY_VERSION_INVALID_REQUEST"));
        }
        let run = ProductFactoryRunRow {
            id: id.clone(),
            plan_revision: 1,
            user_id: user.into(),
            name: old.name,
            idea,
            target_user: old.target_user,
            problem: old.problem,
            expected_output: old.expected_output,
            workspace_path: managed.join(&id).to_string_lossy().into_owned(),
            budget_usd: budget,
            status: "draft".into(),
            interview_json: None,
            blueprint_json: None,
            task_draft_json: None,
            team_id: None,
            created_at: now,
            updated_at: now,
        };
        let operation = ProductFactoryVersionOperationRow {
            id: operation_id.clone(),
            user_id: user.into(),
            source_run_id: run_id.into(),
            version_id: version_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            input_hash: hash,
            kind: "iterate".into(),
            state: "reserved".into(),
            error_code: None,
            created_at: now,
            updated_at: now,
        };
        if versioning
            .repository
            .reserve_iteration(IterationVersionReservation {
                source: &source,
                source_run_id: run_id,
                expected_product_revision: request.expected_product_revision,
                run: &run,
                version_id: &version_id,
                operation: &operation,
                change_request: change,
            })
            .await
            .map_err(storage)?
        {
            self.copy_version(
                user,
                &operation_id,
                CopyFiles {
                    source: source_path,
                    target_root: managed,
                    relative: id,
                    manifest: selected,
                    sealed: false,
                    source_run_id: run_id.to_owned(),
                },
            )
            .await?;
        }
        let result = self.operation_result(user, run_id, &request.idempotency_key).await?;
        let run = self.version_run(user, &result.version.run_id).await?;
        Ok(ProductFactoryVersionIterationResponse {
            product: result.product,
            version: result.version,
            operation: result.operation,
            run: to_response(run),
        })
    }
    pub async fn activate_version(
        &self,
        user: &str,
        run_id: &str,
        request: ActivateProductFactoryVersionRequest,
    ) -> VersionResult<ProductFactoryVersionActivationResponse> {
        self.version_run(user, run_id).await?;
        let repo = &self.versioning()?.repository;
        let current = repo.get_by_run(user, run_id).await?.ok_or(VersionError::NotFound)?;
        let version = repo
            .get_version(user, &request.version_id)
            .await?
            .filter(|row| row.product_id == current.product_id)
            .ok_or(VersionError::NotFound)?;
        let path = self.sealed_path(&version)?;
        let selected = manifest(&version)?;
        let check = path.clone();
        tokio::task::spawn_blocking(move || filesystem::verify_exact_manifest(&check, &selected))
            .await
            .map_err(|_| VersionError::Unavailable("PRODUCT_FACTORY_VERSION_COPY_FAILED"))??;
        let product = repo
            .activate(
                user,
                &version.product_id,
                &version.id,
                request.expected_product_revision,
                now_ms(),
            )
            .await
            .map_err(storage)?;
        Ok(ProductFactoryVersionActivationResponse {
            product: product_response(product),
            version: version_response(version)?,
            workspace_path: path.to_string_lossy().into_owned(),
            scope: "code_only".into(),
        })
    }
}
