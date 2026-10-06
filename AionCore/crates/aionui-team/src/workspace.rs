use std::fs;
use std::sync::Arc;

use aionui_common::{WorkspacePathValidationError, validate_workspace_path_availability};
use aionui_db::models::TeamRow;
use aionui_db::{ITeamRepository, UpdateTeamParams};
use tracing::warn;

use crate::error::TeamError;
use crate::provisioning::TeamConversationProvisioningPort;
use crate::types::{Team, TeammateRole};

pub(crate) fn validate_create_workspace_path(workspace: &str) -> Result<String, TeamError> {
    validate_workspace_path_availability(workspace).map_err(|error| match error {
        WorkspacePathValidationError::Empty => TeamError::InvalidRequest("Workspace directory is empty".into()),
        WorkspacePathValidationError::DoesNotExist(path)
        | WorkspacePathValidationError::NotDirectory(path)
        | WorkspacePathValidationError::NotAccessible { path, .. } => TeamError::WorkspacePathUnavailable(path),
    })
}

fn validate_runtime_workspace_path(workspace: &str) -> Result<String, TeamError> {
    validate_workspace_path_availability(workspace).map_err(|error| match error {
        WorkspacePathValidationError::Empty => TeamError::InvalidRequest("Team workspace is empty".into()),
        WorkspacePathValidationError::DoesNotExist(path)
        | WorkspacePathValidationError::NotDirectory(path)
        | WorkspacePathValidationError::NotAccessible { path, .. } => TeamError::WorkspacePathRuntimeUnavailable(path),
    })
}

fn usable_runtime_workspace(workspace: &str) -> Option<String> {
    validate_runtime_workspace_path(workspace).ok()
}

/// Create and return the durable working directory for one task.
///
/// The task id is deliberately constrained to a single filesystem component,
/// and both the `.tasks` root and the final directory are canonicalized after
/// creation. This keeps task workspaces below the team workspace even when a
/// pre-existing symlink is present in the workspace.
pub(crate) fn resolve_task_dir(workspace: &str, task_id: &str) -> Result<String, TeamError> {
    validate_runtime_workspace_path(workspace)?;
    if task_id.is_empty()
        || !task_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(TeamError::InvalidRequest("Task id is not a safe directory name".into()));
    }

    let root = fs::canonicalize(workspace).map_err(|_| TeamError::WorkspacePathRuntimeUnavailable(workspace.into()))?;
    let tasks_root = root.join(".tasks");
    fs::create_dir_all(&tasks_root)
        .map_err(|_| TeamError::WorkspacePathRuntimeUnavailable(tasks_root.display().to_string()))?;
    let tasks_root = fs::canonicalize(&tasks_root)
        .map_err(|_| TeamError::WorkspacePathRuntimeUnavailable(tasks_root.display().to_string()))?;
    if !tasks_root.starts_with(&root) {
        return Err(TeamError::WorkspacePathRuntimeUnavailable(
            tasks_root.display().to_string(),
        ));
    }

    let task_dir = tasks_root.join(task_id);
    fs::create_dir_all(&task_dir)
        .map_err(|_| TeamError::WorkspacePathRuntimeUnavailable(task_dir.display().to_string()))?;
    let task_dir = fs::canonicalize(&task_dir)
        .map_err(|_| TeamError::WorkspacePathRuntimeUnavailable(task_dir.display().to_string()))?;
    if !task_dir.starts_with(&tasks_root) {
        return Err(TeamError::WorkspacePathRuntimeUnavailable(
            task_dir.display().to_string(),
        ));
    }
    Ok(task_dir.to_string_lossy().into_owned())
}

pub(crate) struct TeamWorkspaceResolver {
    repo: Arc<dyn ITeamRepository>,
    conversation_port: Arc<dyn TeamConversationProvisioningPort>,
}

impl TeamWorkspaceResolver {
    pub(crate) fn new(
        repo: Arc<dyn ITeamRepository>,
        conversation_port: Arc<dyn TeamConversationProvisioningPort>,
    ) -> Self {
        Self {
            repo,
            conversation_port,
        }
    }

