use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use agentciation::{
    AlertSeverity, CoordinationEnvelope, CoordinationMessageKind, CoordinationPayload,
    LaneAcceptedPayload, LaneAssignPayload, LaneCompletePayload, LaneFailPayload,
    LaneHeartbeatPayload, LaneLifecycleState, LaneRegisterPayload, LaneStateSnapshot,
    ManagerAlertPayload, RedisCoordinator, TaskPacketWire, TeamSnapshotLane, TeamSnapshotPayload,
    PUBSUB_ALERTS, PUBSUB_STATUS, PUBSUB_TASKS,
};
use anyhow::{anyhow, Result};
use chrono::Utc;
use runtime::{SessionStore, TaskPacket, WorkerRegistry};

use crate::config::SupervisorConfig;
use crate::launcher::{TerminalLauncher, WorkerLaunchSpec, WorkerProcessHandle};
use crate::registry::{AssignmentRecord, LaneHandle, LaneRegistration, LaneState, SupervisorAlert};

#[derive(Debug)]
pub struct SpawnedLane {
    pub lane: LaneHandle,
    pub process: WorkerProcessHandle,
}

#[derive(Debug)]
pub struct LaneSupervisor {
    config: SupervisorConfig,
    coordinator: Option<RedisCoordinator>,
    worker_registry: WorkerRegistry,
    lanes: Arc<Mutex<HashMap<String, LaneHandle>>>,
    assignments: Arc<Mutex<HashMap<String, AssignmentRecord>>>,
}

