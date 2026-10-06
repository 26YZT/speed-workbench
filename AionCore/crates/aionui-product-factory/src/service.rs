use std::sync::Arc;

use aionui_api_types::{
    ConfirmTaskDraftRequest, CreateProductFactoryRunRequest, ProductFactoryRunResponse, ProductFactoryRunStatus,
    SaveTaskDraftRequest, TaskDraftArtifact,
};
use aionui_common::{generate_id, now_ms};
use aionui_db::{DbError, IProductFactoryRepository, ProductFactoryRunRow};

use crate::task_draft::{generate_task_draft, validate_task_draft};

#[derive(Debug, thiserror::Error)]
pub enum ProductFactoryError {
    #[error("planning is blocked: {0}")]
    PlanningBlocked(String),
    #[error("planning failed: {0}")]
    PlanningFailed(String),
    #[error("product factory run not found: {0}")]
    NotFound(String),
    #[error("invalid product factory request: {0}")]
    InvalidRequest(String),
    #[error("invalid product factory state transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: ProductFactoryRunStatus,
        to: ProductFactoryRunStatus,
    },
    #[error("product factory database error: {0}")]
    Database(#[from] DbError),
    #[error("task draft revision changed")]
    RevisionConflict,
    #[error("handoff is not allowed: {0}")]
    InvalidHandoff(String),
    #[error("team preparation failed: {0}")]
    TeamPreparation(String),
    #[error("execution is blocked: {0}")]
    ExecutionBlocked(String),
    #[error("execution is blocked because cost cannot be verified")]
    ExecutionCostUnknown,
    #[error("execution dispatch outcome is uncertain")]
    ExecutionDispatchFailed,
    #[error("product factory review is blocked: {0}")]
    ReviewBlocked(String),
    #[error("product factory review failed: {0}")]
    ReviewFailed(String),
}

#[derive(Clone)]
pub struct ProductFactoryService {
    pub(crate) repository: Arc<dyn IProductFactoryRepository>,
    pub(crate) team_port: Option<Arc<dyn crate::handoff::ProductFactoryTeamPort>>,
    pub(crate) handoff_lock: Arc<tokio::sync::Mutex<()>>,
    pub(crate) execution_port: Option<Arc<dyn crate::execution::ProductFactoryExecutionPort>>,
    pub(crate) review_port: Option<Arc<dyn crate::review::ProductFactoryReviewPort>>,
    pub(crate) repricing_port: Option<Arc<dyn crate::pricing::ProductFactoryRepricingPort>>,
    pub(crate) planning_repository: Option<Arc<dyn aionui_db::IProductFactoryPlanningRepository>>,
    pub(crate) planning_port: Option<Arc<dyn crate::planning::ProductFactoryPlanningPort>>,
    pub(crate) usage_repository: Option<Arc<dyn aionui_db::IAgentUsageRepository>>,
    pub(crate) planning_workers: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
    pub(crate) managed_workspace_root: Option<std::path::PathBuf>,
    pub(crate) versioning: Option<crate::versions::Versioning>,
}

impl ProductFactoryService {
    pub fn new(repository: Arc<dyn IProductFactoryRepository>) -> Self {
        Self {
            repository,
            team_port: None,
            handoff_lock: Arc::new(tokio::sync::Mutex::new(())),
            execution_port: None,
            review_port: None,
            repricing_port: None,
            planning_repository: None,
            planning_port: None,
            usage_repository: None,
            planning_workers: Arc::new(tokio::sync::Mutex::new(Default::default())),
            managed_workspace_root: None,
            versioning: None,
        }
    }

    /// Allocate an isolated local product folder when the user omits a path.
    pub fn with_managed_workspace_root(mut self, root: std::path::PathBuf) -> Self {
        self.managed_workspace_root = Some(crate::versions::established_root(root));
        self
    }