    pub(crate) async fn resolve_for_new_agent(&self, row: &TeamRow, team: &Team) -> Result<String, TeamError> {
        if let Some(workspace) = usable_runtime_workspace(row.workspace.trim()) {
            return Ok(workspace);
        }

        if let Some(leader_workspace) = self.resolve_from_leader(team).await? {
            self.write_team_workspace(&row.id, &leader_workspace).await?;
            warn!(
                team_id = %row.id,
                workspace_source = "leader_conversation",
                "team workspace lazy backfilled"
            );
            return Ok(leader_workspace);
        }

        let workspace = self
            .conversation_port
            .create_team_temp_workspace(&row.user_id, &row.id)
            .await?;
        let workspace = validate_runtime_workspace_path(&workspace)?;
        self.write_team_workspace(&row.id, &workspace).await?;
        self.patch_leader_workspace_best_effort(&row.id, team, &workspace).await;
        warn!(
            team_id = %row.id,
            workspace_source = "team_temp_fallback",
            "team workspace lazy backfilled"
        );
        Ok(workspace)
    }

    async fn resolve_from_leader(&self, team: &Team) -> Result<Option<String>, TeamError> {
        let Some(leader) = team
            .agents
            .iter()
            .find(|agent| Some(&agent.slot_id) == team.lead_agent_id.as_ref())
            .or_else(|| team.agents.iter().find(|agent| agent.role == TeammateRole::Lead))
        else {
            return Ok(None);
        };
        let Some(workspace) = self
            .conversation_port
            .conversation_workspace(&leader.conversation_id)
            .await?
        else {
            return Ok(None);
        };
        Ok(usable_runtime_workspace(workspace.trim()))
    }

    async fn write_team_workspace(&self, team_id: &str, workspace: &str) -> Result<(), TeamError> {
        let row = self
            .repo
            .get_team_for_restore(team_id)
            .await?
            .ok_or_else(|| TeamError::TeamNotFound(team_id.to_owned()))?;
        self.repo
            .update_team(
                &row.user_id,
                team_id,
                &UpdateTeamParams {
                    workspace: Some(workspace.to_owned()),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    async fn patch_leader_workspace_best_effort(&self, team_id: &str, team: &Team, workspace: &str) {
        let Some(leader) = team
            .agents
            .iter()
            .find(|agent| Some(&agent.slot_id) == team.lead_agent_id.as_ref())
            .or_else(|| team.agents.iter().find(|agent| agent.role == TeammateRole::Lead))
        else {
            return;
        };
        if let Err(e) = self
            .conversation_port
            .patch_runtime_config(&leader.conversation_id, serde_json::json!({ "workspace": workspace }))
            .await
        {
            warn!(
                team_id = %team_id,
                conversation_id = %leader.conversation_id,
                error = %e,
                "failed to patch leader workspace after team temp fallback"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn task_workspace_is_created_under_the_team_workspace() {
        let root = tempdir().unwrap();
        let task_dir = resolve_task_dir(root.path().to_str().unwrap(), "019f4056-task-1").unwrap();

        assert!(Path::new(&task_dir).is_dir());
        assert_eq!(Path::new(&task_dir).parent().unwrap().file_name().unwrap(), ".tasks");
        assert!(Path::new(&task_dir).starts_with(fs::canonicalize(root.path()).unwrap()));
    }

    #[test]
    fn task_workspace_rejects_path_traversal_and_invalid_roots() {
        let root = tempdir().unwrap();
        let root_path = root.path().to_str().unwrap();

        for task_id in ["", "../escape", "nested/task", "/absolute", "task with spaces"] {
            let result = resolve_task_dir(root_path, task_id);
            assert!(result.is_err(), "task id should be rejected: {task_id:?}");
        }

        let missing = root.path().join("missing");
        assert!(resolve_task_dir(missing.to_str().unwrap(), "task-1").is_err());
    }
}
