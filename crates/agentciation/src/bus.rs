use anyhow::{Context, Result};
use redis::{Client, Commands};

use crate::protocol::{
    CoordinationEnvelope, LaneStateSnapshot, ALERT_STREAM, ASSIGNMENT_STREAM, EVENT_STREAM,
};

#[derive(Debug, Clone)]
pub struct CoordinationKeyspace {
    pub events_stream: String,
    pub assignments_stream: String,
    pub alerts_stream: String,
    pub lane_state_prefix: String,
    pub team_snapshot_prefix: String,
}

impl Default for CoordinationKeyspace {
    fn default() -> Self {
        Self {
            events_stream: EVENT_STREAM.to_string(),
            assignments_stream: ASSIGNMENT_STREAM.to_string(),
            alerts_stream: ALERT_STREAM.to_string(),
            lane_state_prefix: "vw:lane".to_string(),
            team_snapshot_prefix: "vw:team".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RedisCoordinator {
    client: Client,
    keyspace: CoordinationKeyspace,
}

impl RedisCoordinator {
    pub fn new(redis_url: &str) -> Result<Self> {
        Self::with_keyspace(redis_url, CoordinationKeyspace::default())
    }

    pub fn with_keyspace(redis_url: &str, keyspace: CoordinationKeyspace) -> Result<Self> {
        let client = Client::open(redis_url)
            .with_context(|| format!("failed to create redis client for {redis_url}"))?;
        Ok(Self { client, keyspace })
    }

    #[must_use]
    pub fn keyspace(&self) -> &CoordinationKeyspace {
        &self.keyspace
    }

    pub fn append_event(&self, envelope: &CoordinationEnvelope) -> Result<()> {
        self.append_json(&self.keyspace.events_stream, envelope)
    }

    pub fn append_assignment(&self, envelope: &CoordinationEnvelope) -> Result<()> {
        self.append_json(&self.keyspace.assignments_stream, envelope)
    }

    pub fn append_alert(&self, envelope: &CoordinationEnvelope) -> Result<()> {
        self.append_json(&self.keyspace.alerts_stream, envelope)
    }

    pub fn publish_json(&self, channel: &str, envelope: &CoordinationEnvelope) -> Result<()> {
        let payload = serde_json::to_string(envelope)?;
        let mut connection = self.client.get_connection()?;
        let _: usize = connection.publish(channel, payload)?;
        Ok(())
    }

    pub fn write_lane_state(&self, snapshot: &LaneStateSnapshot) -> Result<()> {
        let key = format!(
            "{}:{}:state",
            self.keyspace.lane_state_prefix, snapshot.lane_id
        );
        let payload = serde_json::to_string(snapshot)?;
        let mut connection = self.client.get_connection()?;
        let _: () = connection.set(key, payload)?;
        Ok(())
    }

    pub fn write_team_snapshot(&self, team_id: &str, payload_json: &str) -> Result<()> {
        let key = format!(
            "{}:{}:snapshot",
            self.keyspace.team_snapshot_prefix, team_id
        );
        let mut connection = self.client.get_connection()?;
        let _: () = connection.set(key, payload_json)?;
        Ok(())
    }

    fn append_json(&self, stream: &str, envelope: &CoordinationEnvelope) -> Result<()> {
        let payload = serde_json::to_string(envelope)?;
        let mut connection = self.client.get_connection()?;
        let _: String = redis::cmd("XADD")
            .arg(stream)
            .arg("*")
            .arg("kind")
            .arg(serde_json::to_string(&envelope.kind)?)
            .arg("json")
            .arg(payload)
            .query(&mut connection)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CoordinationKeyspace;

    #[test]
    fn default_keyspace_uses_versioned_prefixes() {
        let keyspace = CoordinationKeyspace::default();
        assert_eq!(keyspace.events_stream, "vw:coord:v1:events");
        assert_eq!(keyspace.assignments_stream, "vw:coord:v1:assignments");
        assert_eq!(keyspace.alerts_stream, "vw:coord:v1:alerts");
    }
}
