use crate::ProductFactoryError;
use aionui_api_types::{
    ProductFactoryPlanningCost, ProductFactoryPlanningPhase, TeamTaskUsageResponse, TeamUsageSummaryResponse,
};
use aionui_db::{IAgentUsageRepository, IProductFactoryPlanningRepository};

#[derive(Default)]
pub struct ProductFactoryPlanningCosts {
    pub costs: Vec<ProductFactoryPlanningCost>,
    pub usage: Vec<TeamTaskUsageResponse>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_est: Option<f64>,
    pub cost_unknown: bool,
}

pub(crate) fn phase(value: &str) -> Result<ProductFactoryPlanningPhase, ProductFactoryError> {
    match value {
        "interview" => Ok(ProductFactoryPlanningPhase::Interview),
        "blueprint" => Ok(ProductFactoryPlanningPhase::Blueprint),
        "task_graph" => Ok(ProductFactoryPlanningPhase::TaskGraph),
        _ => Err(ProductFactoryError::PlanningFailed("PLANNING_INVALID_PHASE".into())),
    }
}

pub async fn read_planning_costs(
    planning: &dyn IProductFactoryPlanningRepository,
    usage: &dyn IAgentUsageRepository,
    user_id: &str,
    run_id: &str,
) -> Result<ProductFactoryPlanningCosts, ProductFactoryError> {
    read_costs(planning, usage, user_id, run_id, None).await
}

pub(crate) async fn read_costs(
    planning: &dyn IProductFactoryPlanningRepository,
    usage: &dyn IAgentUsageRepository,
    user_id: &str,
    run_id: &str,
    first_send_conversation: Option<&str>,
) -> Result<ProductFactoryPlanningCosts, ProductFactoryError> {
    let mut result = ProductFactoryPlanningCosts {
        cost_est: Some(0.0),
        ..Default::default()
    };
    let mut total_cost = 0.0;
    for attempt in planning.list(user_id, run_id).await? {
        let rows = match attempt.conversation_id.as_deref() {
            Some(id) => usage.list_by_conversation(user_id, id).await?,
            None => vec![],
        };
        let not_sent_rejection =
            attempt.error_code.as_deref() == Some("USER_MODEL_SEND_ADMISSION_REJECTED") && rows.is_empty();
        let first_send_window = attempt.state == "running"
            && attempt.conversation_id.as_deref() == first_send_conversation
            && rows.is_empty();
        let unknown = attempt.state == "uncertain"
            || attempt.error_code.as_deref() == Some("PLANNING_CANCELLATION_UNCONFIRMED")
            || (attempt.started_at.is_some() && rows.is_empty() && !not_sent_rejection && !first_send_window)
            || rows.iter().any(|row| row.cost_est.is_none());
        result.cost_unknown |= unknown;
        let mut items = Vec::new();
        for row in rows {
            result.input_tokens = result
                .input_tokens
                .checked_add(row.input_tokens)
                .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_USAGE_OUT_OF_RANGE".into()))?;
            result.output_tokens = result
                .output_tokens
                .checked_add(row.output_tokens)
                .ok_or_else(|| ProductFactoryError::PlanningFailed("PLANNING_USAGE_OUT_OF_RANGE".into()))?;
            if let Some(cost) = row.cost_est {
                if !cost.is_finite() || cost < 0.0 {
                    result.cost_unknown = true;
                } else {
                    total_cost += cost;
                }
            }
            items.push(TeamTaskUsageResponse {
                id: row.id,
                task_id: row.task_id,
                agent_id: row.agent_id,
                model: row.model,
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                cost_est: row.cost_est,
                cached_read_tokens: row.cached_read_tokens,
                cached_write_tokens: row.cached_write_tokens,
                cost_source: row.cost_source,
                pricing_snapshot: row.pricing_snapshot,
                cost_unknown_reason: row.cost_unknown_reason,
                conversation_id: row.conversation_id,
                turn_id: row.turn_id,
                attempt_id: row.attempt_id,
                created_at: row.created_at,
            });
        }
        result.usage.extend(items.iter().cloned());
        result.costs.push(ProductFactoryPlanningCost {
            planning_attempt_id: attempt.id,
            phase: phase(&attempt.phase)?,
            state: attempt.state,
            cost_unknown: unknown,
            usage: items,
        });
    }
    if !total_cost.is_finite() {
        result.cost_unknown = true;
    }
    result.cost_est = (!result.cost_unknown).then_some((total_cost * 1_000_000.0).round() / 1_000_000.0);
    Ok(result)
}

pub(crate) fn planning_summary(
    run_id: &str,
    budget: Option<f64>,
    costs: &ProductFactoryPlanningCosts,
) -> TeamUsageSummaryResponse {
    let spent = costs.cost_est.unwrap_or(0.0);
    TeamUsageSummaryResponse {
        team_id: run_id.into(),
        input_tokens: costs.input_tokens,
        output_tokens: costs.output_tokens,
        cost_est: costs.cost_est,
        cost_unknown: costs.cost_unknown,
        budget_limit_usd: budget,
        budget_remaining_usd: (!costs.cost_unknown)
            .then(|| budget.map(|limit| limit - spent))
            .flatten(),
        budget_exceeded: !costs.cost_unknown && budget.is_some_and(|limit| spent >= limit),
        tasks: vec![],
        unassigned_usage: costs.usage.clone(),
    }
}
