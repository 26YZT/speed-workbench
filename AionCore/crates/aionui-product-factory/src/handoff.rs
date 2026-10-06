use std::{collections::BTreeMap, path::Path, sync::Arc};

use aionui_api_types::{
    CreateTeamRequest, HandoffProductFactoryRequest, ProductFactoryHandoffResponse, TaskDraftArtifact,
};
use aionui_common::validate_workspace_path_availability;
use aionui_db::{
    ProductFactoryRunRow,
    models::{TeamRow, TeamTaskRow},
};

use crate::service::{parse_task_draft, to_response};
use crate::{ProductFactoryError, ProductFactoryService, validate_task_draft};

/// Composition-layer adapter to existing Team provisioning, not an execution port.
/// Preparation must not persist a Team, send messages, or start Agent runtime.
/// A failed preparation must clean up its partially created conversations.
#[async_trait::async_trait]
pub trait ProductFactoryTeamPort: Send + Sync {
    async fn prepare_team(
        &self,
        user_id: &str,
        team_id: &str,
        request: CreateTeamRequest,
    ) -> Result<TeamRow, ProductFactoryError>;
    async fn discard_team(&self, user_id: &str, team: &TeamRow) -> Result<(), ProductFactoryError>;
    fn committed(&self, _user_id: &str, _team: &TeamRow) {}
}

impl ProductFactoryService {
    pub fn with_team_port(mut self, port: Arc<dyn ProductFactoryTeamPort>) -> Self {
        self.team_port = Some(port);
        self
    }

    /// Convert one confirmed, current task draft to one durable Team graph.
    pub async fn handoff(
        &self,
        user_id: &str,
        run_id: &str,
        request: HandoffProductFactoryRequest,
    ) -> Result<ProductFactoryHandoffResponse, ProductFactoryError> {
        self.ensure_version_mutable(user_id, run_id).await?;
        // Local serialization avoids unnecessary duplicate provisioning. The SQL
        // snapshot predicate and stable IDs remain authoritative across processes.
        let _guard = self.handoff_lock.lock().await;
        let row = self
            .repository
            .get_run(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::NotFound(run_id.into()))?;
        let draft = parse_task_draft(&row)?;
        if !matches!(
            (draft.version, draft.generated_by.as_str()),
            (1, "draft") | (2, "model")
        ) || !draft.confirmed
            || draft.revision == 0
        {
            return Err(ProductFactoryError::InvalidHandoff(
                "task draft must be confirmed with supported source/version".into(),
            ));
        }
        if draft.revision != request.expected_revision {
            return Err(ProductFactoryError::RevisionConflict);
        }
        validate_task_draft(&draft, true).map_err(|error| ProductFactoryError::InvalidHandoff(error.to_string()))?;
        crate::service::validate_draft_links(&row, &draft)?;
        if row.team_id.is_some() {
            return self.handoff_result(row, &draft).await;
        }
        if row.status != "task_draft_ready" {
            return Err(ProductFactoryError::InvalidHandoff(
                "run is not ready for handoff".into(),
            ));
        }
        let workspace = &row.workspace_path;
        if workspace.trim() != workspace || !Path::new(workspace).is_absolute() || workspace.contains('\0') {
            return Err(ProductFactoryError::InvalidHandoff(
                "workspace must be an absolute directory path".into(),
            ));
        }
        validate_workspace_path_availability(workspace)
            .map_err(|_| ProductFactoryError::InvalidHandoff("workspace directory is unavailable".into()))?;
        let agents = request.agents;
        if agents.is_empty()
            || agents
                .iter()
                .filter(|agent| matches!(agent.role.as_str(), "lead" | "leader"))
                .count()
                != 1
            || agents.iter().any(|agent| {
                agent.name.trim().is_empty()
                    || agent.assistant_id.as_deref().is_none_or(|id| id.trim().is_empty())
                    || !matches!(agent.role.as_str(), "lead" | "leader" | "teammate")
                    || agent.conversation_id.as_deref().is_some_and(|id| !id.trim().is_empty())
            })
        {
            return Err(ProductFactoryError::InvalidHandoff(
                "select configured assistants/models and exactly one leader".into(),
            ));
        }
        let port = self
            .team_port
            .as_ref()
            .ok_or_else(|| ProductFactoryError::TeamPreparation("Team adapter is not configured".into()))?;
        let team_id = format!("factory-{}-r{}", row.id, draft.revision);
        let tasks = task_rows(&team_id, &row.id, &draft, aionui_common::now_ms())?;
        let team = port
            .prepare_team(
                user_id,
                &team_id,
                CreateTeamRequest {
                    name: row.name.clone(),
                    agents,
                    workspace: Some(workspace.clone()),
                },
            )
            .await?;
        if let Err(error) = self.repository.complete_handoff(&row, &team, &tasks).await {
            // Delete only this preparation's member conversations, never another
            // process's committed Team or its graph.
            port.discard_team(user_id, &team).await?;
            let current = self
                .repository
                .get_run(user_id, run_id)
                .await?
                .ok_or_else(|| ProductFactoryError::NotFound(run_id.into()))?;
            if current.team_id.is_some() {
                let current_draft = parse_task_draft(&current)?;
                if current_draft.revision != request.expected_revision {
                    return Err(ProductFactoryError::RevisionConflict);
                }
                return self.handoff_result(current, &current_draft).await;
            }
            return Err(ProductFactoryError::Database(error));
        }
        port.committed(user_id, &team);
        let committed = self
            .repository
            .get_run(user_id, run_id)
            .await?
            .ok_or_else(|| ProductFactoryError::NotFound(run_id.into()))?;
        self.handoff_result(committed, &draft).await
    }

