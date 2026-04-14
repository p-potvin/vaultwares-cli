use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const EVENT_STREAM: &str = "vw:coord:v1:events";
pub const ASSIGNMENT_STREAM: &str = "vw:coord:v1:assignments";
pub const ALERT_STREAM: &str = "vw:coord:v1:alerts";
pub const PUBSUB_TASKS: &str = "vw:coord:v1:tasks";
pub const PUBSUB_STATUS: &str = "vw:coord:v1:status";
pub const PUBSUB_ALERTS: &str = "vw:coord:v1:alerts";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion(pub String);

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self("vw-coord/v1".to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneLifecycleState {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    PromptDelivery,
    TrustGate,
    BranchDivergence,
    Compile,
    Test,
    PluginStartup,
    McpStartup,
    McpHandshake,
    GatewayRouting,
    ToolRuntime,
    WorkspaceMismatch,
    Infra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinationMessageKind {
    #[serde(rename = "lane.register")]
    LaneRegister,
    #[serde(rename = "lane.heartbeat")]
    LaneHeartbeat,
    #[serde(rename = "lane.assign")]
    LaneAssign,
    #[serde(rename = "lane.accepted")]
    LaneAccepted,
    #[serde(rename = "lane.status")]
    LaneStatus,
    #[serde(rename = "lane.complete")]
    LaneComplete,
    #[serde(rename = "lane.fail")]
    LaneFail,
    #[serde(rename = "manager.alert")]
    ManagerAlert,
    #[serde(rename = "manager.request_update")]
    ManagerRequestUpdate,
    #[serde(rename = "manager.realign")]
    ManagerRealign,
    #[serde(rename = "team.snapshot")]
    TeamSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPacketWire {
    pub objective: String,
    pub scope: String,
    pub repo: String,
    pub branch_policy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_tests: Vec<String>,
    pub commit_policy: String,
    pub reporting_contract: String,
    pub escalation_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneRegisterPayload {
    pub role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    pub cwd: String,
    pub catalog_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneHeartbeatPayload {
    pub state: LaneLifecycleState,
    pub missed_heartbeats: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_assignment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneAssignPayload {
    pub assignment_id: String,
    pub task_packet: TaskPacketWire,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lane: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneAcceptedPayload {
    pub assignment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneStatusPayload {
    pub state: LaneLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_event: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaneArtifacts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneCompletePayload {
    pub assignment_id: String,
    pub summary: String,
    pub artifacts: LaneArtifacts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneFailPayload {
    pub assignment_id: String,
    pub failure_class: FailureClass,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagerAlertPayload {
    pub severity: AlertSeverity,
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ManagerRequestUpdatePayload {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_lanes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagerRealignPayload {
    pub note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamSnapshotLane {
    pub lane_id: String,
    pub agent_id: String,
    pub state: LaneLifecycleState,
    pub last_seen_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_assignment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TeamSnapshotPayload {
    pub catalog_revision: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<TeamSnapshotLane>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoordinationPayload {
    LaneRegister(LaneRegisterPayload),
    LaneHeartbeat(LaneHeartbeatPayload),
    LaneAssign(LaneAssignPayload),
    LaneAccepted(LaneAcceptedPayload),
    LaneStatus(LaneStatusPayload),
    LaneComplete(LaneCompletePayload),
    LaneFail(LaneFailPayload),
    ManagerAlert(ManagerAlertPayload),
    ManagerRequestUpdate(ManagerRequestUpdatePayload),
    ManagerRealign(ManagerRealignPayload),
    TeamSnapshot(TeamSnapshotPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinationEnvelope {
    pub protocol_version: ProtocolVersion,
    pub message_id: String,
    pub kind: CoordinationMessageKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub emitted_at: String,
    pub payload: CoordinationPayload,
}

impl CoordinationEnvelope {
    #[must_use]
    pub fn new(
        kind: CoordinationMessageKind,
        message_id: impl Into<String>,
        payload: CoordinationPayload,
    ) -> Self {
        Self {
            protocol_version: ProtocolVersion::default(),
            message_id: message_id.into(),
            kind,
            team_id: None,
            lane_id: None,
            agent_id: None,
            correlation_id: None,
            emitted_at: Utc::now().to_rfc3339(),
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneStateSnapshot {
    pub lane_id: String,
    pub agent_id: String,
    pub state: LaneLifecycleState,
    pub last_seen_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_assignment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_revision: Option<String>,
}
