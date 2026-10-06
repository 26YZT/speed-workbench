use aionui_api_types::{
    AssistantConversationOverridesRequest, AssistantConversationRequest, CreateConversationRequest,
};
use aionui_conversation::{ConversationAgentTurnRequest, ConversationAgentTurnStatus, ConversationService};
use aionui_db::{IAgentUsageRepository, IConversationRepository, IProductFactoryPlanningRepository};
use aionui_product_factory::{
    ProductFactoryError, ProductFactoryPlanningInvocation, ProductFactoryPlanningOutcome, ProductFactoryPlanningPort,
    ProductFactoryPlanningPrepared, ProductFactoryPlanningStarted,
};
use std::{path::PathBuf, sync::Arc};

pub(crate) struct FactoryPlanningAdapter {
    pub conversations: ConversationService,
    pub conversation_repo: Arc<dyn IConversationRepository>,
    pub task_manager: Arc<dyn aionui_ai_agent::IWorkerTaskManager>,
    pub planning_root: PathBuf,
}

#[async_trait::async_trait]
impl ProductFactoryPlanningPort for FactoryPlanningAdapter {
    async fn prepare(
        &self,
        input: &ProductFactoryPlanningInvocation,
    ) -> Result<ProductFactoryPlanningPrepared, ProductFactoryError> {
        let folder = (|| -> std::io::Result<PathBuf> {
            std::fs::create_dir_all(&self.planning_root)?;
            let root = std::fs::canonicalize(&self.planning_root)?;
            let folder = root.join(&input.attempt_id);
            std::fs::create_dir(&folder)?;
            let resolved = std::fs::canonicalize(&folder)?;
            if resolved.parent() != Some(root.as_path()) {
                return Err(std::io::Error::other("planning workspace boundary"));
            }
            Ok(resolved)
        })()
        .map_err(|_| ProductFactoryError::PlanningFailed("PLANNING_WORKSPACE_UNAVAILABLE".into()))?;
        // Ordinary create resolves and freezes the configured assistant without
        // warming a CLI, creating a Team, or reusing a development conversation.
        let response=self.conversations.create(&input.user_id,CreateConversationRequest {
            r#type:None,name:Some(format!("Product planning: {}",input.phase.as_str())),model:None,
            assistant:Some(AssistantConversationRequest {id:input.assistant_id.clone(),locale:None,conversation_overrides:Some(AssistantConversationOverridesRequest {model:(!input.model.is_empty()).then(||input.model.clone()),..Default::default()})}),
            source:None,channel_chat_id:None,extra:serde_json::json!({"workspace":folder,"factory_planning_run_id":input.run_id,"factory_planning_attempt_id":input.attempt_id}),
        }).await.map_err(|_|ProductFactoryError::PlanningFailed("PLANNING_ASSISTANT_UNAVAILABLE".into()))?;
        let snapshot = self
            .conversation_repo
            .get_assistant_snapshot(&input.user_id, &response.id)
            .await?
            .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_ASSISTANT_SNAPSHOT_MISSING".into()))?;
        Ok(ProductFactoryPlanningPrepared {
            conversation_id: response.id,
            assistant_snapshot: serde_json::to_value(snapshot)
                .map_err(|_| ProductFactoryError::PlanningFailed("PLANNING_ASSISTANT_SNAPSHOT_INVALID".into()))?,
        })
    }
    async fn run(
        &self,
        input: &ProductFactoryPlanningInvocation,
        conversation_id: &str,
        on_started: ProductFactoryPlanningStarted,
    ) -> Result<ProductFactoryPlanningOutcome, ProductFactoryError> {
        let callback: aionui_conversation::ConversationAgentTurnStartedCallback = Arc::new(move |started| {
            let callback = on_started.clone();
            Box::pin(async move {
                callback(started.turn_id).await;
            })
        });
        let outcome = self
            .conversations
            .run_agent_turn(ConversationAgentTurnRequest {
                user_id: input.user_id.clone(),
                conversation_id: conversation_id.into(),
                task_id: None,
                task_workspace: None,
                content: input.prompt.clone(),
                files: vec![],
                inject_skills: vec![],
                required_runtime_mode: None,
                persist_user_message: true,
                user_message_hidden: false,
                on_started: Some(callback),
            })
            .await
            .map_err(|_| ProductFactoryError::PlanningFailed("PLANNING_RUNTIME_OUTCOME_UNCERTAIN".into()))?;
        Ok(ProductFactoryPlanningOutcome {
            app_turn_id: outcome.turn_id,
            completed: outcome.status == ConversationAgentTurnStatus::Completed,
            output_complete: outcome.output_complete,
            final_text: outcome.final_text,
            admission_rejected: outcome.admission_rejected,
        })
    }
    async fn cancel(&self, user_id: &str, conversation_id: &str, app_turn_id: &str) -> Result<(), ProductFactoryError> {
        self.conversations
            .cancel(user_id, conversation_id, app_turn_id, &self.task_manager)
            .await
            .map(|_| ())
            .map_err(|_| ProductFactoryError::PlanningFailed("PLANNING_CANCELLATION_UNCONFIRMED".into()))
    }
}

pub(crate) struct FactoryPlanningBudgetAdapter {
    pub planning: Arc<dyn IProductFactoryPlanningRepository>,
    pub usage: Arc<dyn IAgentUsageRepository>,
}
#[async_trait::async_trait]
impl aionui_team::TeamBudgetContributionPort for FactoryPlanningBudgetAdapter {
    async fn get_contribution(
        &self,
        user_id: &str,
        team_id: &str,
    ) -> Result<Option<aionui_team::TeamBudgetContribution>, aionui_team::TeamError> {
        let Some(run_id) = self.planning.run_for_team(user_id, team_id).await? else {
            return Ok(None);
        };
        let costs =
            aionui_product_factory::read_planning_costs(self.planning.as_ref(), self.usage.as_ref(), user_id, &run_id)
                .await
                .map_err(|_| {
                    aionui_team::TeamError::InvalidRequest("Factory planning costs could not be verified".into())
                })?;
        Ok(Some(aionui_team::TeamBudgetContribution {
            usage: costs.usage,
            cost_unknown: costs.cost_unknown,
        }))
    }
}
