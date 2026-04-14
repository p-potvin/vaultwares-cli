use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Manager,
    Worker,
    Specialist,
    Observer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_allowlist: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_preferences: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatPolicy {
    pub interval_secs: u64,
    pub max_missed_heartbeats: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NudgePolicy {
    pub silence_threshold_secs: u64,
    pub realign_after_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub repo_subpath: String,
    pub git_revision: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub id: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_priority: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: String,
    pub display_name: String,
    pub role: AgentRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_types: Vec<TaskDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompt_refs: Vec<SourceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflow_refs: Vec<String>,
    pub heartbeat_policy: HeartbeatPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nudge_policy: Option<NudgePolicy>,
    pub source: SourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStage {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<WorkflowStage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escalation_rules: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manager_role: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worker_roles: Vec<String>,
    pub source: SourceRef,
}
