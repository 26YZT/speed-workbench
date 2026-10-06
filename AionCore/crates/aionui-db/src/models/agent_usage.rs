use aionui_common::TimestampMs;
use serde::{Deserialize, Serialize};

/// One normalized per-turn usage report emitted by an agent runtime.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct AgentUsageRow {
    pub id: String,
    pub user_id: String,
    pub task_id: Option<String>,
    #[serde(default)]
    pub team_id: Option<String>,
    pub agent_id: Option<String>,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_est: Option<f64>,
    /// None means the runtime never captured this bucket (legacy records).
    #[serde(default)]
    pub cached_read_tokens: Option<i64>,
    #[serde(default)]
    pub cached_write_tokens: Option<i64>,
    #[serde(default)]
    pub cost_source: Option<String>,
    /// Exact configuration used to estimate this report; never a credential.
    #[serde(default)]
    pub pricing_snapshot: Option<String>,
    #[serde(default)]
    pub cost_unknown_reason: Option<String>,

    pub conversation_id: String,
    pub turn_id: String,
    #[serde(default = "legacy_attempt")]
    pub attempt_id: String,
    pub created_at: TimestampMs,
}

/// Explicit repricing result. The resolver must identify its price source.
#[derive(Debug, Clone)]
pub struct AgentUsageCost {
    pub cost_usd: f64,
    pub source: String,
    pub pricing_snapshot: String,
}

fn legacy_attempt() -> String {
    "legacy".to_owned()
}