impl LaneSupervisor {
    pub fn new(config: SupervisorConfig) -> Result<Self> {
        Ok(Self {
            coordinator: Some(RedisCoordinator::new(&config.redis_url)?),
            config,
            worker_registry: WorkerRegistry::new(),
            lanes: Arc::new(Mutex::new(HashMap::new())),
            assignments: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    #[must_use]
    pub fn new_offline(config: SupervisorConfig) -> Self {
        Self {
            config,
            coordinator: None,
            worker_registry: WorkerRegistry::new(),
            lanes: Arc::new(Mutex::new(HashMap::new())),
            assignments: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_lane(&self, registration: LaneRegistration) -> Result<LaneHandle> {
        let worker_id = registration
            .worker_id
            .clone()
            .ok_or_else(|| anyhow!("lane registration missing worker_id"))?;
        let session_store = SessionStore::from_cwd(&registration.cwd)?;
        let lane = LaneHandle {
            lane_id: registration.lane_id.clone(),
            worker_name: registration.worker_name.clone(),
            worker_id,
            cwd: registration.cwd.clone(),
            session_dir: session_store.sessions_dir().to_path_buf(),
            state: LaneState::Idle,
            terminal_target: registration.terminal_target.clone(),
            active_assignment_id: None,
            last_seen_at: now_rfc3339(),
            missed_heartbeats: 0,
            catalog_revision: registration.catalog_revision.clone(),
        };
        self.lanes
            .lock()
            .expect("lane registry lock poisoned")
            .insert(lane.lane_id.clone(), lane.clone());

        let payload = LaneRegisterPayload {
            role: registration.role,
            capabilities: registration.capabilities,
            cwd: registration.cwd.display().to_string(),
            catalog_revision: registration.catalog_revision.unwrap_or_default(),
            terminal_target: Some(registration.terminal_target.as_str().to_string()),
            worker_id: Some(lane.worker_id.clone()),
        };
        let mut envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::LaneRegister,
            message_id("register"),
            CoordinationPayload::LaneRegister(payload),
        );
        envelope.lane_id = Some(lane.lane_id.clone());
        envelope.agent_id = Some(lane.worker_name.clone());

        self.publish_event(&envelope)?;
        self.publish_json(PUBSUB_STATUS, &envelope)?;
        self.write_lane_state(&self.snapshot_for(&lane))?;
        Ok(lane)
    }

    pub fn spawn_visible_lane<L: TerminalLauncher>(
        &self,
        launcher: &L,
        spec: WorkerLaunchSpec,
        role: String,
        capabilities: Vec<String>,
        catalog_revision: Option<String>,
    ) -> Result<SpawnedLane> {
        let worker = self.worker_registry.create(
            &spec.cwd.display().to_string(),
            &self.config.trusted_roots,
            self.config.auto_recover_prompt_misdelivery,
        );
        let process = launcher.launch_visible(&spec)?;
        let lane = self.register_lane(LaneRegistration {
            lane_id: spec.lane_id,
            worker_name: spec.worker_name,
            cwd: spec.cwd,
            role,
            capabilities,
            terminal_target: spec.terminal_target,
            catalog_revision,
            worker_id: Some(worker.worker_id),
        })?;
        Ok(SpawnedLane { lane, process })
    }

    pub fn record_heartbeat(
        &self,
        lane_id: &str,
        state: LaneState,
        active_assignment_id: Option<String>,
    ) -> Result<LaneHandle> {
        let mut lanes = self.lanes.lock().expect("lane registry lock poisoned");
        let lane = lanes
            .get_mut(lane_id)
            .ok_or_else(|| anyhow!("unknown lane: {lane_id}"))?;
        lane.state = state;
        lane.active_assignment_id = active_assignment_id.clone();
        lane.last_seen_at = now_rfc3339();
        lane.missed_heartbeats = 0;

        let payload = LaneHeartbeatPayload {
            state: LaneLifecycleState::from(lane.state),
            missed_heartbeats: lane.missed_heartbeats,
            active_assignment_id,
        };
        let mut envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::LaneHeartbeat,
            message_id("heartbeat"),
            CoordinationPayload::LaneHeartbeat(payload),
        );
        envelope.lane_id = Some(lane.lane_id.clone());
        envelope.agent_id = Some(lane.worker_name.clone());

        self.publish_event(&envelope)?;
        self.publish_json(PUBSUB_STATUS, &envelope)?;
        self.write_lane_state(&self.snapshot_for(lane))?;
        Ok(lane.clone())
    }

    pub fn assign_task(
        &self,
        lane_id: &str,
        packet: TaskPacket,
        team_id: Option<String>,
        workflow_id: Option<String>,
        correlation_id: Option<String>,
    ) -> Result<AssignmentRecord> {
        let mut lanes = self.lanes.lock().expect("lane registry lock poisoned");
        let lane = lanes
            .get_mut(lane_id)
            .ok_or_else(|| anyhow!("unknown lane: {lane_id}"))?;
        let assignment = AssignmentRecord {
            assignment_id: message_id("assign"),
            team_id: team_id.clone(),
            target_lane: lane_id.to_string(),
            created_at: now_rfc3339(),
            correlation_id: correlation_id.clone(),
        };
        lane.state = LaneState::Assigned;
        lane.active_assignment_id = Some(assignment.assignment_id.clone());

        self.assignments
            .lock()
            .expect("assignment registry lock poisoned")
            .insert(assignment.assignment_id.clone(), assignment.clone());

        let payload = LaneAssignPayload {
            assignment_id: assignment.assignment_id.clone(),
            task_packet: packet_to_wire(packet),
            workflow_id,
            target_lane: Some(lane_id.to_string()),
        };
        let mut envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::LaneAssign,
            message_id("lane-assign"),
            CoordinationPayload::LaneAssign(payload),
        );
        envelope.team_id = team_id;
        envelope.lane_id = Some(lane_id.to_string());
        envelope.agent_id = Some(lane.worker_name.clone());
        envelope.correlation_id = correlation_id;

        self.publish_assignment(&envelope)?;
        self.publish_json(PUBSUB_TASKS, &envelope)?;
        self.write_lane_state(&self.snapshot_for(lane))?;
        Ok(assignment)
    }

    pub fn mark_accepted(&self, lane_id: &str, assignment_id: &str) -> Result<()> {
        let lanes = self.lanes.lock().expect("lane registry lock poisoned");
        let lane = lanes
            .get(lane_id)
            .ok_or_else(|| anyhow!("unknown lane: {lane_id}"))?;
        let mut envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::LaneAccepted,
            message_id("accepted"),
            CoordinationPayload::LaneAccepted(LaneAcceptedPayload {
                assignment_id: assignment_id.to_string(),
            }),
        );
        envelope.lane_id = Some(lane_id.to_string());
        envelope.agent_id = Some(lane.worker_name.clone());
        self.publish_event(&envelope)?;
        self.publish_json(PUBSUB_STATUS, &envelope)?;
        Ok(())
    }

    pub fn mark_complete(
        &self,
        lane_id: &str,
        summary: String,
        artifact_paths: Vec<String>,
    ) -> Result<LaneHandle> {
        let mut lanes = self.lanes.lock().expect("lane registry lock poisoned");
        let lane = lanes
            .get_mut(lane_id)
            .ok_or_else(|| anyhow!("unknown lane: {lane_id}"))?;
        let assignment_id = lane.active_assignment_id.clone().unwrap_or_default();
        lane.state = LaneState::Finished;
        lane.last_seen_at = now_rfc3339();

        let payload = LaneCompletePayload {
            assignment_id,
            summary,
            artifacts: agentciation::LaneArtifacts {
                paths: artifact_paths,
            },
        };
        let mut envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::LaneComplete,
            message_id("complete"),
            CoordinationPayload::LaneComplete(payload),
        );
        envelope.lane_id = Some(lane_id.to_string());
        envelope.agent_id = Some(lane.worker_name.clone());
        self.publish_event(&envelope)?;
        self.publish_json(PUBSUB_STATUS, &envelope)?;
        self.write_lane_state(&self.snapshot_for(lane))?;
        Ok(lane.clone())
    }