    pub async fn create_run(
        &self,
        user_id: &str,
        request: CreateProductFactoryRunRequest,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        let name = required_text("name", request.name)?;
        let idea = required_text("idea", request.idea)?;
        let budget_usd = normalize_budget(request.budget_usd)?;
        let now = now_ms();
        let mut row = ProductFactoryRunRow {
            id: generate_id(),
            plan_revision: 1,
            user_id: user_id.to_owned(),
            name,
            idea,
            target_user: optional_text(request.target_user),
            problem: optional_text(request.problem),
            expected_output: optional_text(request.expected_output),
            workspace_path: optional_text(request.workspace_path),
            budget_usd,
            status: ProductFactoryRunStatus::Draft.as_str().to_owned(),
            interview_json: None,
            blueprint_json: None,
            task_draft_json: None,
            team_id: None,
            created_at: now,
            updated_at: now,
        };
        if !row.workspace_path.is_empty() {
            row.workspace_path = crate::versions::established_root(std::path::PathBuf::from(&row.workspace_path))
                .to_string_lossy()
                .into_owned();
        }
        let managed = row.workspace_path.is_empty() && self.managed_workspace_root.is_some();
        if let Some(root) = self.managed_workspace_root.as_ref().filter(|_| managed) {
            let prepare = || -> std::io::Result<std::path::PathBuf> {
                std::fs::create_dir_all(root)?;
                let folder = std::fs::canonicalize(root)?.join(&row.id);
                std::fs::create_dir(&folder)?;
                Ok(folder)
            };
            let folder = prepare().map_err(|error| {
                tracing::warn!(run_id = %row.id, kind = ?error.kind(), "managed product workspace preparation failed");
                ProductFactoryError::InvalidRequest(
                    "local product folder could not be prepared; choose an existing writable directory".into(),
                )
            })?;
            row.workspace_path = folder.to_string_lossy().into_owned();
        }
        if let Err(error) = self.repository.create_run(&row).await {
            if managed {
                // Only remove our new empty folder; never delete user files.
                let _ = std::fs::remove_dir(&row.workspace_path);
            }
            return Err(error.into());
        }
        tracing::info!(user_id, run_id = %row.id, status = %row.status, "product factory run created");
        Ok(to_response(row))
    }

    pub async fn list_runs(&self, user_id: &str) -> Result<Vec<ProductFactoryRunResponse>, ProductFactoryError> {
        Ok(self
            .repository
            .list_runs(user_id)
            .await?
            .into_iter()
            .map(to_response)
            .collect())
    }

