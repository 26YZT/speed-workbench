use crate::planning_cost::{ProductFactoryPlanningCosts, phase, read_planning_costs};
use crate::planning_validation::{planning_prompt, validate_result};
use crate::service::to_response;
use crate::{ProductFactoryError, ProductFactoryService};
use aionui_api_types::{
    ApplyProductFactoryPlanningRequest, ProductFactoryPlanningPhase, ProductFactoryPlanningResponse,
    ProductFactoryRunResponse, StartProductFactoryPlanningRequest,
};
use aionui_common::{generate_id, now_ms};
use aionui_db::models::ProductFactoryPlanningRow;
use aionui_db::{IAgentUsageRepository, IProductFactoryPlanningRepository, ProductFactoryRunRow};
use futures_util::FutureExt;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProductFactoryPlanningInvocation {
    pub user_id: String,
    pub run_id: String,
    pub attempt_id: String,
    pub phase: ProductFactoryPlanningPhase,
    pub assistant_id: String,
    pub model: String,
    pub prompt: String,
}
pub struct ProductFactoryPlanningPrepared {
    pub conversation_id: String,
    pub assistant_snapshot: serde_json::Value,
}
pub struct ProductFactoryPlanningOutcome {
    pub app_turn_id: String,
    pub completed: bool,
    pub output_complete: bool,
    pub final_text: Option<String>,
    pub admission_rejected: bool,
}
pub type ProductFactoryPlanningStarted = Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[async_trait::async_trait]
pub trait ProductFactoryPlanningPort: Send + Sync {
    async fn prepare(
        &self,
        input: &ProductFactoryPlanningInvocation,
    ) -> Result<ProductFactoryPlanningPrepared, ProductFactoryError>;
    async fn run(
        &self,
        input: &ProductFactoryPlanningInvocation,
        conversation_id: &str,
        on_started: ProductFactoryPlanningStarted,
    ) -> Result<ProductFactoryPlanningOutcome, ProductFactoryError>;
    async fn cancel(&self, user_id: &str, conversation_id: &str, app_turn_id: &str) -> Result<(), ProductFactoryError>;
}

impl ProductFactoryService {
    pub fn with_planning_repository(mut self, repository: Arc<dyn IProductFactoryPlanningRepository>) -> Self {
        self.planning_repository = Some(repository);
        self
    }
    pub fn with_planning_port(mut self, port: Arc<dyn ProductFactoryPlanningPort>) -> Self {
        self.planning_port = Some(port);
        self
    }
    pub fn with_usage_repository(mut self, repository: Arc<dyn IAgentUsageRepository>) -> Self {
        self.usage_repository = Some(repository);
        self
    }
    fn planning_repo(&self) -> Result<&Arc<dyn IProductFactoryPlanningRepository>, ProductFactoryError> {
        self.planning_repository
            .as_ref()
            .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_UNAVAILABLE".into()))
    }

