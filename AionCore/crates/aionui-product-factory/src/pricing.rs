use crate::service::parse_task_draft;
use crate::{ProductFactoryError, ProductFactoryService};
use aionui_api_types::ProductFactoryRepriceResponse;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ProductFactoryRepricingPort: Send + Sync {
    async fn reprice_planning_usage(
        &self,
        _user_id: &str,
        _conversation_ids: &[String],
    ) -> Result<ProductFactoryRepriceResponse, ProductFactoryError> {
        Err(ProductFactoryError::PlanningFailed(
            "PLANNING_PRICING_UNAVAILABLE".into(),
        ))
    }
    async fn reprice_task_usage(
        &self,
        user_id: &str,
        team_id: &str,
        task_ids: &[String],
    ) -> Result<ProductFactoryRepriceResponse, ProductFactoryError>;
}

impl ProductFactoryService {
    pub fn with_repricing_port(mut self, port: Arc<dyn ProductFactoryRepricingPort>) -> Self {
        self.repricing_port = Some(port);
        self
    }
    pub async fn reprice_costs(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryRepriceResponse, ProductFactoryError> {
        let run = self.load_run(user_id, run_id).await?;
        let port = self
            .repricing_port
            .as_ref()
            .ok_or_else(|| ProductFactoryError::ReviewFailed("usage pricing adapter is unavailable".into()))?;
        let mut result = ProductFactoryRepriceResponse::default();
        if let Some(repo) = self.planning_repository.as_ref() {
            let ids = repo
                .list(user_id, run_id)
                .await?
                .into_iter()
                .filter_map(|row| row.conversation_id)
                .collect::<Vec<_>>();
            if !ids.is_empty() {
                let planning = port.reprice_planning_usage(user_id, &ids).await?;
                result.updated_count += planning.updated_count;
                result.skipped_count += planning.skipped_count;
            }
        }
        if let Some(team_id) = run.team_id.as_deref() {
            let task_ids = parse_task_draft(&run)?
                .tasks
                .iter()
                .map(|task| format!("{team_id}-{}", task.id))
                .collect::<Vec<_>>();
            for id in &task_ids {
                if self.repository.get_handoff_task(user_id, team_id, id).await?.is_none() {
                    return Err(ProductFactoryError::ReviewBlocked("handoff task is unavailable".into()));
                }
            }
            let execution = port.reprice_task_usage(user_id, team_id, &task_ids).await?;
            result.updated_count += execution.updated_count;
            result.skipped_count += execution.skipped_count;
        }
        Ok(result)
    }
}