    pub fn mark_failed(
        &self,
        lane_id: &str,
        failure_class: agentciation::FailureClass,
        detail: String,
    ) -> Result<(LaneHandle, SupervisorAlert)> {
        let mut lanes = self.lanes.lock().expect("lane registry lock poisoned");
        let lane = lanes
            .get_mut(lane_id)
            .ok_or_else(|| anyhow!("unknown lane: {lane_id}"))?;
        let assignment_id = lane.active_assignment_id.clone().unwrap_or_default();
        lane.state = LaneState::Failed;
        lane.last_seen_at = now_rfc3339();

        let fail_payload = LaneFailPayload {
            assignment_id,
            failure_class,
            detail: detail.clone(),
        };
        let mut fail_envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::LaneFail,
            message_id("fail"),
            CoordinationPayload::LaneFail(fail_payload),
        );
        fail_envelope.lane_id = Some(lane_id.to_string());
        fail_envelope.agent_id = Some(lane.worker_name.clone());

        let alert = SupervisorAlert {
            severity: AlertSeverity::High,
            code: "lane_failed".to_string(),
            lane_id: Some(lane_id.to_string()),
            detail,
            emitted_at: now_rfc3339(),
            failure_class: Some(failure_class),
        };
        let alert_envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::ManagerAlert,
            message_id("alert"),
            CoordinationPayload::ManagerAlert(ManagerAlertPayload {
                severity: alert.severity,
                code: alert.code.clone(),
                detail: alert.detail.clone(),
            }),
        );

        self.publish_event(&fail_envelope)?;
        self.publish_json(PUBSUB_STATUS, &fail_envelope)?;
        self.publish_alert(&alert_envelope)?;
        self.publish_json(PUBSUB_ALERTS, &alert_envelope)?;
        self.write_lane_state(&self.snapshot_for(lane))?;
        Ok((lane.clone(), alert))
    }

    #[must_use] 
    pub fn stale_alerts(&self) -> Vec<SupervisorAlert> {
        let now = now_epoch_secs();
        self.lanes
            .lock()
            .expect("lane registry lock poisoned")
            .values()
            .filter_map(|lane| {
                let last_seen = chrono::DateTime::parse_from_rfc3339(&lane.last_seen_at)
                    .ok()?
                    .timestamp() as u64;
                let elapsed = now.saturating_sub(last_seen);
                let missed = (elapsed / self.config.heartbeat_ttl_secs) as u32;
                (missed >= self.config.max_missed_heartbeats).then(|| SupervisorAlert {
                    severity: AlertSeverity::Critical,
                    code: "missed_heartbeats".to_string(),
                    lane_id: Some(lane.lane_id.clone()),
                    detail: format!("lane {} missed {} heartbeat windows", lane.lane_id, missed),
                    emitted_at: now_rfc3339(),
                    failure_class: None,
                })
            })
            .collect()
    }

    pub fn emit_team_snapshot(&self, team_id: &str, catalog_revision: String) -> Result<String> {
        let lanes = self.lanes.lock().expect("lane registry lock poisoned");
        let payload = TeamSnapshotPayload {
            catalog_revision,
            lanes: lanes
                .values()
                .map(|lane| TeamSnapshotLane {
                    lane_id: lane.lane_id.clone(),
                    agent_id: lane.worker_name.clone(),
                    state: lane.state.into(),
                    last_seen_at: lane.last_seen_at.clone(),
                    active_assignment_id: lane.active_assignment_id.clone(),
                })
                .collect(),
        };
        let mut envelope = CoordinationEnvelope::new(
            CoordinationMessageKind::TeamSnapshot,
            message_id("snapshot"),
            CoordinationPayload::TeamSnapshot(payload),
        );
        envelope.team_id = Some(team_id.to_string());
        let json = serde_json::to_string(&envelope)?;
        self.write_team_snapshot(team_id, &json)?;
        self.publish_event(&envelope)?;
        self.publish_json(PUBSUB_STATUS, &envelope)?;
        Ok(json)
    }

    #[must_use]
    pub fn lane(&self, lane_id: &str) -> Option<LaneHandle> {
        self.lanes
            .lock()
            .expect("lane registry lock poisoned")
            .get(lane_id)
            .cloned()
    }

    fn snapshot_for(&self, lane: &LaneHandle) -> LaneStateSnapshot {
        LaneStateSnapshot {
            lane_id: lane.lane_id.clone(),
            agent_id: lane.worker_name.clone(),
            state: lane.state.into(),
            last_seen_at: lane.last_seen_at.clone(),
            active_assignment_id: lane.active_assignment_id.clone(),
            catalog_revision: lane.catalog_revision.clone(),
        }
    }

    fn publish_event(&self, envelope: &CoordinationEnvelope) -> Result<()> {
        if let Some(coordinator) = &self.coordinator {
            coordinator.append_event(envelope)?;
        }
        Ok(())
    }

    fn publish_assignment(&self, envelope: &CoordinationEnvelope) -> Result<()> {
        if let Some(coordinator) = &self.coordinator {
            coordinator.append_assignment(envelope)?;
        }
        Ok(())
    }

    fn publish_alert(&self, envelope: &CoordinationEnvelope) -> Result<()> {
        if let Some(coordinator) = &self.coordinator {
            coordinator.append_alert(envelope)?;
        }
        Ok(())
    }

    fn publish_json(&self, channel: &str, envelope: &CoordinationEnvelope) -> Result<()> {
        if let Some(coordinator) = &self.coordinator {
            coordinator.publish_json(channel, envelope)?;
        }
        Ok(())
    }

    fn write_lane_state(&self, snapshot: &LaneStateSnapshot) -> Result<()> {
        if let Some(coordinator) = &self.coordinator {
            coordinator.write_lane_state(snapshot)?;
        }
        Ok(())
    }

    fn write_team_snapshot(&self, team_id: &str, payload_json: &str) -> Result<()> {
        if let Some(coordinator) = &self.coordinator {
            coordinator.write_team_snapshot(team_id, payload_json)?;
        }
        Ok(())
    }
}