    pub async fn get_planning_costs(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryPlanningCosts, ProductFactoryError> {
        self.load_run(user_id, run_id).await?;
        let Some(repo) = self.planning_repository.as_ref() else {
            return Ok(ProductFactoryPlanningCosts {
                cost_est: Some(0.0),
                ..Default::default()
            });
        };
        let usage = self
            .usage_repository
            .as_ref()
            .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_USAGE_UNAVAILABLE".into()))?;
        read_planning_costs(repo.as_ref(), usage.as_ref(), user_id, run_id).await
    }
    async fn check_planning_budget(&self, run: &ProductFactoryRunRow) -> Result<(), ProductFactoryError> {
        if self
            .planning_repo()?
            .list(&run.user_id, &run.id)
            .await?
            .iter()
            .any(|attempt| attempt.state == "uncertain")
        {
            return Err(ProductFactoryError::PlanningBlocked(
                "PLANNING_UNCERTAIN_ATTEMPT".into(),
            ));
        }
        let costs = self.get_planning_costs(&run.user_id, &run.id).await?;
        if costs.cost_unknown && run.budget_usd.is_some() {
            return Err(ProductFactoryError::PlanningBlocked("PLANNING_COST_UNKNOWN".into()));
        }
        if run
            .budget_usd
            .is_some_and(|limit| costs.cost_est.unwrap_or(0.0) >= limit)
        {
            return Err(ProductFactoryError::PlanningBlocked("PLANNING_BUDGET_EXHAUSTED".into()));
        }
        Ok(())
    }
    pub async fn planning_run_for_conversation(
        &self,
        user_id: &str,
        conversation_id: &str,
    ) -> Result<Option<String>, ProductFactoryError> {
        let Some(repo) = self.planning_repository.as_ref() else {
            return Ok(None);
        };
        Ok(repo
            .get_by_conversation(user_id, conversation_id)
            .await?
            .map(|attempt| attempt.run_id))
    }
    pub async fn check_planning_send_budget(
        &self,
        user_id: &str,
        run_id: &str,
        conversation_id: &str,
        app_turn_id: &str,
    ) -> Result<(), ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let repo = self.planning_repo()?;
        let attempt = repo
            .get_by_conversation(user_id, conversation_id)
            .await?
            .ok_or_else(|| ProductFactoryError::PlanningBlocked("PLANNING_CONVERSATION_NOT_BOUND".into()))?;
        if attempt.run_id != run_id || attempt.state != "running" {
            return Err(ProductFactoryError::PlanningBlocked(
                "PLANNING_ATTEMPT_NOT_RUNNING".into(),
            ));
        }
        if app_turn_id.is_empty() || attempt.app_turn_id.as_deref() != Some(app_turn_id) {
            return Err(ProductFactoryError::PlanningBlocked(
                "PLANNING_APP_TURN_NOT_BOUND".into(),
            ));
        }
        let run = self.load_run(user_id, run_id).await?;
        let usage = self
            .usage_repository
            .as_ref()
            .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_USAGE_UNAVAILABLE".into()))?;
        let costs =
            crate::planning_cost::read_costs(repo.as_ref(), usage.as_ref(), user_id, run_id, Some(conversation_id))
                .await?;
        if repo
            .list(user_id, run_id)
            .await?
            .iter()
            .any(|other| other.id != attempt.id && other.state == "uncertain")
        {
            return Err(ProductFactoryError::PlanningBlocked(
                "PLANNING_UNCERTAIN_ATTEMPT".into(),
            ));
        }
        if run.budget_usd.is_some() && costs.cost_unknown {
            return Err(ProductFactoryError::PlanningBlocked("PLANNING_COST_UNKNOWN".into()));
        }
        if run
            .budget_usd
            .is_some_and(|limit| costs.cost_est.unwrap_or(0.0) >= limit)
        {
            return Err(ProductFactoryError::PlanningBlocked("PLANNING_BUDGET_EXHAUSTED".into()));
        }
        Ok(())
    }
    pub async fn start_planning(
        &self,
        user_id: &str,
        run_id: &str,
        request: StartProductFactoryPlanningRequest,
    ) -> Result<ProductFactoryPlanningResponse, ProductFactoryError> {
        let run = self.load_run(user_id, run_id).await?;
        let repo = self.planning_repo()?;
        if request.idempotency_key.is_empty()
            || request.idempotency_key.len() > 128
            || !request
                .idempotency_key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            || request.assistant_id.trim().is_empty()
            || request.assistant_id.len() > 256
            || request.model.len() > 256
            || request.model.trim() != request.model
        {
            return Err(ProductFactoryError::InvalidRequest(
                "planning key, assistant or model is invalid".into(),
            ));
        }
        if let Some(existing) = repo.get_by_key(user_id, run_id, &request.idempotency_key).await? {
            if existing.phase != request.phase.as_str()
                || existing.input_plan_revision as u64 != request.expected_plan_revision
                || existing.assistant_id != request.assistant_id
                || existing.model != request.model
            {
                return Err(ProductFactoryError::PlanningBlocked(
                    "PLANNING_IDEMPOTENCY_CONFLICT".into(),
                ));
            }
            return self.get_planning(user_id, run_id, &existing.id).await;
        }
        self.ensure_version_mutable(user_id, run_id).await?;
        if run.plan_revision as u64 != request.expected_plan_revision {
            return Err(ProductFactoryError::RevisionConflict);
        }
        validate_phase(&run, request.phase)?;
        self.check_planning_budget(&run).await?;
        if self.planning_port.is_none() {
            return Err(ProductFactoryError::PlanningFailed("PLANNING_UNAVAILABLE".into()));
        }
        let input = serde_json::json!({"idea":run.idea,"target_user":run.target_user,"problem":run.problem,"expected_output":run.expected_output,
            "interview":run.interview_json.as_deref().and_then(|raw|serde_json::from_str::<serde_json::Value>(raw).ok()),
            "blueprint":run.blueprint_json.as_deref().and_then(|raw|serde_json::from_str::<serde_json::Value>(raw).ok())});
        let prompt = planning_prompt(request.phase, &input);
        if prompt.len() > 65_536 {
            return Err(ProductFactoryError::InvalidRequest(
                "planning input is too large".into(),
            ));
        }
        let now = now_ms();
        let row = ProductFactoryPlanningRow {
            id: generate_id(),
            user_id: user_id.into(),
            run_id: run_id.into(),
            phase: request.phase.as_str().into(),
            input_plan_revision: run.plan_revision,
            input_hash: format!("{:x}", Sha256::digest(input.to_string().as_bytes())),
            idempotency_key: request.idempotency_key,
            assistant_id: request.assistant_id,
            model: request.model,
            assistant_snapshot_json: None,
            conversation_id: None,
            app_turn_id: None,
            state: "reserved".into(),
            result_json: None,
            error_code: None,
            started_at: None,
            created_at: now,
            updated_at: now,
        };
        self.planning_workers.lock().await.insert(row.id.clone());
        let reserved = match repo.reserve(&run, &row).await {
            Ok(reserved) => reserved,
            Err(error) => {
                self.planning_workers.lock().await.remove(&row.id);
                return Err(error.into());
            }
        };
        if !reserved {
            self.planning_workers.lock().await.remove(&row.id);
            if let Some(existing) = repo.get_by_key(user_id, run_id, &row.idempotency_key).await? {
                if existing.input_hash != row.input_hash
                    || existing.assistant_id != row.assistant_id
                    || existing.model != row.model
                    || existing.phase != row.phase
                {
                    return Err(ProductFactoryError::PlanningBlocked(
                        "PLANNING_IDEMPOTENCY_CONFLICT".into(),
                    ));
                }
                return self.get_planning(user_id, run_id, &existing.id).await;
            }
            return Err(ProductFactoryError::PlanningBlocked(
                "PLANNING_ATTEMPT_ALREADY_ACTIVE".into(),
            ));
        }
        let response = planning_response(row.clone())?;
        tracing::info!(user_id,run_id,attempt_id=%row.id,phase=%row.phase,input_plan_revision=row.input_plan_revision,"product planning reserved");
        let service = self.clone();
        tokio::spawn(async move {
            let operation = std::panic::AssertUnwindSafe(service.execute_planning(&run, &row, prompt))
                .catch_unwind()
                .await;
            if !matches!(operation, Ok(Ok(()))) {
                let (state, code) = match operation {
                    Ok(Err(ProductFactoryError::PlanningBlocked(_))) => ("failed", "PLANNING_BUDGET_BLOCKED"),
                    _ => ("uncertain", "PLANNING_INVOCATION_UNCERTAIN"),
                };
                if let Some(repo) = service.planning_repository.as_ref() {
                    let _ = repo
                        .finish(&row.user_id, &row.id, state, None, Some(code), now_ms())
                        .await;
                }
                tracing::warn!(run_id=%row.run_id,attempt_id=%row.id,state,"product planning did not settle normally");
            }
            service.planning_workers.lock().await.remove(&row.id);
        });
        Ok(response)
    }
    async fn execute_planning(
        &self,
        run: &ProductFactoryRunRow,
        row: &ProductFactoryPlanningRow,
        prompt: String,
    ) -> Result<(), ProductFactoryError> {
        let repo = self.planning_repo()?;
        if !repo.claim_preparing(&row.user_id, &row.id, now_ms()).await? {
            return Ok(());
        }
        let current = self.load_run(&row.user_id, &row.run_id).await?;
        if current.plan_revision != row.input_plan_revision || current.team_id.is_some() {
            repo.finish(
                &row.user_id,
                &row.id,
                "failed",
                None,
                Some("PLANNING_INPUT_CHANGED"),
                now_ms(),
            )
            .await?;
            return Ok(());
        }
        self.check_planning_budget(&current).await?;
        let port = self
            .planning_port
            .as_ref()
            .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_UNAVAILABLE".into()))?;
        let input = ProductFactoryPlanningInvocation {
            user_id: row.user_id.clone(),
            run_id: row.run_id.clone(),
            attempt_id: row.id.clone(),
            phase: phase(&row.phase)?,
            assistant_id: row.assistant_id.clone(),
            model: row.model.clone(),
            prompt,
        };
        let prepared = match port.prepare(&input).await {
            Ok(prepared) => prepared,
            Err(_) => {
                repo.finish(
                    &row.user_id,
                    &row.id,
                    "failed",
                    None,
                    Some("PLANNING_PREPARATION_FAILED"),
                    now_ms(),
                )
                .await?;
                return Ok(());
            }
        };
        if !repo
            .bind_conversation(
                &row.user_id,
                &row.id,
                &prepared.conversation_id,
                &prepared.assistant_snapshot.to_string(),
                now_ms(),
            )
            .await?
        {
            return Err(ProductFactoryError::PlanningFailed("PLANNING_RECEIPT_LOST".into()));
        }
        if !repo.mark_running(&row.user_id, &row.id, now_ms()).await? {
            return Ok(());
        }
        tracing::info!(run_id=%row.run_id,attempt_id=%row.id,conversation_id=%prepared.conversation_id,"product planning invocation beginning");
        let started_repo = repo.clone();
        let user = row.user_id.clone();
        let id = row.id.clone();
        let on_started: ProductFactoryPlanningStarted = Arc::new(move |turn| {
            let repo = started_repo.clone();
            let user = user.clone();
            let id = id.clone();
            Box::pin(async move {
                if let Err(error) = repo.bind_app_turn(&user, &id, &turn, now_ms()).await {
                    tracing::warn!(attempt_id=%id,%error,"planning app turn binding failed");
                }
            })
        });
        let outcome = port.run(&input, &prepared.conversation_id, on_started).await?;
        if outcome.admission_rejected {
            repo.finish(
                &row.user_id,
                &row.id,
                "failed",
                None,
                Some("USER_MODEL_SEND_ADMISSION_REJECTED"),
                now_ms(),
            )
            .await?;
            return Ok(());
        }
        repo.bind_app_turn(&row.user_id, &row.id, &outcome.app_turn_id, now_ms())
            .await?;
        let result = if outcome.completed && outcome.output_complete {
            outcome
                .final_text
                .as_deref()
                .ok_or("PLANNING_OUTPUT_INCOMPLETE")
                .and_then(|text| validate_result(input.phase, text, run, now_ms()))
        } else {
            Err("PLANNING_OUTPUT_INCOMPLETE")
        };
        match result {
            Ok(value) => {
                repo.finish(
                    &row.user_id,
                    &row.id,
                    "candidate_ready",
                    Some(&value.to_string()),
                    None,
                    now_ms(),
                )
                .await?;
            }
            Err(code) => {
                repo.finish(&row.user_id, &row.id, "failed", None, Some(code), now_ms())
                    .await?;
            }
        }
        Ok(())
    }
    pub async fn get_planning(
        &self,
        user_id: &str,
        run_id: &str,
        id: &str,
    ) -> Result<ProductFactoryPlanningResponse, ProductFactoryError> {
        self.load_run(user_id, run_id).await?;
        let repo = self.planning_repo()?;
        let mut row = repo
            .get(user_id, run_id, id)
            .await?
            .ok_or_else(|| ProductFactoryError::NotFound(id.into()))?;
        if matches!(row.state.as_str(), "reserved" | "preparing" | "running")
            && !self.planning_workers.lock().await.contains(id)
        {
            repo.mark_uncertain(user_id, id, now_ms()).await?;
            row = repo
                .get(user_id, run_id, id)
                .await?
                .ok_or_else(|| ProductFactoryError::NotFound(id.into()))?;
        }
        planning_response(row)
    }
    pub async fn list_planning(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<Vec<ProductFactoryPlanningResponse>, ProductFactoryError> {
        self.load_run(user_id, run_id).await?;
        let rows = self.planning_repo()?.list(user_id, run_id).await?;
        let mut result = Vec::new();
        for row in rows {
            result.push(self.get_planning(user_id, run_id, &row.id).await?);
        }
        Ok(result)
    }
    pub async fn cancel_planning(
        &self,
        user_id: &str,
        run_id: &str,
        id: &str,
    ) -> Result<ProductFactoryPlanningResponse, ProductFactoryError> {
        let row = self.get_planning(user_id, run_id, id).await?;
        if row.state == "applied" {
            return Err(ProductFactoryError::PlanningBlocked("PLANNING_ALREADY_APPLIED".into()));
        }
        self.planning_repo()?.cancel(user_id, id, now_ms()).await?;
        if matches!(row.state.as_str(), "running" | "uncertain") {
            let confirmed = if let (Some(conversation), Some(turn), Some(port)) = (
                row.conversation_id.as_deref(),
                row.app_turn_id.as_deref(),
                self.planning_port.as_ref(),
            ) {
                port.cancel(user_id, conversation, turn).await.is_ok()
            } else {
                false
            };
            if !confirmed {
                self.planning_repo()?
                    .finish(
                        user_id,
                        id,
                        "cancelled",
                        None,
                        Some("PLANNING_CANCELLATION_UNCONFIRMED"),
                        now_ms(),
                    )
                    .await?;
                tracing::warn!(attempt_id = id, "planning cancellation could not be confirmed");
            }
        }
        self.get_planning(user_id, run_id, id).await
    }
    pub async fn apply_planning(
        &self,
        user_id: &str,
        run_id: &str,
        id: &str,
        request: ApplyProductFactoryPlanningRequest,
    ) -> Result<ProductFactoryRunResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let mut run = self.load_run(user_id, run_id).await?;
        let attempt = self.get_planning(user_id, run_id, id).await?;
        if run.plan_revision as u64 != request.expected_plan_revision
            || attempt.input_plan_revision != request.expected_plan_revision
        {
            return Err(ProductFactoryError::RevisionConflict);
        }
        if attempt.state != "candidate_ready" {
            return Err(ProductFactoryError::PlanningBlocked(
                "PLANNING_CANDIDATE_NOT_READY".into(),
            ));
        }
        validate_phase(&run, attempt.phase)?;
        let candidate = attempt
            .candidate
            .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_RESULT_MISSING".into()))?;
        if candidate.get("confirmed").and_then(serde_json::Value::as_bool) != Some(false) {
            return Err(ProductFactoryError::PlanningFailed("PLANNING_RESULT_INVALID".into()));
        }
        match attempt.phase {
            ProductFactoryPlanningPhase::Interview => {
                run.interview_json = Some(candidate.to_string());
                run.blueprint_json = None;
                run.task_draft_json = None;
                run.status = "interviewing".into();
            }
            ProductFactoryPlanningPhase::Blueprint => {
                run.blueprint_json = Some(candidate.to_string());
                run.task_draft_json = None;
                run.status = "blueprint_ready".into();
            }
            ProductFactoryPlanningPhase::TaskGraph => {
                run.task_draft_json = Some(candidate.to_string());
                run.status = "task_draft_ready".into();
            }
        }
        if !self
            .planning_repo()?
            .apply_candidate(&run, id, run.plan_revision, now_ms())
            .await?
        {
            return Err(ProductFactoryError::RevisionConflict);
        }
        run.plan_revision += 1;
        run.updated_at = now_ms();
        Ok(to_response(run))
    }
}

