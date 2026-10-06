use std::sync::Arc;

use aionui_api_types::{
    ProductFactoryExecutionResponse, ProductFactoryNextTask, ProductFactoryTaskReviewResponse, TeamTaskResponse,
    TeamTaskReviewRequest, TeamTaskUsageResponse, TeamTaskWorkspaceResponse, TeamUsageSummaryResponse,
};
use aionui_common::now_ms;
use aionui_db::models::TeamTaskRow;

use crate::service::{parse_task_draft, to_response};
use crate::{ProductFactoryError, ProductFactoryService};

#[derive(Debug, Clone)]
pub struct ProductFactoryTaskReviewSnapshot {
    pub task: TeamTaskResponse,
    pub workspace: TeamTaskWorkspaceResponse,
    pub usage: Vec<TeamTaskUsageResponse>,
    pub usage_summary: TeamUsageSummaryResponse,
}

#[async_trait::async_trait]
pub trait ProductFactoryReviewPort: Send + Sync {
    async fn get_usage_summary(
        &self,
        _user_id: &str,
        _team_id: &str,
    ) -> Result<TeamUsageSummaryResponse, ProductFactoryError> {
        Err(ProductFactoryError::ReviewFailed(
            "Team usage adapter is unavailable".into(),
        ))
    }
    async fn get_task_snapshot(
        &self,
        user_id: &str,
        team_id: &str,
        task_id: &str,
    ) -> Result<ProductFactoryTaskReviewSnapshot, ProductFactoryError>;

    async fn review_task(
        &self,
        user_id: &str,
        team_id: &str,
        task_id: &str,
        request: TeamTaskReviewRequest,
    ) -> Result<TeamTaskResponse, ProductFactoryError>;
}

impl ProductFactoryService {
    pub fn with_review_port(mut self, port: Arc<dyn ProductFactoryReviewPort>) -> Self {
        self.review_port = Some(port);
        self
    }

    pub async fn get_review(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryTaskReviewResponse, ProductFactoryError> {
        let mut row = self.load_run(user_id, run_id).await?;
        let team_id = row
            .team_id
            .clone()
            .ok_or_else(|| ProductFactoryError::ReviewBlocked("run has not been handed off to Team".into()))?;
        let execution = self
            .repository
            .get_execution(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::ReviewBlocked("the task has not been dispatched".into()))?;
        let port = self
            .review_port
            .as_ref()
            .ok_or_else(|| ProductFactoryError::ReviewFailed("Team review adapter is unavailable".into()))?;
        let snapshot = port.get_task_snapshot(user_id, &team_id, &execution.task_id).await?;
        let draft = parse_task_draft(&row)?;
        // An enqueue receipt alone does not prove a model started. A task-
        // scoped runtime usage report does, so reflect the observed workflow.
        if snapshot.task.status == "in_progress" && row.status == "handed_off" && !snapshot.usage.is_empty() {
            let _ = self
                .repository
                .set_execution_run_status(user_id, run_id, "handed_off", "running", now_ms())
                .await?;
            row = self.load_run(user_id, run_id).await?;
        }
        if snapshot.task.status == "in_review" && row.status != "in_review" {
            let _ = self
                .repository
                .set_execution_run_status(user_id, run_id, &row.status, "in_review", now_ms())
                .await?;
            row = self.load_run(user_id, run_id).await?;
        }
        let current_draft_task = draft_task_for_team_task(&team_id, &draft, &execution.task_id);
        let next_task = if snapshot.task.status == "completed" {
            self.find_next_ready_task(user_id, &team_id, &draft).await?
        } else {
            None
        };
        let execution = self.get_execution(user_id, run_id).await?.execution;
        Ok(ProductFactoryTaskReviewResponse {
            run: to_response(row),
            task: snapshot.task,
            workspace: snapshot.workspace,
            usage: snapshot.usage,
            usage_summary: snapshot.usage_summary,
            acceptance_criteria: current_draft_task
                .map(|task| task.acceptance_criteria.clone())
                .unwrap_or_default(),
            next_task: next_task.map(|(task, _)| ProductFactoryNextTask {
                id: task.id,
                title: task.title,
                acceptance_criteria: task.acceptance_criteria,
            }),
            execution,
        })
    }

    pub async fn review(
        &self,
        user_id: &str,
        run_id: &str,
        request: TeamTaskReviewRequest,
    ) -> Result<ProductFactoryTaskReviewResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let row = self.load_run(user_id, run_id).await?;
        let team_id = row
            .team_id
            .as_deref()
            .ok_or_else(|| ProductFactoryError::ReviewBlocked("run has not been handed off to Team".into()))?;
        let execution = self
            .repository
            .get_execution(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::ReviewBlocked("the task has not been dispatched".into()))?;
        let port = self
            .review_port
            .as_ref()
            .ok_or_else(|| ProductFactoryError::ReviewFailed("Team review adapter is unavailable".into()))?;
        let reviewed = port.review_task(user_id, team_id, &execution.task_id, request).await?;
        let draft = parse_task_draft(&row)?;
        let next_status = if reviewed.status == "completed" {
            let next_ready = self.find_next_ready_task(user_id, team_id, &draft).await?.is_some();
            let unfinished = self.find_unfinished_task(user_id, team_id, &draft).await?.is_some();
            if next_ready || unfinished {
                "running"
            } else {
                "completed"
            }
        } else {
            "running"
        };
        if row.status != next_status {
            let _ = self
                .repository
                .set_execution_run_status(user_id, run_id, &row.status, next_status, now_ms())
                .await?;
        }
        self.get_review(user_id, run_id).await
    }