    async fn handoff_result(
        &self,
        row: ProductFactoryRunRow,
        draft: &TaskDraftArtifact,
    ) -> Result<ProductFactoryHandoffResponse, ProductFactoryError> {
        let team_id = row
            .team_id
            .as_ref()
            .ok_or_else(|| ProductFactoryError::InvalidHandoff("handoff result is missing".into()))?
            .clone();
        if !matches!(
            row.status.as_str(),
            "handed_off" | "running" | "in_review" | "completed"
        ) || self
            .repository
            .get_handoff_team(&row.user_id, &team_id)
            .await?
            .is_none()
        {
            return Err(ProductFactoryError::InvalidHandoff(
                "previous handoff Team is unavailable; it will not be recreated".into(),
            ));
        }
        Ok(ProductFactoryHandoffResponse {
            task_id_map: task_id_map(&team_id, draft),
            run: to_response(row),
            team_id,
        })
    }
}

fn task_id_map(team_id: &str, draft: &TaskDraftArtifact) -> BTreeMap<String, String> {
    draft
        .tasks
        .iter()
        .map(|task| (task.id.clone(), format!("{team_id}-{}", task.id)))
        .collect()
}

fn task_rows(
    team_id: &str,
    run_id: &str,
    draft: &TaskDraftArtifact,
    now: i64,
) -> Result<Vec<TeamTaskRow>, ProductFactoryError> {
    let map = task_id_map(team_id, draft);
    draft
        .tasks
        .iter()
        .map(|task| {
            let dependencies: Vec<_> = task.blocked_by.iter().map(|id| map[id].clone()).collect();
            let blocks: Vec<_> = draft
                .tasks
                .iter()
                .filter(|other| other.blocked_by.contains(&task.id))
                .map(|other| map[&other.id].clone())
                .collect();
            Ok(TeamTaskRow {
                id: map[&task.id].clone(),
                team_id: team_id.into(),
                subject: task.title.clone(),
                description: Some(task.description.clone()),
                status: "pending".into(),
                owner: None,
                blocked_by: serde_json::to_string(&dependencies).map_err(serialization_error)?,
                blocks: serde_json::to_string(&blocks).map_err(serialization_error)?,
                metadata: Some(
                    serde_json::json!({"product_factory":{
                        "run_id":run_id, "revision":draft.revision, "draft_task_id":task.id,
                        "type":task.task_type, "acceptance_criteria":task.acceptance_criteria,
                        "suggested_role":task.suggested_role, "effort":task.effort,
                        "version":draft.version,"execution_scope":task.execution_scope,"requirement_ids":task.requirement_ids,
                    }})
                    .to_string(),
                ),
                created_at: now,
                updated_at: now,
            })
        })
        .collect()
}

fn serialization_error(error: serde_json::Error) -> ProductFactoryError {
    ProductFactoryError::InvalidHandoff(error.to_string())
}