fn validate_phase(run: &ProductFactoryRunRow, phase: ProductFactoryPlanningPhase) -> Result<(), ProductFactoryError> {
    let blocked = || ProductFactoryError::PlanningBlocked("PLANNING_PHASE_NOT_EDITABLE".into());
    if run.team_id.is_some() {
        return Err(blocked());
    }
    let artifact = match phase {
        ProductFactoryPlanningPhase::Interview => run.interview_json.as_deref(),
        ProductFactoryPlanningPhase::Blueprint => run.blueprint_json.as_deref(),
        ProductFactoryPlanningPhase::TaskGraph => run.task_draft_json.as_deref(),
    };
    if artifact
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .is_some_and(|value| value["confirmed"] == true)
    {
        return Err(blocked());
    }
    match phase {
        ProductFactoryPlanningPhase::Interview if !matches!(run.status.as_str(), "draft" | "interviewing") => {
            return Err(blocked());
        }
        ProductFactoryPlanningPhase::Blueprint
            if !matches!(run.status.as_str(), "blueprint_ready" | "blueprint_generating") =>
        {
            return Err(blocked());
        }
        ProductFactoryPlanningPhase::TaskGraph => {
            if !matches!(run.status.as_str(), "blueprint_ready" | "task_draft_ready") {
                return Err(blocked());
            }
            let blueprint = run
                .blueprint_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
                .ok_or_else(blocked)?;
            if blueprint["confirmed"] != true
                || blueprint
                    .get("requirements")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(|items| items.is_empty())
            {
                return Err(blocked());
            }
        }
        _ => {}
    }
    Ok(())
}
fn planning_response(row: ProductFactoryPlanningRow) -> Result<ProductFactoryPlanningResponse, ProductFactoryError> {
    Ok(ProductFactoryPlanningResponse {
        id: row.id,
        run_id: row.run_id,
        phase: phase(&row.phase)?,
        input_plan_revision: row.input_plan_revision as u64,
        input_hash: row.input_hash,
        idempotency_key: row.idempotency_key,
        assistant_id: row.assistant_id,
        model: row.model,
        conversation_id: row.conversation_id,
        app_turn_id: row.app_turn_id,
        state: row.state,
        candidate: row.result_json.and_then(|raw| serde_json::from_str(&raw).ok()),
        error_code: row.error_code,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
