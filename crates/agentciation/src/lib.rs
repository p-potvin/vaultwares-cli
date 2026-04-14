mod bus;
mod catalog;
mod loader;
mod manifest;
mod protocol;

pub use bus::{CoordinationKeyspace, RedisCoordinator};
pub use catalog::{CatalogRevision, CoordinationCatalog};
pub use loader::{CatalogLoaderConfig, SubmoduleCatalogLoader};
pub use manifest::{
    AgentDefinition, AgentRole, Capability, HeartbeatPolicy, NudgePolicy, SourceRef,
    TaskDefinition, WorkflowDefinition, WorkflowStage,
};
pub use protocol::{
    AlertSeverity, CoordinationEnvelope, CoordinationMessageKind, CoordinationPayload,
    FailureClass, LaneAcceptedPayload, LaneArtifacts, LaneAssignPayload, LaneCompletePayload,
    LaneFailPayload, LaneHeartbeatPayload, LaneLifecycleState, LaneRegisterPayload,
    LaneStateSnapshot, LaneStatusPayload, ManagerAlertPayload, ManagerRealignPayload,
    ManagerRequestUpdatePayload, ProtocolVersion, TaskPacketWire, TeamSnapshotLane,
    TeamSnapshotPayload, ALERT_STREAM, ASSIGNMENT_STREAM, EVENT_STREAM, PUBSUB_ALERTS,
    PUBSUB_STATUS, PUBSUB_TASKS,
};
