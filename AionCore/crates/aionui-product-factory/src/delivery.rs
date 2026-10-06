use aionui_api_types::{
    ProductFactoryCostReportResponse, ProductFactoryDeliveryCheck, ProductFactoryDeliveryResponse,
    ProductFactoryDeliveryTaskCounts,
};
use aionui_common::{now_ms, validate_workspace_path_availability};
use std::path::Path;

use crate::service::{parse_task_draft, to_response};
use crate::{ProductFactoryError, ProductFactoryService};

impl ProductFactoryService {
    pub async fn get_cost_report(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryCostReportResponse, ProductFactoryError> {
        let row = self.load_run(user_id, run_id).await?;
        let planning = self.get_planning_costs(user_id, run_id).await?;
        if row.team_id.is_none() {
            let summary = crate::planning_cost::planning_summary(&row.id, row.budget_usd, &planning);
            return Ok(ProductFactoryCostReportResponse {
                run: to_response(row),
                usage: planning.usage,
                usage_summary: summary,
                task_counts: Default::default(),
                generated_at: now_ms(),
                planning_usage: planning.costs,
            });
        }
        if let Some(ledger) = self.usage_repository.as_ref() {
            let team_id = row
                .team_id
                .as_deref()
                .ok_or_else(|| ProductFactoryError::ReviewBlocked("Team is unavailable".into()))?;
            let port = self
                .review_port
                .as_ref()
                .ok_or_else(|| ProductFactoryError::ReviewFailed("Team usage adapter is unavailable".into()))?;
            let summary = port.get_usage_summary(user_id, team_id).await?;
            let draft = parse_task_draft(&row)?;
            let mut task_counts = ProductFactoryDeliveryTaskCounts {
                total: draft.tasks.len() as u32,
                ..Default::default()
            };
            let mut usage = summary.unassigned_usage.clone();
            for task in &draft.tasks {
                let task_id = format!("{team_id}-{}", task.id);
                let task_row = self
                    .repository
                    .get_handoff_task(user_id, team_id, &task_id)
                    .await?
                    .ok_or_else(|| ProductFactoryError::ReviewBlocked("handoff task is unavailable".into()))?;
                match task_row.status.as_str() {
                    "pending" => task_counts.pending += 1,
                    "in_progress" => task_counts.in_progress += 1,
                    "in_review" => task_counts.in_review += 1,
                    "completed" => task_counts.completed += 1,
                    _ => task_counts.failed += 1,
                }
                usage.extend(ledger.list_by_task(user_id, &task_id).await?.into_iter().map(|row| {
                    aionui_api_types::TeamTaskUsageResponse {
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
                    }
                }));
            }
            return Ok(ProductFactoryCostReportResponse {
                run: to_response(row),
                usage,
                usage_summary: summary,
                task_counts,
                generated_at: now_ms(),
                planning_usage: planning.costs,
            });
        }
        let delivery = self.get_delivery(user_id, run_id).await?;
        let team_id = delivery
            .run
            .team_id
            .as_deref()
            .ok_or_else(|| ProductFactoryError::ReviewBlocked("run has not been handed off to Team".into()))?;
        let port = self
            .review_port
            .as_ref()
            .ok_or_else(|| ProductFactoryError::ReviewFailed("Team review adapter is unavailable".into()))?;
        let mut usage = delivery.usage_summary.unassigned_usage.clone();
        let draft = parse_task_draft(&self.load_run(user_id, run_id).await?)?;
        for task in &draft.tasks {
            let task_id = format!("{team_id}-{}", task.id);
            usage.extend(port.get_task_snapshot(user_id, team_id, &task_id).await?.usage);
        }
        Ok(ProductFactoryCostReportResponse {
            run: delivery.run,
            usage,
            usage_summary: delivery.usage_summary,
            task_counts: delivery.task_counts,
            generated_at: now_ms(),
            planning_usage: planning.costs,
        })
    }

    pub async fn get_delivery(
        &self,
        user_id: &str,
        run_id: &str,
    ) -> Result<ProductFactoryDeliveryResponse, ProductFactoryError> {
        let row = self.load_run(user_id, run_id).await?;
        let team_id = row
            .team_id
            .as_deref()
            .ok_or_else(|| ProductFactoryError::ReviewBlocked("run has not been handed off to Team".into()))?;
        let draft = parse_task_draft(&row)?;
        let review = self.get_review(user_id, run_id).await?;
        let mut task_counts = ProductFactoryDeliveryTaskCounts {
            total: draft.tasks.len() as u32,
            ..Default::default()
        };

        for draft_task in &draft.tasks {
            let task_id = format!("{team_id}-{}", draft_task.id);
            let task = self
                .repository
                .get_handoff_task(user_id, team_id, &task_id)
                .await?
                .ok_or_else(|| ProductFactoryError::ReviewBlocked("handoff task is unavailable".into()))?;
            match task.status.as_str() {
                "pending" => task_counts.pending += 1,
                "in_progress" => task_counts.in_progress += 1,
                "in_review" => task_counts.in_review += 1,
                "completed" => task_counts.completed += 1,
                "failed" => task_counts.failed += 1,
                _ => {}
            }
        }

        let workspace_available = validate_workspace_path_availability(&row.workspace_path).is_ok();
        let start_guide_exists = Path::new(&row.workspace_path).join("START.md").is_file();
        let acceptance_evidence_exists = Path::new(&row.workspace_path).join("ACCEPTANCE.md").is_file();
        let all_tasks_completed = task_counts.total > 0 && task_counts.completed == task_counts.total;
        let run_completed = row.status == "completed";
        let usage_recorded = review.usage_summary.input_tokens > 0
            || review.usage_summary.output_tokens > 0
            || review.usage_summary.cost_est.is_some();
        let cost_known =
            usage_recorded && !review.usage_summary.cost_unknown && review.usage_summary.cost_est.is_some();
        let checks = vec![
            check("run_completed", run_completed, "Product Factory run is completed"),
            check(
                "all_tasks_completed",
                all_tasks_completed,
                "All planned tasks are completed",
            ),
            check(
                "workspace_available",
                workspace_available,
                "Workspace path is available",
            ),
            check(
                "start_guide_exists",
                start_guide_exists,
                "START.md exists in the project workspace",
            ),
            check(
                "acceptance_evidence_exists",
                acceptance_evidence_exists,
                "ACCEPTANCE.md exists in the project workspace",
            ),
            check("usage_recorded", usage_recorded, "Task usage records are present"),
            check(
                "cost_known",
                cost_known,
                "Recorded task costs are known or configured estimates",
            ),
            check(
                "budget_within_limit",
                review.usage_summary.budget_limit_usd.is_none()
                    || (cost_known && !review.usage_summary.budget_exceeded),
                "If a budget is configured, recorded costs must remain below its limit",
            ),
        ];
        let ready = checks.iter().all(|item| item.passed);

        Ok(ProductFactoryDeliveryResponse {
            run: to_response(row),
            ready,
            workspace_path: review.run.workspace_path.clone(),
            task_counts,
            usage_summary: review.usage_summary,
            checks,
        })
    }
}

fn check(code: &str, passed: bool, detail: &str) -> ProductFactoryDeliveryCheck {
    ProductFactoryDeliveryCheck {
        code: code.to_owned(),
        passed,
        detail: detail.to_owned(),
    }
}
