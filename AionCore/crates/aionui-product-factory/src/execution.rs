use std::sync::Arc;

use aionui_api_types::{
    ProductFactoryExecution, ProductFactoryExecutionResponse, ProductFactoryExecutionState, StartProductFactoryRequest,
};
use aionui_common::{now_ms, validate_workspace_path_availability};
use aionui_db::models::ProductFactoryExecutionRow;

use crate::service::parse_task_draft;
use crate::{ProductFactoryError, ProductFactoryService, validate_task_draft};

pub struct ProductFactoryDispatchReceipt {
    pub message_id: String,
    pub team_run_id: String,
}

/// Composition binds this port to the existing Team user-message path.
#[async_trait::async_trait]
pub trait ProductFactoryExecutionPort: Send + Sync {
    /// Read-only admission checks must fail closed, before reserving a dispatch.
    async fn check_start(&self, user_id: &str, team_id: &str) -> Result<(), ProductFactoryError>;
    async fn enqueue(
        &self,
        user_id: &str,
        team_id: &str,
        task_id: &str,
        content: &str,
    ) -> Result<ProductFactoryDispatchReceipt, ProductFactoryError>;

    /// Dispatch an explicit user-approved retry for a task returned for changes.
    async fn resume(
        &self,
        user_id: &str,
        team_id: &str,
        task_id: &str,
    ) -> Result<ProductFactoryDispatchReceipt, ProductFactoryError>;
}

impl ProductFactoryService {
    pub fn with_execution_port(mut self, port: Arc<dyn ProductFactoryExecutionPort>) -> Self {
        self.execution_port = Some(port);
        self
    }

