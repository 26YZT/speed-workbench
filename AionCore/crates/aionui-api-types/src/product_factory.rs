use aionui_common::TimestampMs;
use serde::{Deserialize, Serialize};

use crate::{TeamTaskResponse, TeamTaskUsageResponse, TeamTaskWorkspaceResponse, TeamUsageSummaryResponse};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartProductFactoryRequest {
    pub expected_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductFactoryExecutionState {
    Pending,
    Enqueued,
    Uncertain,
}

impl ProductFactoryExecutionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Enqueued => "enqueued",
            Self::Uncertain => "uncertain",
        }
    }
}

/// A dispatch receipt, not proof that an Agent or the generated product ran.
#[derive(Debug, Serialize)]
pub struct ProductFactoryExecution {
    pub state: ProductFactoryExecutionState,
    pub team_id: String,
    pub task_id: String,
    pub message_id: Option<String>,
    pub team_run_id: Option<String>,
    pub requested_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductFactoryExecutionResponse {
    pub execution: Option<ProductFactoryExecution>,
}

/// Explicit handoff confirmation. Assistant/model selection uses existing Team inputs.
/// Empty agents are accepted only when reading back a previously committed handoff.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffProductFactoryRequest {
    pub expected_revision: u64,
    #[serde(default)]
    pub agents: Vec<crate::TeamAgentInput>,
}

#[derive(Debug, Serialize)]
pub struct ProductFactoryHandoffResponse {
    pub run: ProductFactoryRunResponse,
    pub team_id: String,
    pub task_id_map: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDraftArtifact {
    pub version: u32,
    pub generated_by: String,
    pub confirmed: bool,
    pub revision: u64,
    pub updated_at: i64,
    pub tasks: Vec<TaskDraftTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDraftTask {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub task_type: TaskDraftType,
    pub blocked_by: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub suggested_role: String,
    pub effort: TaskDraftEffort,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_scope: Option<TaskDraftExecutionScope>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirement_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDraftExecutionScope {
    TaskWorkspace,
    ProjectIntegration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskDraftType {
    Frontend,
    Backend,
    Data,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskDraftEffort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateTaskDraftRequest {}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveTaskDraftRequest {
    pub expected_revision: u64,
    pub tasks: Vec<TaskDraftTask>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfirmTaskDraftRequest {
    pub expected_revision: u64,
}

/// Persisted state of a Product Factory workflow run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductFactoryRunStatus {
    Draft,
    Interviewing,
    BlueprintGenerating,
    BlueprintReady,
    TaskDraftGenerating,
    TaskDraftReady,
    HandedOff,
    Running,
    InReview,
    Completed,
    Failed,
}

impl ProductFactoryRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Interviewing => "interviewing",
            Self::BlueprintGenerating => "blueprint_generating",
            Self::BlueprintReady => "blueprint_ready",
            Self::TaskDraftGenerating => "task_draft_generating",
            Self::TaskDraftReady => "task_draft_ready",
            Self::HandedOff => "handed_off",
            Self::Running => "running",
            Self::InReview => "in_review",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for ProductFactoryRunStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "interviewing" => Ok(Self::Interviewing),
            "blueprint_generating" => Ok(Self::BlueprintGenerating),
            "blueprint_ready" => Ok(Self::BlueprintReady),
            "task_draft_generating" => Ok(Self::TaskDraftGenerating),
            "task_draft_ready" => Ok(Self::TaskDraftReady),
            "handed_off" => Ok(Self::HandedOff),
            "running" => Ok(Self::Running),
            "in_review" => Ok(Self::InReview),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown product factory status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProductFactoryRunRequest {
    pub name: String,
    pub idea: String,
    #[serde(default)]
    pub target_user: Option<String>,
    #[serde(default)]
    pub problem: Option<String>,
    #[serde(default)]
    pub expected_output: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransitionProductFactoryRunRequest {
    pub status: ProductFactoryRunStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveProductFactoryInterviewRequest {
    pub interview: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveProductFactoryBlueprintRequest {
    pub blueprint: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryRunResponse {
    pub id: String,
    pub plan_revision: u64,
    pub name: String,
    pub idea: String,
    pub target_user: String,
    pub problem: String,
    pub expected_output: String,
    pub workspace_path: String,
    pub budget_usd: Option<f64>,
    pub status: ProductFactoryRunStatus,
    pub interview: Option<serde_json::Value>,
    pub blueprint: Option<serde_json::Value>,
    pub task_draft: Option<serde_json::Value>,
    pub team_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The human-review surface for the currently dispatched Product Factory task.
#[derive(Debug, Serialize)]
pub struct ProductFactoryTaskReviewResponse {
    pub run: ProductFactoryRunResponse,
    pub task: TeamTaskResponse,
    pub workspace: TeamTaskWorkspaceResponse,
    pub usage: Vec<TeamTaskUsageResponse>,
    pub usage_summary: TeamUsageSummaryResponse,
    pub acceptance_criteria: Vec<String>,
    pub next_task: Option<ProductFactoryNextTask>,
    pub execution: Option<ProductFactoryExecution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryNextTask {
    pub id: String,
    pub title: String,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryDeliveryCheck {
    pub code: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProductFactoryDeliveryTaskCounts {
    pub total: u32,
    pub pending: u32,
    pub in_progress: u32,
    pub in_review: u32,
    pub completed: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryDeliveryResponse {
    pub run: ProductFactoryRunResponse,
    pub ready: bool,
    pub workspace_path: String,
    pub task_counts: ProductFactoryDeliveryTaskCounts,
    pub usage_summary: TeamUsageSummaryResponse,
    pub checks: Vec<ProductFactoryDeliveryCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryCostReportResponse {
    pub run: ProductFactoryRunResponse,
    pub usage: Vec<TeamTaskUsageResponse>,
    pub usage_summary: TeamUsageSummaryResponse,
    pub task_counts: ProductFactoryDeliveryTaskCounts,
    pub generated_at: TimestampMs,
    pub planning_usage: Vec<ProductFactoryPlanningCost>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductFactoryPlanningPhase {
    Interview,
    Blueprint,
    TaskGraph,
}

impl ProductFactoryPlanningPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interview => "interview",
            Self::Blueprint => "blueprint",
            Self::TaskGraph => "task_graph",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartProductFactoryPlanningRequest {
    pub phase: ProductFactoryPlanningPhase,
    pub expected_plan_revision: u64,
    pub idempotency_key: String,
    pub assistant_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyProductFactoryPlanningRequest {
    pub expected_plan_revision: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelProductFactoryPlanningRequest {}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryPlanningResponse {
    pub id: String,
    pub run_id: String,
    pub phase: ProductFactoryPlanningPhase,
    pub input_plan_revision: u64,
    pub input_hash: String,
    pub idempotency_key: String,
    pub assistant_id: String,
    pub model: String,
    pub conversation_id: Option<String>,
    pub app_turn_id: Option<String>,
    pub state: String,
    pub candidate: Option<serde_json::Value>,
    pub error_code: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductFactoryPlanningCost {
    pub planning_attempt_id: String,
    pub phase: ProductFactoryPlanningPhase,
    pub state: String,
    pub cost_unknown: bool,
    pub usage: Vec<TeamTaskUsageResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductFactoryRepriceResponse {
    pub updated_count: u64,
    pub skipped_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepriceProductFactoryRequest {}
