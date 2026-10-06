//! Shared Send budget checks without a strong service reference cycle.

use std::sync::{Arc, Weak};

use aionui_api_types::TeamSessionBinding;
use aionui_conversation::ConversationSendAdmissionPort;
use aionui_db::{IConversationRepository, ITeamRepository, SqliteConversationRepository, SqliteTeamRepository};
use aionui_product_factory::ProductFactoryService;
use aionui_team::TeamSessionService;

use crate::services::AppServices;

pub(super) fn install_send_admission(
    services: &AppServices,
    team: &Arc<TeamSessionService>,
    factory: &Arc<ProductFactoryService>,
) {
    services
        .conversation_service
        .with_send_admission_port(Arc::new(SendBudgetAdmission {
            team: Arc::downgrade(team),
            factory: Arc::downgrade(factory),
            conversations: Arc::new(SqliteConversationRepository::new(services.database.pool().clone())),
            teams: Arc::new(SqliteTeamRepository::new(services.database.pool().clone())),
        }));
}

struct SendBudgetAdmission {
    team: Weak<TeamSessionService>,
    factory: Weak<ProductFactoryService>,
    conversations: Arc<dyn IConversationRepository>,
    teams: Arc<dyn ITeamRepository>,
}

#[async_trait::async_trait]
impl ConversationSendAdmissionPort for SendBudgetAdmission {
    async fn check(
        &self,
        user_id: &str,
        conversation_id: &str,
        team_id: Option<&str>,
        app_turn_id: &str,
    ) -> Result<(), String> {
        let conversation = self
            .conversations
            .get(user_id, conversation_id)
            .await
            .map_err(|_| "conversation_budget_lookup_failed".to_owned())?
            .ok_or_else(|| "conversation_budget_owner_not_found".to_owned())?;
        let bound_team = persisted_team_binding(self.teams.as_ref(), user_id, conversation_id).await?;
        let marker_team = TeamSessionBinding::team_id_marker_from_extra_str(&conversation.extra);
        validate_team_binding(bound_team.as_deref(), marker_team.as_deref(), team_id)?;
        if let Some(team_id) = team_id {
            self.team
                .upgrade()
                .ok_or_else(|| "team_budget_unavailable".to_owned())?
                .ensure_task_budget(user_id, team_id)
                .await
                .map_err(|error| error.to_string())?;
        }

        // The durable attempt binding is authoritative even if an editable
        // conversation extra marker has been removed.
        let factory = self
            .factory
            .upgrade()
            .ok_or_else(|| "planning_budget_unavailable".to_owned())?;
        if let Some(team_id) = team_id {
            factory
                .ensure_team_version_mutable(user_id, team_id)
                .await
                .map_err(|error| error.to_string())?;
        }
        let planning_run = factory
            .planning_run_for_conversation(user_id, conversation_id)
            .await
            .map_err(|_| "planning_budget_lookup_failed".to_owned())?;
        let extra = serde_json::from_str::<serde_json::Value>(&conversation.extra)
            .map_err(|_| "conversation_budget_context_invalid".to_owned())?;
        let marker = extra.get("factory_planning_run_id").and_then(serde_json::Value::as_str);
        if marker.is_some() && marker != planning_run.as_deref() {
            return Err("planning_budget_binding_changed".into());
        }
        if let Some(run_id) = planning_run {
            factory
                .check_planning_send_budget(user_id, &run_id, conversation_id, app_turn_id)
                .await
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

async fn persisted_team_binding(
    teams: &dyn ITeamRepository,
    user: &str,
    conversation: &str,
) -> Result<Option<String>, String> {
    let mut matched = None;
    for row in teams
        .list_owned_teams_for_binding(user)
        .await
        .map_err(|_| "team_budget_lookup_failed".to_owned())?
    {
        let team = aionui_team::types::Team::from_row(&row).map_err(|_| "team_budget_binding_invalid".to_owned())?;
        if team.agents.iter().any(|agent| agent.conversation_id == conversation) {
            if matched.is_some() {
                return Err("team_budget_binding_ambiguous".into());
            }
            matched = Some(row.id);
        }
    }
    Ok(matched)
}

fn validate_team_binding(persisted: Option<&str>, marker: Option<&str>, captured: Option<&str>) -> Result<(), String> {
    if persisted != marker || persisted != captured {
        return Err("conversation_team_binding_changed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aionui_db::models::TeamRow;

    fn row(id: &str, user: &str, conversation: &str) -> TeamRow {
        TeamRow {
            id: id.into(),
            user_id: user.into(),
            name: "Fixture".into(),
            workspace: "/tmp".into(),
            workspace_mode: "shared".into(),
            agents: serde_json::json!([{
                "slot_id":"slot", "name":"Builder", "role":"lead", "conversation_id":conversation,
                "backend":"codex", "model":"fixture",
            }])
            .to_string(),
            lead_agent_id: None,
            session_mode: None,
            agents_version: "1".into(),
            created_at: 0,
            updated_at: 0,
            project_id: None,
            folder_id: None,
        }
    }

    #[tokio::test]
    async fn persisted_team_binding_is_owned_and_rejects_ambiguous_membership() {
        let database = aionui_db::init_database_memory().await.unwrap();
        let teams = SqliteTeamRepository::new(database.pool().clone());
        teams.create_team(&row("owned", "owner", "conversation")).await.unwrap();
        teams
            .create_team(&row("foreign", "other", "foreign-conversation"))
            .await
            .unwrap();
        assert_eq!(
            persisted_team_binding(&teams, "owner", "conversation")
                .await
                .unwrap()
                .as_deref(),
            Some("owned")
        );
        assert_eq!(
            persisted_team_binding(&teams, "other", "conversation").await.unwrap(),
            None
        );
        assert_eq!(
            persisted_team_binding(&teams, "owner", "foreign-conversation")
                .await
                .unwrap(),
            None
        );
        sqlx::query("UPDATE teams SET archived_at=1 WHERE id='owned'")
            .execute(database.pool())
            .await
            .unwrap();
        assert!(teams.list_teams_by_user("owner").await.unwrap().is_empty());
        assert_eq!(
            persisted_team_binding(&teams, "owner", "conversation")
                .await
                .unwrap()
                .as_deref(),
            Some("owned")
        );
        teams
            .create_team(&row("duplicate", "owner", "conversation"))
            .await
            .unwrap();
        assert_eq!(
            persisted_team_binding(&teams, "owner", "conversation")
                .await
                .unwrap_err(),
            "team_budget_binding_ambiguous"
        );
    }

    #[test]
    fn editing_extra_cannot_detach_a_persisted_member_from_its_budget() {
        assert_eq!(
            validate_team_binding(Some("team"), None, None).unwrap_err(),
            "conversation_team_binding_changed"
        );
        assert_eq!(
            validate_team_binding(Some("team"), Some("fake"), Some("fake")).unwrap_err(),
            "conversation_team_binding_changed"
        );
        assert!(validate_team_binding(Some("team"), Some("team"), Some("team")).is_ok());
        assert!(validate_team_binding(None, None, None).is_ok());
    }
}