    pub async fn get_execution(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryExecutionResponse, ProductFactoryError> {
        self.get_run(user_id, run_id).await?;
        Ok(ProductFactoryExecutionResponse {
            execution: self
                .repository
                .get_execution(user_id, run_id)
                .await?
                .map(execution_response)
                .transpose()?,
        })
    }

    pub async fn start_run(
        &self,
        user_id: &str,
        run_id: &str,
        request: StartProductFactoryRequest,
    ) -> Result<ProductFactoryExecutionResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let row = self
            .repository
            .get_run(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::NotFound(run_id.into()))?;
        let draft = parse_task_draft(&row)?;
        if draft.revision != request.expected_revision {
            return Err(ProductFactoryError::RevisionConflict);
        }
        // A durable reservation wins across processes and response-loss retries.
        // Never resend an uncertain attempt: Team enqueue may already have persisted.
        if self.repository.get_execution(user_id, run_id).await?.is_some() {
            return self.get_execution(user_id, run_id).await;
        }
        if row.status != "handed_off"
            || !draft.confirmed
            || !matches!(
                (draft.version, draft.generated_by.as_str()),
                (1, "draft") | (2, "model")
            )
        {
            return Err(blocked("run must have a confirmed, committed handoff"));
        }
        validate_task_draft(&draft, true).map_err(|_| blocked("confirmed task graph is invalid"))?;
        crate::service::validate_draft_links(&row, &draft)?;
        let blueprint: serde_json::Value = row
            .blueprint_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .ok_or_else(|| blocked("confirmed blueprint is missing"))?;
        if blueprint.get("confirmed").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(blocked("blueprint must be confirmed before execution"));
        }
        let team_id = row
            .team_id
            .as_deref()
            .ok_or_else(|| blocked("handoff Team is missing"))?;
        let team = self
            .repository
            .get_handoff_team(user_id, team_id)
            .await?
            .ok_or_else(|| blocked("handoff Team is unavailable"))?;
        let agents: Vec<serde_json::Value> =
            serde_json::from_str(&team.agents).map_err(|_| blocked("Team member configuration is invalid"))?;
        if agents.len() != 1
            || !matches!(agents[0]["role"].as_str(), Some("lead" | "leader"))
            || agents[0]["slot_id"].as_str().is_none_or(str::is_empty)
            || agents[0]["slot_id"].as_str() != team.lead_agent_id.as_deref()
            || agents[0]["conversation_id"].as_str().is_none_or(str::is_empty)
        {
            return Err(blocked("this MVP dispatch requires exactly one configured Team leader"));
        }
        if team.workspace != row.workspace_path {
            return Err(blocked("Team workspace changed after handoff"));
        }
        validate_workspace_path_availability(&row.workspace_path)
            .map_err(|_| blocked("workspace directory is unavailable"))?;
        let first = draft
            .tasks
            .iter()
            .find(|task| task.blocked_by.is_empty())
            .ok_or_else(|| blocked("no dependency-free task is available"))?;
        let task_id = format!("{team_id}-{}", first.id);
        let task = self
            .repository
            .get_handoff_task(user_id, team_id, &task_id)
            .await?
            .ok_or_else(|| blocked("handoff task is unavailable"))?;
        let metadata: serde_json::Value = task
            .metadata
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .ok_or_else(|| blocked("handoff task provenance is missing"))?;
        let dependencies: Vec<String> =
            serde_json::from_str(&task.blocked_by).map_err(|_| blocked("task dependencies are invalid"))?;
        if task.status != "pending"
            || task.owner.is_some()
            || !dependencies.is_empty()
            || task.subject != first.title
            || task.description.as_deref() != Some(first.description.as_str())
            || metadata["product_factory"]["run_id"].as_str() != Some(run_id)
            || metadata["product_factory"]["revision"].as_u64() != Some(draft.revision)
            || metadata["product_factory"]["draft_task_id"].as_str() != Some(first.id.as_str())
            || metadata["product_factory"]["acceptance_criteria"] != serde_json::json!(first.acceptance_criteria)
        {
            return Err(blocked(
                "first task changed or has already started; inspect Team before continuing",
            ));
        }
        let prompt = format!(
            "Product Factory first-task dispatch. Run: {run_id}\nTask: {task_id}\n\
             Product idea: {}\nTarget users: {}\nExpected output: {}\n\
             Approved blueprint: {blueprint}\n\
             Task title: {}\nTask description: {}\nAcceptance criteria: {}\n\
             Implement ONLY the assigned task in the user-approved workspace: {}. Use the existing Team task tools \
             to keep the task updated and submit it for human review. Do not approve your own work, start other \
             tasks, delegate to other agents, create paid cloud resources, or expose secrets. Run relevant \
             checks. Report changed files, test evidence and blockers in the task description and acceptance files. team_send_message sends only to other team members, not the human user; this single-leader workflow requires no broadcast. The workspace is not a security sandbox.",
            row.idea,
            row.target_user,
            row.expected_output,
            first.title,
            first.description,
            serde_json::to_string(&first.acceptance_criteria).map_err(|_| blocked("task criteria are invalid"))?,
            row.workspace_path,
        );
        if prompt.len() > 32_768 {
            return Err(blocked("execution brief exceeds the supported size"));
        }
        let port = self
            .execution_port
            .as_ref()
            .ok_or_else(|| blocked("Team execution adapter is unavailable"))?;
        port.check_start(user_id, team_id).await?;
        if !self.repository.reserve_execution(&row, &team, &task, now_ms()).await? {
            return self.get_execution(user_id, run_id).await;
        }
        tracing::info!(run_id, team_id, task_id, "product factory first-task dispatch reserved");
        match port.enqueue(user_id, team_id, &task.id, &prompt).await {
            Ok(receipt) if !receipt.message_id.is_empty() && !receipt.team_run_id.is_empty() => {
                self.repository
                    .finish_execution(
                        user_id,
                        run_id,
                        "enqueued",
                        Some(&receipt.message_id),
                        Some(&receipt.team_run_id),
                        now_ms(),
                    )
                    .await?;
                tracing::info!(run_id, team_id, message_id = %receipt.message_id, "product factory dispatch enqueued; Agent outcome unverified");
            }
            _ => {
                self.repository
                    .finish_execution(user_id, run_id, "uncertain", None, None, now_ms())
                    .await?;
                tracing::warn!(
                    run_id,
                    team_id,
                    "product factory dispatch outcome uncertain; automatic resend blocked"
                );
            }
        }
        self.get_execution(user_id, run_id).await
    }
}

fn blocked(message: &str) -> ProductFactoryError {
    ProductFactoryError::ExecutionBlocked(message.into())
}

fn execution_response(row: ProductFactoryExecutionRow) -> Result<ProductFactoryExecution, ProductFactoryError> {
    let state = match row.state.as_str() {
        "pending" => ProductFactoryExecutionState::Pending,
        "enqueued" => ProductFactoryExecutionState::Enqueued,
        "uncertain" => ProductFactoryExecutionState::Uncertain,
        _ => return Err(blocked("persisted execution receipt is invalid")),
    };
    Ok(ProductFactoryExecution {
        state,
        team_id: row.team_id,
        task_id: row.task_id,
        message_id: row.message_id,
        team_run_id: row.team_run_id,
        requested_at: row.requested_at,
        updated_at: row.updated_at,
    })
}
