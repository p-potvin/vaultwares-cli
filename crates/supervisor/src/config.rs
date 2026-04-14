#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisorConfig {
    pub redis_url: String,
    pub heartbeat_ttl_secs: u64,
    pub max_missed_heartbeats: u32,
    pub trusted_roots: Vec<String>,
    pub auto_recover_prompt_misdelivery: bool,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1/".to_string(),
            heartbeat_ttl_secs: 30,
            max_missed_heartbeats: 5,
            trusted_roots: Vec::new(),
            auto_recover_prompt_misdelivery: true,
        }
    }
}