    pub async fn get_run(&self, user_id: &str, run_id: &str) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        let row = self
            .repository
            .get_run(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::NotFound(run_id.to_owned()))?;
        Ok(to_response(row))
    }

    pub async fn transition_status(
        &self,
        user_id: &str,
        run_id: &str,
        target: ProductFactoryRunStatus,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let mut row = self
            .repository
            .get_run(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::NotFound(run_id.to_owned()))?;
        let current = parse_status(&row.status)?;
        // Execution states are facts established by handoff/runtime/review operations,
        // never claims a client may write through the generic status endpoint.
        if matches!(
            target,
            ProductFactoryRunStatus::HandedOff
                | ProductFactoryRunStatus::Running
                | ProductFactoryRunStatus::InReview
                | ProductFactoryRunStatus::Completed
        ) {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: target,
            });
        }
        if current != target {
            if !is_valid_transition(current, target) {
                return Err(ProductFactoryError::InvalidTransition {
                    from: current,
                    to: target,
                });
            }
            row.status = target.as_str().to_owned();
            row.updated_at = now_ms();
            row.plan_revision += 1;
            self.repository.update_run(&row).await?;
            tracing::info!(
                user_id,
                run_id,
                from = current.as_str(),
                to = target.as_str(),
                "product factory run status transitioned"
            );
        }
        Ok(to_response(row))
    }

    pub async fn save_interview(
        &self,
        user_id: &str,
        run_id: &str,
        interview: serde_json::Value,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        validate_artifact("interview", &interview)?;
        let mut row = self.load_run(user_id, run_id).await?;
        let current = parse_status(&row.status)?;
        if !matches!(
            current,
            ProductFactoryRunStatus::Draft | ProductFactoryRunStatus::Interviewing
        ) {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: ProductFactoryRunStatus::Interviewing,
            });
        }
        row.interview_json = Some(serde_json::to_string(&interview).map_err(|error| {
            ProductFactoryError::InvalidRequest(format!("interview could not be serialized: {error}"))
        })?);
        if current == ProductFactoryRunStatus::Draft {
            row.status = ProductFactoryRunStatus::Interviewing.as_str().to_owned();
        }
        row.updated_at = now_ms();
        row.plan_revision += 1;
        self.repository.update_run(&row).await?;
        Ok(to_response(row))
    }

    pub async fn confirm_interview(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let mut row = self.load_run(user_id, run_id).await?;
        let current = parse_status(&row.status)?;
        if current == ProductFactoryRunStatus::BlueprintReady {
            return Ok(to_response(row));
        }
        if current != ProductFactoryRunStatus::Interviewing {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: ProductFactoryRunStatus::BlueprintGenerating,
            });
        }
        let mut interview = row
            .interview_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .ok_or_else(|| {
                ProductFactoryError::InvalidRequest("interview must be saved before confirmation".to_owned())
            })?;
        let summary = interview
            .get("summary")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ProductFactoryError::InvalidRequest("interview summary is required".to_owned()))?
            .to_owned();
        interview["confirmed"] = serde_json::Value::Bool(true);
        row.interview_json = Some(interview.to_string());

        row.status = ProductFactoryRunStatus::BlueprintGenerating.as_str().to_owned();
        row.updated_at = now_ms();
        row.plan_revision += 1;
        self.repository.update_run(&row).await?;

        row.blueprint_json = Some(
            serde_json::to_string(&draft_blueprint(&row, &summary)).map_err(|error| {
                ProductFactoryError::InvalidRequest(format!("blueprint could not be serialized: {error}"))
            })?,
        );
        row.status = ProductFactoryRunStatus::BlueprintReady.as_str().to_owned();
        row.updated_at = now_ms();
        row.plan_revision += 1;
        self.repository.update_run(&row).await?;
        Ok(to_response(row))
    }

    pub async fn save_blueprint(
        &self,
        user_id: &str,
        run_id: &str,
        mut blueprint: serde_json::Value,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        validate_artifact("blueprint", &blueprint)?;
        let mut row = self.load_run(user_id, run_id).await?;
        let current = parse_status(&row.status)?;
        if !matches!(
            current,
            ProductFactoryRunStatus::BlueprintGenerating | ProductFactoryRunStatus::BlueprintReady
        ) {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: ProductFactoryRunStatus::BlueprintReady,
            });
        }
        if blueprint["version"] == 2 || blueprint["generated_by"] == "model" {
            blueprint = crate::planning_validation::normalize_model_blueprint(&blueprint, &row)?;
        }
        row.blueprint_json = Some(serde_json::to_string(&blueprint).map_err(|error| {
            ProductFactoryError::InvalidRequest(format!("blueprint could not be serialized: {error}"))
        })?);
        row.updated_at = now_ms();
        row.plan_revision += 1;
        self.repository.update_run(&row).await?;
        Ok(to_response(row))
    }

    pub async fn confirm_blueprint(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let mut row = self.load_run(user_id, run_id).await?;
        let current = parse_status(&row.status)?;
        if current != ProductFactoryRunStatus::BlueprintReady {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: ProductFactoryRunStatus::BlueprintReady,
            });
        }
        let mut blueprint = row
            .blueprint_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .ok_or_else(|| {
                ProductFactoryError::InvalidRequest("blueprint must be saved before confirmation".to_owned())
            })?;
        if blueprint["version"] == 2 || blueprint["generated_by"] == "model" {
            blueprint = crate::planning_validation::normalize_model_blueprint(&blueprint, &row)?;
        }
        let object = blueprint
            .as_object_mut()
            .ok_or_else(|| ProductFactoryError::InvalidRequest("blueprint must be a JSON object".to_owned()))?;
        object.insert("confirmed".to_owned(), serde_json::Value::Bool(true));
        row.blueprint_json = Some(serde_json::to_string(&blueprint).map_err(|error| {
            ProductFactoryError::InvalidRequest(format!("blueprint could not be serialized: {error}"))
        })?);
        row.updated_at = now_ms();
        row.plan_revision += 1;
        self.repository.update_run(&row).await?;
        Ok(to_response(row))
    }

    pub async fn generate_task_draft(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let mut row = self.load_run(user_id, run_id).await?;
        let current = parse_status(&row.status)?;
        if !matches!(
            current,
            ProductFactoryRunStatus::BlueprintReady | ProductFactoryRunStatus::TaskDraftReady
        ) {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: ProductFactoryRunStatus::TaskDraftGenerating,
            });
        }
        if let Some(existing) = row.task_draft_json.as_deref()
            && serde_json::from_str::<TaskDraftArtifact>(existing).is_ok()
        {
            return Ok(to_response(row));
        }
        let blueprint = row
            .blueprint_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .ok_or_else(|| {
                ProductFactoryError::InvalidRequest("blueprint must be saved before task draft generation".to_owned())
            })?;
        if !blueprint
            .get("confirmed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            return Err(ProductFactoryError::InvalidRequest(
                "blueprint must be confirmed before task draft generation".to_owned(),
            ));
        }
        let draft = generate_task_draft(&blueprint, now_ms());
        validate_task_draft(&draft, false).map_err(|error| ProductFactoryError::InvalidRequest(error.to_string()))?;
        let serialized = serde_json::to_string(&draft).map_err(|error| {
            ProductFactoryError::InvalidRequest(format!("task draft could not be serialized: {error}"))
        })?;
        let expected_json = row.task_draft_json.clone();
        row.task_draft_json = Some(serialized);
        row.status = ProductFactoryRunStatus::TaskDraftReady.as_str().to_owned();
        row.updated_at = now_ms();
        row.plan_revision += 1;
        self.repository
            .update_task_draft_if_current(&row, Some(draft.revision), expected_json.as_deref(), current.as_str())
            .await?;
        Ok(to_response(row))
    }

    pub async fn save_task_draft(
        &self,
        user_id: &str,
        run_id: &str,
        request: SaveTaskDraftRequest,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let mut row = self.load_run(user_id, run_id).await?;
        let current = parse_status(&row.status)?;
        if current != ProductFactoryRunStatus::TaskDraftReady {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: ProductFactoryRunStatus::TaskDraftReady,
            });
        }
        let mut draft = parse_task_draft(&row)?;
        if draft.confirmed || draft.revision != request.expected_revision {
            return Err(ProductFactoryError::InvalidRequest(
                "task draft revision conflict or draft is already confirmed".to_owned(),
            ));
        }
        draft.tasks = request.tasks;
        draft.revision += 1;
        draft.updated_at = now_ms();
        validate_task_draft(&draft, false).map_err(|error| ProductFactoryError::InvalidRequest(error.to_string()))?;
        validate_draft_links(&row, &draft)?;
        let expected_json = row.task_draft_json.clone();
        row.task_draft_json = Some(
            serde_json::to_string(&draft).map_err(|error| ProductFactoryError::InvalidRequest(error.to_string()))?,
        );
        row.updated_at = draft.updated_at;
        row.plan_revision += 1;
        self.repository
            .update_task_draft_if_current(
                &row,
                Some(request.expected_revision),
                expected_json.as_deref(),
                current.as_str(),
            )
            .await?;
        Ok(to_response(row))
    }

    pub async fn confirm_task_draft(
        &self,
        user_id: &str,
        run_id: &str,
        request: ConfirmTaskDraftRequest,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let mut row = self.load_run(user_id, run_id).await?;
        let current = parse_status(&row.status)?;
        if current != ProductFactoryRunStatus::TaskDraftReady {
            return Err(ProductFactoryError::InvalidTransition {
                from: current,
                to: ProductFactoryRunStatus::TaskDraftReady,
            });
        }
        let mut draft = parse_task_draft(&row)?;
        if draft.confirmed {
            return Ok(to_response(row));
        }
        if draft.revision != request.expected_revision {
            return Err(ProductFactoryError::InvalidRequest(
                "task draft revision conflict".to_owned(),
            ));
        }
        validate_task_draft(&draft, true).map_err(|error| ProductFactoryError::InvalidRequest(error.to_string()))?;
        validate_draft_links(&row, &draft)?;
        draft.confirmed = true;
        draft.updated_at = now_ms();
        let expected_json = row.task_draft_json.clone();
        row.task_draft_json = Some(
            serde_json::to_string(&draft).map_err(|error| ProductFactoryError::InvalidRequest(error.to_string()))?,
        );
        row.updated_at = draft.updated_at;
        row.plan_revision += 1;
        self.repository
            .update_task_draft_if_current(
                &row,
                Some(request.expected_revision),
                expected_json.as_deref(),
                current.as_str(),
            )
            .await?;
        Ok(to_response(row))
    }

    pub(crate) async fn load_run(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryRunRow, ProductFactoryError> {
        self.repository
            .get_run(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::NotFound(run_id.to_owned()))
    }
}

