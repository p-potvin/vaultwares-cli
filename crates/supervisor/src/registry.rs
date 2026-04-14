use std::path::PathBuf;

use agentciation::{AlertSeverity, FailureClass, LaneLifecycleState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTarget {
    WindowsTerminalTab,
    WindowsTerminalWindow,
    ConPtyFallback,
}

impl TerminalTarget {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WindowsTerminalTab => "windows_terminal_tab",
            Self::WindowsTerminalWindow => "windows_terminal_window",
            Self::ConPtyFallback => "conpty_fallback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneState {
    Spawning,
    TrustRequired,
    Ready,
    Idle,
    Assigned,
    Running,
    Blocked,
    Lost,
    Finished,
    Failed,
}

impl From<LaneState> for LaneLifecycleState {
    fn from(value: LaneState) -> Self {
        match value {
            LaneState::Spawning => Self::Spawning,
            LaneState::TrustRequired => Self::TrustRequired,
            LaneState::Ready => Self::Ready,
            LaneState::Idle => Self::Idle,
            LaneState::Assigned => Self::Assigned,
            LaneState::Running => Self::Running,
            LaneState::Blocked => Self::Blocked,
            LaneState::Lost => Self::Lost,
            LaneState::Finished => Self::Finished,
            LaneState::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneRegistration {
    pub lane_id: String,
    pub worker_name: String,
    pub cwd: PathBuf,
    pub role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    pub terminal_target: TerminalTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneHandle {
    pub lane_id: String,
    pub worker_name: String,
    pub worker_id: String,
    pub cwd: PathBuf,
    pub session_dir: PathBuf,
    pub state: LaneState,
    pub terminal_target: TerminalTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_assignment_id: Option<String>,
    pub last_seen_at: String,
    pub missed_heartbeats: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneLease {
    pub lane_id: String,
    pub heartbeat_at: String,
    pub ttl_secs: u64,
    pub missed_heartbeats: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignmentRecord {
    pub assignment_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    pub target_lane: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupervisorAlert {
    pub severity: AlertSeverity,
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<String>,
    pub detail: String,
    pub emitted_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<FailureClass>,
}