    /// Explicitly continue execution after a human decision. Approval advances
    /// to the next ready task; request-changes retries the same task. Neither
    /// path is called automatically by the review read endpoint.
    pub async fn continue_run(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryExecutionResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        let row = self.load_run(user_id, run_id).await?;
        let team_id = row
            .team_id
            .as_deref()
            .ok_or_else(|| ProductFactoryError::ExecutionBlocked("run has not been handed off to Team".into()))?;
        let execution = self
            .repository
            .get_execution(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::ExecutionBlocked("the task has not been dispatched".into()))?;
        let current = self
            .repository
            .get_handoff_task(user_id, team_id, &execution.task_id)
            .await?
            .ok_or_else(|| ProductFactoryError::ExecutionBlocked("current Team task is unavailable".into()))?;
        let port = self
            .execution_port
            .as_ref()
            .ok_or_else(|| ProductFactoryError::ExecutionBlocked("Team execution adapter is unavailable".into()))?;

        if current.status == "in_progress" {
            if execution.state != "enqueued" {
                return Err(ProductFactoryError::ExecutionBlocked(
                    "dispatch outcome is uncertain; inspect Team before resuming".into(),
                ));
            }
            port.check_start(user_id, team_id).await?;
            if !self
                .repository
                .retry_execution(user_id, run_id, team_id, &current.id, now_ms())
                .await?
            {
                return self.get_execution(user_id, run_id).await;
            }
            let result = port.resume(user_id, team_id, &current.id).await;
            return self
                .finish_continuation(user_id, run_id, team_id, &current.id, result)
                .await;
        }

        if current.status != "completed" {
            return Err(ProductFactoryError::ReviewBlocked(
                "the current task must be approved before continuing".into(),
            ));
        }
        let draft = parse_task_draft(&row)?;
        let Some((next_draft_task, next_row)) = self.find_next_ready_task(user_id, team_id, &draft).await? else {
            if let Some(task) = self.find_unfinished_task(user_id, team_id, &draft).await? {
                return Err(ProductFactoryError::ReviewBlocked(format!(
                    "task {} is not ready for dispatch",
                    task.title
                )));
            }
            let _ = self
                .repository
                .set_execution_run_status(user_id, run_id, &row.status, "completed", now_ms())
                .await?;
            return self.get_execution(user_id, run_id).await;
        };
        let prompt = continuation_prompt(
            &draft,
            &row.idea,
            &row.target_user,
            &row.expected_output,
            &row.workspace_path,
            &next_draft_task,
        );
        if prompt.len() > 32_768 {
            return Err(ProductFactoryError::ExecutionBlocked(
                "execution brief exceeds the supported size".into(),
            ));
        }
        port.check_start(user_id, team_id).await?;
        if !self
            .repository
            .advance_execution(user_id, run_id, team_id, &current.id, &next_row, now_ms())
            .await?
        {
            return self.get_execution(user_id, run_id).await;
        }
        let result = port.enqueue(user_id, team_id, &next_row.id, &prompt).await;
        self.finish_continuation(user_id, run_id, team_id, &next_row.id, result)
            .await
    }