fn validate_artifact(name: &str, value: &serde_json::Value) -> Result<(), ProductFactoryError> {
    if value.is_object() {
        Ok(())
    } else {
        Err(ProductFactoryError::InvalidRequest(format!(
            "{name} must be a JSON object"
        )))
    }
}

pub(crate) fn parse_task_draft(row: &ProductFactoryRunRow) -> Result<TaskDraftArtifact, ProductFactoryError> {
    row.task_draft_json
        .as_deref()
        .ok_or_else(|| ProductFactoryError::InvalidRequest("task draft must be generated first".to_owned()))
        .and_then(|value| {
            serde_json::from_str(value)
                .map_err(|error| ProductFactoryError::InvalidRequest(format!("invalid task draft: {error}")))
        })
}

pub(crate) fn validate_draft_links(
    row: &ProductFactoryRunRow,
    draft: &TaskDraftArtifact,
) -> Result<(), ProductFactoryError> {
    if draft.version == 2 {
        let blueprint = row
            .blueprint_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .ok_or_else(|| ProductFactoryError::InvalidRequest("confirmed requirements are unavailable".into()))?;
        if blueprint["confirmed"] != true {
            return Err(ProductFactoryError::InvalidRequest(
                "blueprint must be confirmed".into(),
            ));
        }
        crate::task_draft::validate_requirement_links(draft, &blueprint)
            .map_err(|error| ProductFactoryError::InvalidRequest(error.to_string()))?;
    }
    Ok(())
}

