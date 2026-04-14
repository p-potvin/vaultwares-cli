use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SessionPaths {
    pub state_dir: PathBuf,
    pub sessions_dir: PathBuf,
    pub transcripts_dir: PathBuf,
}

impl SessionPaths {
    pub fn discover(cwd: &Path) -> Self {
        let state_dir = cwd.join(".vaultwares-cli");
        Self {
            sessions_dir: state_dir.join("sessions"),
            transcripts_dir: state_dir.join("transcripts"),
            state_dir,
        }
    }
}