    async fn finish_continuation(
        &self,
        user_id: &str,
        run_id: &str,
        team_id: &str,
        task_id: &str,
        result: Result<crate::execution::ProductFactoryDispatchReceipt, ProductFactoryError>,
    ) -> Result<ProductFactoryExecutionResponse, ProductFactoryError> {
        match result {
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
            }
            _ => {
                self.repository
                    .finish_execution(user_id, run_id, "uncertain", None, None, now_ms())
                    .await?;
            }
        }
        tracing::info!(
            run_id,
            team_id,
            task_id,
            "product factory continuation dispatch recorded"
        );
        self.get_execution(user_id, run_id).await
    }

    async fn find_next_ready_task(
        &self,
        user_id: &str,
        team_id: &str,
        draft: &aionui_api_types::TaskDraftArtifact,
    ) -> Result<Option<(aionui_api_types::TaskDraftTask, TeamTaskRow)>, ProductFactoryError> {
        for draft_task in &draft.tasks {
            let task_id = format!("{team_id}-{}", draft_task.id);
            let Some(row) = self.repository.get_handoff_task(user_id, team_id, &task_id).await? else {
                return Err(ProductFactoryError::ReviewBlocked("handoff task is unavailable".into()));
            };
            let blocked_by: Vec<String> = serde_json::from_str(&row.blocked_by)
                .map_err(|_| ProductFactoryError::ReviewBlocked("task dependencies are invalid".into()))?;
            if row.status == "pending" && row.owner.is_none() && blocked_by.is_empty() {
                return Ok(Some((draft_task.clone(), row)));
            }
        }
        Ok(None)
    }

    async fn find_unfinished_task(
        &self,
        user_id: &str,
        team_id: &str,
        draft: &aionui_api_types::TaskDraftArtifact,
    ) -> Result<Option<aionui_api_types::TaskDraftTask>, ProductFactoryError> {
        for draft_task in &draft.tasks {
            let task_id = format!("{team_id}-{}", draft_task.id);
            let Some(row) = self.repository.get_handoff_task(user_id, team_id, &task_id).await? else {
                return Err(ProductFactoryError::ReviewBlocked("handoff task is unavailable".into()));
            };
            if row.status != "completed" {
                return Ok(Some(draft_task.clone()));
            }
        }
        Ok(None)
    }
}

fn draft_task_for_team_task<'a>(
    team_id: &str,
    draft: &'a aionui_api_types::TaskDraftArtifact,
    task_id: &str,
) -> Option<&'a aionui_api_types::TaskDraftTask> {
    draft
        .tasks
        .iter()
        .find(|task| format!("{team_id}-{}", task.id) == task_id)
}

fn continuation_prompt(
    draft: &aionui_api_types::TaskDraftArtifact,
    idea: &str,
    target_user: &str,
    expected_output: &str,
    workspace_path: &str,
    task: &aionui_api_types::TaskDraftTask,
) -> String {
    let integration_guidance = if crate::task_draft::is_integration_task(draft, task) {
        "This is the final integration and acceptance task: inspect completed task workspaces, assemble the runnable product in the project workspace root, create START.md and ACCEPTANCE.md, and run a restart/persistence smoke test before submitting for review."
    } else {
        "Keep implementation changes scoped to the assigned task and its task workspace."
    };
    format!(
        "Product Factory continuation task.\nProduct idea: {idea}\nTarget users: {target_user}\nExpected output: {expected_output}\nProject workspace: {workspace_path}\nTask: {}\nDescription: {}\nAcceptance criteria: {}\n{integration_guidance}\nDo not approve your own work, start other tasks, or create paid cloud resources. Report changed files, test evidence and blockers in the task description and acceptance files. team_send_message sends only to other team members, not the human user; this single-leader workflow requires no broadcast. Run relevant checks and submit the task for human review.",
        task.title,
        task.description,
        serde_json::to_string(&task.acceptance_criteria).unwrap_or_else(|_| "[]".into()),
    )
}