fn packet_to_wire(packet: TaskPacket) -> TaskPacketWire {
    TaskPacketWire {
        objective: packet.objective,
        scope: packet.scope,
        repo: packet.repo,
        branch_policy: packet.branch_policy,
        acceptance_tests: packet.acceptance_tests,
        commit_policy: packet.commit_policy,
        reporting_contract: packet.reporting_contract,
        escalation_policy: packet.escalation_policy,
    }
}

fn message_id(prefix: &str) -> String {
    format!("{prefix}_{:x}", now_epoch_secs())
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use agentciation::FailureClass;
    use runtime::TaskPacket;

    use super::LaneSupervisor;
    use crate::config::SupervisorConfig;
    use crate::registry::{LaneRegistration, LaneState, TerminalTarget};

    #[test]
    fn records_lane_assignment_and_failure_in_memory() {
        let supervisor = LaneSupervisor::new_offline(SupervisorConfig::default());
        let lane = supervisor
            .register_lane(LaneRegistration {
                lane_id: "lane_1".to_string(),
                worker_name: "worker-a".to_string(),
                cwd: PathBuf::from("."),
                role: "worker".to_string(),
                capabilities: vec!["captioning".to_string()],
                terminal_target: TerminalTarget::WindowsTerminalTab,
                catalog_revision: Some("rev-1".to_string()),
                worker_id: Some("worker_123".to_string()),
            })
            .expect("register lane");
        assert_eq!(lane.state, LaneState::Idle);

        let assignment = supervisor
            .assign_task(
                "lane_1",
                TaskPacket {
                    objective: "Ship it".to_string(),
                    scope: "cli".to_string(),
                    repo: "vaultwares-cli".to_string(),
                    branch_policy: "main".to_string(),
                    acceptance_tests: vec!["cargo test".to_string()],
                    commit_policy: "single".to_string(),
                    reporting_contract: "report".to_string(),
                    escalation_policy: "escalate".to_string(),
                },
                Some("team_1".to_string()),
                Some("team-exec".to_string()),
                Some("cid_1".to_string()),
            )
            .expect("assign task");
        assert_eq!(assignment.target_lane, "lane_1");

        let (failed_lane, alert) = supervisor
            .mark_failed(
                "lane_1",
                FailureClass::ToolRuntime,
                "tool crashed".to_string(),
            )
            .expect("mark failed");
        assert_eq!(failed_lane.state, LaneState::Failed);
        assert_eq!(alert.code, "lane_failed");
    }
}
