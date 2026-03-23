#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    WaitingForInput,
    Processing,
    RunningTool,
    WaitingForApproval,
    Compacting,
    Ended,
}

impl SessionStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Processing => "●",
            Self::RunningTool => "◆",
            Self::WaitingForInput => "○",
            Self::WaitingForApproval => "⚠",
            Self::Compacting => "⊘",
            Self::Ended => "✗",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Processing => "processing",
            Self::RunningTool => "running",
            Self::WaitingForInput => "idle",
            Self::WaitingForApproval => "approval",
            Self::Compacting => "compacting",
            Self::Ended => "ended",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Processing => Color::Cyan,
            Self::RunningTool => Color::Yellow,
            Self::WaitingForInput => Color::DarkGray,
            Self::WaitingForApproval => Color::Yellow,
            Self::Compacting => Color::Blue,
            Self::Ended => Color::Red,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Processing | Self::RunningTool | Self::WaitingForApproval | Self::Compacting
        )
    }

    pub fn sort_priority(&self) -> u8 {
        match self {
            Self::WaitingForApproval => 0,
            Self::Processing | Self::RunningTool | Self::Compacting => 1,
            Self::WaitingForInput => 2,
            Self::Ended => 3,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolHistoryEntry {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_output: Option<String>,
    pub success: Option<bool>,
    pub tool_use_id: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentNode {
    pub agent_id: String,
    pub status: String,
    pub started_at: i64,
    pub stopped_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub session_id: String,
    pub status: SessionStatus,
    pub cwd: String,
    pub transcript_path: String,
    pub pid: i64,
    pub started_at: i64,
    pub last_event_at: i64,
    pub current_tool: Option<String>,
    pub current_tool_input: Option<serde_json::Value>,
    pub current_tool_use_id: Option<String>,
    pub tool_history: Vec<ToolHistoryEntry>,
    pub agents: Vec<AgentNode>,
    pub last_notification: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingPermission {
    pub connection_id: String,
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub cwd: String,
    pub requested_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonMessage {
    StateSnapshot {
        sessions: Vec<SessionState>,
        #[serde(rename = "pendingPermissions")]
        pending_permissions: Vec<PendingPermission>,
    },
    StateDelta {
        sessions: Vec<SessionState>,
        #[serde(rename = "pendingPermissions")]
        pending_permissions: Vec<PendingPermission>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct TuiDecision {
    #[serde(rename = "type")]
    pub msg_type: &'static str,
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub decision: String,
}

// ── Usage types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelBreakdown {
    pub model_name: String,
    pub total_tokens: i64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyUsage {
    pub cost: f64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub date: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MonthlyUsage {
    pub month: String,
    pub total_cost: f64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub model_breakdowns: Vec<ModelBreakdown>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TotalUsage {
    pub total_cost: f64,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
    pub sessions: i64,
    pub model_breakdowns: Vec<ModelBreakdown>,
}

#[derive(Debug, Clone, Default)]
pub struct RateLimitEntry {
    pub utilization: f64,
    pub resets_at: String,
}

#[derive(Debug, Clone, Default)]
pub struct RateLimits {
    pub five_hour: RateLimitEntry,
    pub seven_day: RateLimitEntry,
    pub seven_day_sonnet: Option<RateLimitEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct UsageData {
    pub today: Option<DailyUsage>,
    pub yesterday: Option<DailyUsage>,
    pub monthly: Option<MonthlyUsage>,
    pub total: Option<TotalUsage>,
    pub daily_history: Vec<DailyUsage>,
    pub limits: Option<RateLimits>,
    pub loading: bool,
    pub limits_loading: bool,
    pub error: Option<String>,
    pub last_fetched: Option<std::time::Instant>,
}