fn draft_blueprint(row: &ProductFactoryRunRow, summary: &str) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "sections": [
            {"id": "goals", "title": "Goals", "content": summary, "confirmed": false},
            {"id": "users", "title": "Users", "content": row.target_user, "confirmed": false},
            {"id": "core_loop", "title": "Core loop", "content": row.idea, "confirmed": false},
            {"id": "modules", "title": "Modules", "content": row.expected_output, "confirmed": false},
            {"id": "risks", "title": "Risks", "content": row.problem, "confirmed": false}
        ],
        "risks": [],
        "open_questions": [],
        "generated_by": "draft",
        "confirmed": false,
        "updated_at": now_ms()
    })
}

fn required_text(field: &'static str, value: String) -> Result<String, ProductFactoryError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ProductFactoryError::InvalidRequest(format!("{field} is required")));
    }
    Ok(value)
}

fn optional_text(value: Option<String>) -> String {
    value.unwrap_or_default().trim().to_owned()
}

fn normalize_budget(value: Option<f64>) -> Result<Option<f64>, ProductFactoryError> {
    value
        .map(|budget| {
            if budget.is_finite() && budget > 0.0 {
                Ok((budget * 100.0).round() / 100.0)
            } else {
                Err(ProductFactoryError::InvalidRequest(
                    "budget_usd must be a finite positive number".to_owned(),
                ))
            }
        })
        .transpose()
}

