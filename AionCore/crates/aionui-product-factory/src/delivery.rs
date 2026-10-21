use aionui_api_types::{
    ProductFactoryCostReportResponse, ProductFactoryDeliveryCheck, ProductFactoryDeliveryResponse,
    ProductFactoryDeliveryTaskCounts,
};
use aionui_common::{now_ms, validate_workspace_path_availability};
use std::io::Read;
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
                count_task_status(&mut task_counts, &task_row.status);
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
            count_task_status(&mut task_counts, &task.status);
        }

        let workspace_available = validate_workspace_path_availability(&row.workspace_path).is_ok();
        // Existence alone is not evidence: an empty START.md satisfies `is_file`
        // but delivers nothing, and version snapshots already reject blank
        // copies of the same two files.
        let start_guide_exists = delivered_document(&row.workspace_path, "START.md");
        let acceptance_evidence_exists = delivered_document(&row.workspace_path, "ACCEPTANCE.md");
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

/// Count one Team task status for the delivery and cost reports.
///
/// Both reports must agree: the cost report used to fold every unknown status
/// into `failed` while the delivery gate ignored it, so the two screens could
/// disagree about whether all tasks were completed.
fn count_task_status(counts: &mut ProductFactoryDeliveryTaskCounts, status: &str) {
    match status {
        "pending" => counts.pending += 1,
        "in_progress" => counts.in_progress += 1,
        "in_review" => counts.in_review += 1,
        "completed" => counts.completed += 1,
        // `deleted` is the only other status Team defines; anything else is a
        // status this build does not know, which cannot count as delivered.
        _ => counts.failed += 1,
    }
}

/// Whether a required delivery document exists and carries content.
///
/// Read in bounded chunks. Leading whitespace is valid, while symlinks and
/// non-regular files cannot certify delivery or block the request on a FIFO.
fn delivered_document(workspace_path: &str, name: &str) -> bool {
    let path = Path::new(workspace_path).join(name);
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    if !std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
        return false;
    }
    let Ok(mut file) = options.open(path) else {
        return false;
    };
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return false;
    }
    let mut chunk = [0; 4096];
    loop {
        match file.read(&mut chunk) {
            Ok(0) | Err(_) => return false,
            Ok(length) if chunk[..length].iter().any(|byte| !byte.is_ascii_whitespace()) => return true,
            Ok(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_requires_a_regular_document_with_content() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path().to_str().unwrap();
        assert!(!delivered_document(root, "START.md"));
        for text in ["", " \n\t"] {
            std::fs::write(workspace.path().join("START.md"), text).unwrap();
            assert!(!delivered_document(root, "START.md"));
        }
        std::fs::write(
            workspace.path().join("START.md"),
            format!("{}# Start", " ".repeat(5000)),
        )
        .unwrap();
        assert!(delivered_document(root, "START.md"));
        std::fs::create_dir(workspace.path().join("ACCEPTANCE.md")).unwrap();
        assert!(!delivered_document(root, "ACCEPTANCE.md"));
    }

    #[cfg(unix)]
    #[test]
    fn delivery_rejects_a_symlink_instead_of_following_it() {
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), "# Evidence").unwrap();
        std::os::unix::fs::symlink(outside.path(), workspace.path().join("START.md")).unwrap();
        assert!(!delivered_document(workspace.path().to_str().unwrap(), "START.md"));
    }
}
