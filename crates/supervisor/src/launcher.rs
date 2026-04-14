use std::path::PathBuf;
use std::process::{Child, Command};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::registry::TerminalTarget;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLaunchSpec {
    pub lane_id: String,
    pub worker_name: String,
    pub cwd: PathBuf,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    pub terminal_target: TerminalTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLineSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub struct WorkerProcessHandle {
    pub lane_id: String,
    pub child: Child,
}

pub trait TerminalLauncher {
    fn launch_visible(&self, spec: &WorkerLaunchSpec) -> Result<WorkerProcessHandle>;
}

#[derive(Debug, Default)]
pub struct WindowsTerminalLauncher;

impl WindowsTerminalLauncher {
    #[must_use]
    pub fn build_command(spec: &WorkerLaunchSpec) -> CommandLineSpec {
        let title = format!("VaultWares CLI - {}", spec.worker_name);
        let shell_command = format!(
            "Set-Location -LiteralPath '{}'; & {}{}",
            spec.cwd.display(),
            spec.command,
            if spec.args.is_empty() {
                String::new()
            } else {
                format!(" {}", spec.args.join(" "))
            }
        );

        let mut args = match spec.terminal_target {
            TerminalTarget::WindowsTerminalTab => {
                vec!["-w", "0", "new-tab", "--title", &title]
            }
            TerminalTarget::WindowsTerminalWindow => {
                vec!["new-window", "--title", &title]
            }
            TerminalTarget::ConPtyFallback => {
                vec!["-w", "0", "new-tab", "--title", &title]
            }
        }
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        args.extend(
            ["pwsh", "-NoExit", "-Command", &shell_command]
                .into_iter()
                .map(str::to_string),
        );

        CommandLineSpec {
            program: "wt".to_string(),
            args,
        }
    }
}

impl TerminalLauncher for WindowsTerminalLauncher {
    fn launch_visible(&self, spec: &WorkerLaunchSpec) -> Result<WorkerProcessHandle> {
        let command_spec = Self::build_command(spec);
        let child = Command::new(&command_spec.program)
            .args(&command_spec.args)
            .spawn()
            .with_context(|| "failed to launch visible worker terminal via wt")?;
        Ok(WorkerProcessHandle {
            lane_id: spec.lane_id.clone(),
            child,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{WindowsTerminalLauncher, WorkerLaunchSpec};
    use crate::registry::TerminalTarget;

    #[test]
    fn builds_windows_terminal_command_for_tab_launch() {
        let spec = WorkerLaunchSpec {
            lane_id: "lane_1".to_string(),
            worker_name: "worker-a".to_string(),
            cwd: PathBuf::from("C:\\repo"),
            command: "cargo".to_string(),
            args: vec!["run".to_string()],
            terminal_target: TerminalTarget::WindowsTerminalTab,
        };

        let command = WindowsTerminalLauncher::build_command(&spec);
        assert_eq!(command.program, "wt");
        assert!(command.args.iter().any(|value| value == "new-tab"));
        assert!(command
            .args
            .iter()
            .any(|value| value.contains("VaultWares CLI - worker-a")));
    }
}
