use aionui_common::TimestampMs;
use serde::{Deserialize, Serialize};

/// One durable Product Factory workflow run owned by a user.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct ProductFactoryRunRow {
    pub id: String,
    pub plan_revision: i64,
    pub user_id: String,
    pub name: String,
    pub idea: String,
    pub target_user: String,
    pub problem: String,
    pub expected_output: String,
    pub workspace_path: String,
    pub budget_usd: Option<f64>,
    pub status: String,
    pub interview_json: Option<String>,
    pub blueprint_json: Option<String>,
    pub task_draft_json: Option<String>,
    pub team_id: Option<String>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ProductFactoryPlanningRow {
    pub id: String,
    pub user_id: String,
    pub run_id: String,
    pub phase: String,
    pub input_plan_revision: i64,
    pub input_hash: String,
    pub idempotency_key: String,
    pub assistant_id: String,
    pub model: String,
    pub assistant_snapshot_json: Option<String>,
    pub conversation_id: Option<String>,
    pub app_turn_id: Option<String>,
    pub state: String,
    pub result_json: Option<String>,
    pub error_code: Option<String>,
    pub started_at: Option<TimestampMs>,
    pub created_at: TimestampMs,
    pub updated_at: TimestampMs,
}

/// Durable receipt for one explicit dispatch attempt, never an Agent outcome.
#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct ProductFactoryExecutionRow {
    pub run_id: String,
    pub user_id: String,
    pub team_id: String,
    pub task_id: String,
    pub state: String,
    pub message_id: Option<String>,
    pub team_run_id: Option<String>,
    pub requested_at: TimestampMs,
    pub updated_at: TimestampMs,
}