fn parse_status(value: &str) -> Result<ProductFactoryRunStatus, ProductFactoryError> {
    value
        .parse()
        .map_err(|_| ProductFactoryError::InvalidRequest(format!("unknown persisted status: {value}")))
}

fn is_valid_transition(from: ProductFactoryRunStatus, to: ProductFactoryRunStatus) -> bool {
    matches!(
        (from, to),
        (ProductFactoryRunStatus::Draft, ProductFactoryRunStatus::Interviewing)
            | (
                ProductFactoryRunStatus::Interviewing,
                ProductFactoryRunStatus::BlueprintGenerating
            )
            | (
                ProductFactoryRunStatus::BlueprintGenerating,
                ProductFactoryRunStatus::BlueprintReady
            )
            | (
                ProductFactoryRunStatus::BlueprintReady,
                ProductFactoryRunStatus::TaskDraftGenerating
            )
            | (
                ProductFactoryRunStatus::TaskDraftGenerating,
                ProductFactoryRunStatus::TaskDraftReady
            )
            | (
                ProductFactoryRunStatus::TaskDraftReady,
                ProductFactoryRunStatus::HandedOff
            )
            | (ProductFactoryRunStatus::HandedOff, ProductFactoryRunStatus::Running)
            | (ProductFactoryRunStatus::Running, ProductFactoryRunStatus::InReview)
            | (ProductFactoryRunStatus::InReview, ProductFactoryRunStatus::Completed)
            | (ProductFactoryRunStatus::Interviewing, ProductFactoryRunStatus::Failed)
            | (
                ProductFactoryRunStatus::BlueprintGenerating,
                ProductFactoryRunStatus::Failed
            )
            | (
                ProductFactoryRunStatus::TaskDraftGenerating,
                ProductFactoryRunStatus::Failed
            )
    )
}

pub(crate) fn to_response(row: ProductFactoryRunRow) -> ProductFactoryRunResponse {
    ProductFactoryRunResponse {
        id: row.id,
        plan_revision: row.plan_revision as u64,
        name: row.name,
        idea: row.idea,
        target_user: row.target_user,
        problem: row.problem,
        expected_output: row.expected_output,
        workspace_path: row.workspace_path,
        budget_usd: row.budget_usd,
        status: parse_status(&row.status).unwrap_or(ProductFactoryRunStatus::Failed),
        interview: parse_json(row.interview_json),
        blueprint: parse_json(row.blueprint_json),
        task_draft: parse_json(row.task_draft_json),
        team_id: row.team_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn parse_json(value: Option<String>) -> Option<serde_json::Value> {
    value.and_then(|value| serde_json::from_str(&value).ok())
}

#[cfg(test)]
mod tests {
    use super::{is_valid_transition, normalize_budget};
    use aionui_api_types::ProductFactoryRunStatus;

    #[test]
    fn only_adjacent_workflow_states_are_valid() {
        assert!(is_valid_transition(
            ProductFactoryRunStatus::Draft,
            ProductFactoryRunStatus::Interviewing
        ));
        assert!(!is_valid_transition(
            ProductFactoryRunStatus::Draft,
            ProductFactoryRunStatus::TaskDraftReady
        ));
    }

    #[test]
    fn budget_is_rounded_to_cents() {
        assert_eq!(normalize_budget(Some(10.126)).unwrap(), Some(10.13));
        assert!(normalize_budget(Some(0.0)).is_err());
    }
}
