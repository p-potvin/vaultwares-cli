mod config;
mod launcher;
mod manager;
mod registry;

pub use config::SupervisorConfig;
pub use launcher::{
    CommandLineSpec, TerminalLauncher, WindowsTerminalLauncher, WorkerLaunchSpec,
    WorkerProcessHandle,
};
pub use manager::{LaneSupervisor, SpawnedLane};
pub use registry::{
    AssignmentRecord, LaneHandle, LaneLease, LaneRegistration, LaneState, SupervisorAlert,
    TerminalTarget,
};
