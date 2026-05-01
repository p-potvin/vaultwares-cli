use crate::*;

#[derive(Debug, Clone, Default)]
pub struct StatusContext {
    pub(crate) cwd: PathBuf,
    pub(crate) session_path: Option<PathBuf>,
    pub(crate) loaded_config_files: usize,
    pub(crate) discovered_config_files: usize,
    pub(crate) memory_file_count: usize,
    pub(crate) project_root: Option<PathBuf>,
    pub(crate) git_branch: Option<String>,
    pub(crate) git_summary: GitWorkspaceSummary,
    pub(crate) sandbox_status: runtime::SandboxStatus,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StatusUsage {
    pub(crate) message_count: usize,
    pub(crate) turns: u32,
    pub(crate) latest: TokenUsage,
    pub(crate) cumulative: TokenUsage,
    pub(crate) estimated_tokens: usize,
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GitWorkspaceSummary {
    pub(crate) changed_files: usize,
    pub(crate) staged_files: usize,
    pub(crate) unstaged_files: usize,
    pub(crate) untracked_files: usize,
    pub(crate) conflicted_files: usize,
}

impl GitWorkspaceSummary {
    pub(crate) fn is_clean(self) -> bool {
        self.changed_files == 0
    }

    pub(crate) fn headline(self) -> String {
        if self.is_clean() {
            "clean".to_string()
        } else {
            let mut details = Vec::new();
            if self.staged_files > 0 {
                details.push(format!("{} staged", self.staged_files));
            }
            if self.unstaged_files > 0 {
                details.push(format!("{} unstaged", self.unstaged_files));
            }
            if self.untracked_files > 0 {
                details.push(format!("{} untracked", self.untracked_files));
            }
            if self.conflicted_files > 0 {
                details.push(format!("{} conflicted", self.conflicted_files));
            }
            format!(
                "dirty · {} files · {}",
                self.changed_files,
                details.join(", ")
            )
        }
    }
}

pub(crate) fn parse_git_status_metadata(status: Option<&str>) -> (Option<PathBuf>, Option<String>) {
    parse_git_status_metadata_for(
        &env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        status,
    )
}

pub(crate) fn parse_git_status_branch(status: Option<&str>) -> Option<String> {
    let status = status?;
    let first_line = status.lines().next()?;
    let line = first_line.strip_prefix("## ")?;
    if line.starts_with("HEAD") {
        return Some("detached HEAD".to_string());
    }
    let branch = line.split(['.', ' ']).next().unwrap_or_default().trim();
    if branch.is_empty() {
        None
    } else {
        Some(branch.to_string())
    }
}

pub(crate) fn parse_git_workspace_summary(status: Option<&str>) -> GitWorkspaceSummary {
    let mut summary = GitWorkspaceSummary::default();
    let Some(status) = status else {
        return summary;
    };

    for line in status.lines() {
        if line.starts_with("## ") || line.trim().is_empty() {
            continue;
        }

        summary.changed_files += 1;
        let mut chars = line.chars();
        let index_status = chars.next().unwrap_or(' ');
        let worktree_status = chars.next().unwrap_or(' ');

        if index_status == '?' && worktree_status == '?' {
            summary.untracked_files += 1;
            continue;
        }

        if index_status != ' ' {
            summary.staged_files += 1;
        }
        if worktree_status != ' ' {
            summary.unstaged_files += 1;
        }
        if (matches!(index_status, 'U' | 'A') && matches!(worktree_status, 'U' | 'A'))
            || index_status == 'U'
            || worktree_status == 'U'
        {
            summary.conflicted_files += 1;
        }
    }

    summary
}

pub(crate) fn resolve_git_branch_for(cwd: &Path) -> Option<String> {
    let branch = run_git_capture_in(cwd, &["branch", "--show-current"])?;
    let branch = branch.trim();
    if !branch.is_empty() {
        return Some(branch.to_string());
    }

    let fallback = run_git_capture_in(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let fallback = fallback.trim();
    if fallback.is_empty() {
        None
    } else if fallback == "HEAD" {
        Some("detached HEAD".to_string())
    } else {
        Some(fallback.to_string())
    }
}

pub(crate) fn run_git_capture_in(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

pub(crate) fn find_git_root_in(cwd: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()?;
    if !output.status.success() {
        return Err("not a git repository".into());
    }
    let path = String::from_utf8(output.stdout)?.trim().to_string();
    if path.is_empty() {
        return Err("empty git root".into());
    }
    Ok(PathBuf::from(path))
}

pub(crate) fn parse_git_status_metadata_for(
    cwd: &Path,
    status: Option<&str>,
) -> (Option<PathBuf>, Option<String>) {
    let branch = resolve_git_branch_for(cwd).or_else(|| parse_git_status_branch(status));
    let project_root = find_git_root_in(cwd).ok();
    (project_root, branch)
}

pub(crate) fn print_status_snapshot(
    model: &str,
    permission_mode: PermissionMode,
    output_format: CliOutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let usage = StatusUsage {
        message_count: 0,
        turns: 0,
        latest: TokenUsage::default(),
        cumulative: TokenUsage::default(),
        estimated_tokens: 0,
    };
    let context = status_context(None)?;
    match output_format {
        CliOutputFormat::Text => println!(
            "{}",
            format_status_report(model, usage, permission_mode.as_str(), &context)
        ),
        CliOutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&status_json_value(
                Some(model),
                usage,
                permission_mode.as_str(),
                &context,
            ))?
        ),
    }
    Ok(())
}

pub(crate) fn status_json_value(
    model: Option<&str>,
    usage: StatusUsage,
    permission_mode: &str,
    context: &StatusContext,
) -> serde_json::Value {
    json!({
        "kind": "status",
        "model": model,
        "permission_mode": permission_mode,
        "usage": {
            "messages": usage.message_count,
            "turns": usage.turns,
            "latest_total": usage.latest.total_tokens(),
            "cumulative_input": usage.cumulative.input_tokens,
            "cumulative_output": usage.cumulative.output_tokens,
            "cumulative_total": usage.cumulative.total_tokens(),
            "estimated_tokens": usage.estimated_tokens,
        },
        "workspace": {
            "cwd": context.cwd,
            "project_root": context.project_root,
            "git_branch": context.git_branch,
            "git_state": context.git_summary.headline(),
            "changed_files": context.git_summary.changed_files,
            "staged_files": context.git_summary.staged_files,
            "unstaged_files": context.git_summary.unstaged_files,
            "untracked_files": context.git_summary.untracked_files,
            "session": context.session_path.as_ref().map_or_else(|| "live-repl".to_string(), |path| path.display().to_string()),
            "session_id": context.session_path.as_ref().and_then(|path| path.file_stem().map(|name| name.to_string_lossy().into_owned())),
            "loaded_config_files": context.loaded_config_files,
            "discovered_config_files": context.discovered_config_files,
            "memory_file_count": context.memory_file_count,
        },
        "sandbox": {
            "enabled": context.sandbox_status.enabled,
            "active": context.sandbox_status.active,
            "supported": context.sandbox_status.supported,
            "in_container": context.sandbox_status.in_container,
            "requested_namespace": context.sandbox_status.requested.namespace_restrictions,
            "active_namespace": context.sandbox_status.namespace_active,
            "requested_network": context.sandbox_status.requested.network_isolation,
            "active_network": context.sandbox_status.network_active,
            "filesystem_mode": context.sandbox_status.filesystem_mode.as_str(),
            "filesystem_active": context.sandbox_status.filesystem_active,
            "allowed_mounts": context.sandbox_status.allowed_mounts,
            "markers": context.sandbox_status.container_markers,
            "fallback_reason": context.sandbox_status.fallback_reason,
        }
    })
}

pub(crate) fn status_context(
    session_path: Option<&Path>,
) -> Result<StatusContext, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let loader = ConfigLoader::default_for(&cwd);
    let discovered_config_files = loader.discover().len();
    let runtime_config = loader.load()?;
    let project_context = ProjectContext::discover_with_git(&cwd, DEFAULT_DATE)?;
    let (project_root, git_branch) =
        parse_git_status_metadata(project_context.git_status.as_deref());
    let git_summary = parse_git_workspace_summary(project_context.git_status.as_deref());
    let sandbox_status = resolve_sandbox_status(runtime_config.sandbox(), &cwd);
    Ok(StatusContext {
        cwd,
        session_path: session_path.map(Path::to_path_buf),
        loaded_config_files: runtime_config.loaded_entries().len(),
        discovered_config_files,
        memory_file_count: project_context.instruction_files.len(),
        project_root,
        git_branch,
        git_summary,
        sandbox_status,
    })
}

pub(crate) fn format_status_report(
    model: &str,
    usage: StatusUsage,
    permission_mode: &str,
    context: &StatusContext,
) -> String {
    [
        format!(
            "Status
  Model            {model}
  Permission mode  {permission_mode}
  Messages         {}
  Turns            {}
  Estimated tokens {}",
            usage.message_count, usage.turns, usage.estimated_tokens,
        ),
        format!(
            "Usage
  Latest total     {}
  Cumulative input {}
  Cumulative output {}
  Cumulative total {}",
            usage.latest.total_tokens(),
            usage.cumulative.input_tokens,
            usage.cumulative.output_tokens,
            usage.cumulative.total_tokens(),
        ),
        format!(
            "Workspace
  Cwd              {}
  Project root     {}
  Git branch       {}
  Git state        {}
  Changed files    {}
  Staged           {}
  Unstaged         {}
  Untracked        {}
  Session          {}
  Config files     loaded {}/{}
  Memory files     {}
  Suggested flow   /status → /diff → /commit",
            context.cwd.display(),
            context
                .project_root
                .as_ref()
                .map_or_else(|| "unknown".to_string(), |path| path.display().to_string()),
            context.git_branch.as_deref().unwrap_or("unknown"),
            context.git_summary.headline(),
            context.git_summary.changed_files,
            context.git_summary.staged_files,
            context.git_summary.unstaged_files,
            context.git_summary.untracked_files,
            context.session_path.as_ref().map_or_else(
                || "live-repl".to_string(),
                |path| path.display().to_string()
            ),
            context.loaded_config_files,
            context.discovered_config_files,
            context.memory_file_count,
        ),
        format_sandbox_report(&context.sandbox_status),
    ]
    .join("\n\n")
}

/// How many rows the HUD occupies at the bottom of the terminal.
/// Callers must reserve this many lines when computing scroll regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudHeight {
    /// A two-row HUD (context row + usage row).
    Double,
    /// Collapsed to a single row (narrow terminals < 60 cols).
    Single,
}

impl HudHeight {
    pub(crate) fn rows(self) -> u16 {
        match self {
            Self::Double => 2,
            Self::Single => 1,
        }
    }
}

/// Render the HUD at the bottom of the terminal.
///
/// Returns the [`HudHeight`] that was actually drawn so callers can adjust
/// scroll-region reservations without an extra `terminal::size()` call.
pub(crate) fn draw_hud(
    model: &str,
    permission_mode: &str,
    usage: StatusUsage,
    context: &StatusContext,
) -> io::Result<HudHeight> {
    use crossterm::{
        cursor,
        style::{self, Color, Stylize},
        terminal::{self, ClearType},
        QueueableCommand,
    };
    use std::io::stdout;

    let (width, height) = terminal::size().unwrap_or((80, 24));
    let mut out = stdout();

    // ── helpers ──────────────────────────────────────────────────────────────
    /// Pad / truncate a string to exactly `w` *display* columns.
    fn fit(s: &str, w: usize) -> String {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() >= w {
            chars.into_iter().take(w).collect()
        } else {
            format!("{s:<width$}", width = w)
        }
    }

    // ── data extraction ───────────────────────────────────────────────────────
    let session_id = context
        .session_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "live".to_string());

    // Shorten UUID-style session IDs to the first 8 chars for the HUD.
    let session_short = if session_id.len() > 12 {
        format!("{}…", &session_id[..8])
    } else {
        session_id.clone()
    };

    let branch = context.git_branch.as_deref().unwrap_or("—");
    let git_dirty = if context.git_summary.is_clean() {
        "✓".to_string()
    } else {
        format!("~{}", context.git_summary.changed_files)
    };

    let pricing = pricing_for_model(model).unwrap_or_else(ModelPricing::default_sonnet_tier);
    let cost = usage.cumulative.estimate_cost_usd_with_pricing(pricing);
    let cost_str = format_usd(cost.total_cost_usd());

    let w = width as usize;

    // ── narrow / single-row fallback ─────────────────────────────────────────
    if w < 60 {
        let line = fit(
            &format!(
                " {} │ {} │ {} │ in:{} out:{} {}",
                model,
                permission_mode,
                session_short,
                usage.cumulative.input_tokens,
                usage.cumulative.output_tokens,
                cost_str,
            ),
            w,
        );
        out.queue(cursor::SavePosition)?;
        out.queue(cursor::Hide)?;
        out.queue(cursor::MoveTo(0, height.saturating_sub(1)))?;
        out.queue(terminal::Clear(ClearType::CurrentLine))?;
        out.queue(style::PrintStyledContent(
            line.with(Color::Black).on(Color::DarkCyan).bold(),
        ))?;
        out.queue(cursor::Show)?;
        out.queue(cursor::RestorePosition)?;
        out.flush()?;
        return Ok(HudHeight::Single);
    }

    // ── dual-row HUD ─────────────────────────────────────────────────────────
    //
    // Row 1  (context): CLAW │ model │ permission │ session │  branch  git
    // Row 2  (usage  ): turns │ msgs │ in:N out:N │ est:N │  cost  │ sandbox

    // -- row 1 --
    let perm_icon = match permission_mode {
        "bypassPermissions" | "dangerFullAccess" => "⚡",
        "readOnly"           => "👁",
        _                    => "✏",
    };
    let sandbox_icon = if context.sandbox_status.active { "🔒" } else { "  " };

    let row1 = fit(
        &format!(
            "  CLAW  │  {}  │  {} {}  │  session: {}  │  {} {}  {}",
            model,
            perm_icon,
            permission_mode,
            session_short,
            branch,
            git_dirty,
            sandbox_icon,
        ),
        w,
    );

    // -- row 2 --
    let row2 = fit(
        &format!(
            "  turns: {} │ msgs: {} │ in: {} out: {} cache: {} │ est: {} tok │ {}",
            usage.turns,
            usage.message_count,
            usage.cumulative.input_tokens,
            usage.cumulative.output_tokens,
            usage.cumulative.cache_creation_input_tokens
                + usage.cumulative.cache_read_input_tokens,
            usage.estimated_tokens,
            cost_str,
        ),
        w,
    );

    let row1_y = height.saturating_sub(2);
    let row2_y = height.saturating_sub(1);

    out.queue(cursor::SavePosition)?;
    out.queue(cursor::Hide)?;

    // Row 1 — dark navy + bright white text
    out.queue(cursor::MoveTo(0, row1_y))?;
    out.queue(terminal::Clear(ClearType::CurrentLine))?;
    out.queue(style::PrintStyledContent(
        row1.with(Color::White)
            .on(Color::Rgb { r: 15, g: 23, b: 42 })
            .bold(),
    ))?;

    // Row 2 — slightly lighter slate, dimmer text
    out.queue(cursor::MoveTo(0, row2_y))?;
    out.queue(terminal::Clear(ClearType::CurrentLine))?;
    out.queue(style::PrintStyledContent(
        row2.with(Color::Rgb { r: 148, g: 163, b: 184 })
            .on(Color::Rgb { r: 30, g: 41, b: 59 }),
    ))?;

    out.queue(cursor::Show)?;
    out.queue(cursor::RestorePosition)?;
    out.flush()?;

    Ok(HudHeight::Double)
}

pub(crate) fn clear_hud() -> io::Result<()> {
    use crossterm::{cursor, terminal::{self, ClearType}, QueueableCommand};
    use std::io::stdout;

    let (_, height) = terminal::size().unwrap_or((80, 24));
    let mut out = stdout();

    out.queue(cursor::SavePosition)?;
    // Clear both potential rows
    out.queue(cursor::MoveTo(0, height.saturating_sub(2)))?;
    out.queue(terminal::Clear(ClearType::CurrentLine))?;
    out.queue(cursor::MoveTo(0, height.saturating_sub(1)))?;
    out.queue(terminal::Clear(ClearType::CurrentLine))?;
    out.queue(cursor::RestorePosition)?;
    out.flush()?;

    Ok(())
}


pub(crate) fn format_turn_footer(
    model: &str,
    permission_mode: &str,
    session_id: &str,
    usage: TokenUsage,
    elapsed: Duration,
) -> String {
    let pricing = pricing_for_model(model).unwrap_or_else(ModelPricing::default_sonnet_tier);
    let estimated_cost = usage.estimate_cost_usd_with_pricing(pricing);
    format!(
        "\x1b[2mTurn summary  model {model} | permissions {permission_mode} | session {session_id} | elapsed {} | tokens {} total (in {}, out {}, cache {}) | cost {}\x1b[0m",
        format_duration_compact(elapsed),
        usage.total_tokens(),
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_creation_input_tokens + usage.cache_read_input_tokens,
        format_usd(estimated_cost.total_cost_usd()),
    )
}

pub(crate) fn format_duration_compact(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    if total_seconds < 60 {
        return format!("{total_seconds}.{:01}s", duration.subsec_millis() / 100);
    }

    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}m {seconds:02}s")
}

pub(crate) fn format_sandbox_report(status: &runtime::SandboxStatus) -> String {
    format!(
        "Sandbox
  Enabled           {}
  Active            {}
  Supported         {}
  In container      {}
  Requested ns      {}
  Active ns         {}
  Requested net     {}
  Active net        {}
  Filesystem mode   {}
  Filesystem active {}
  Allowed mounts    {}
  Markers           {}
  Fallback reason   {}",
        status.enabled,
        status.active,
        status.supported,
        status.in_container,
        status.requested.namespace_restrictions,
        status.namespace_active,
        status.requested.network_isolation,
        status.network_active,
        status.filesystem_mode.as_str(),
        status.filesystem_active,
        if status.allowed_mounts.is_empty() {
            "<none>".to_string()
        } else {
            status.allowed_mounts.join(", ")
        },
        if status.container_markers.is_empty() {
            "<none>".to_string()
        } else {
            status.container_markers.join(", ")
        },
        status
            .fallback_reason
            .clone()
            .unwrap_or_else(|| "<none>".to_string()),
    )
}

pub(crate) fn format_commit_preflight_report(
    branch: Option<&str>,
    summary: GitWorkspaceSummary,
) -> String {
    format!(
        "Commit
  Result           ready
  Branch           {}
  Workspace        {}
  Changed files    {}
  Action           create a git commit from the current workspace changes",
        branch.unwrap_or("unknown"),
        summary.headline(),
        summary.changed_files,
    )
}

pub(crate) fn format_commit_skipped_report() -> String {
    "Commit
  Result           skipped
  Reason           no workspace changes
  Action           create a git commit from the current workspace changes
  Next             /status to inspect context · /diff to inspect repo changes"
        .to_string()
}

pub(crate) fn print_sandbox_status_snapshot(
    output_format: CliOutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let loader = ConfigLoader::default_for(&cwd);
    let runtime_config = loader
        .load()
        .unwrap_or_else(|_| runtime::RuntimeConfig::empty());
    let status = resolve_sandbox_status(runtime_config.sandbox(), &cwd);
    match output_format {
        CliOutputFormat::Text => println!("{}", format_sandbox_report(&status)),
        CliOutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&sandbox_json_value(&status))?
        ),
    }
    Ok(())
}

pub(crate) fn sandbox_json_value(status: &runtime::SandboxStatus) -> serde_json::Value {
    json!({
        "kind": "sandbox",
        "enabled": status.enabled,
        "active": status.active,
        "supported": status.supported,
        "in_container": status.in_container,
        "requested_namespace": status.requested.namespace_restrictions,
        "active_namespace": status.namespace_active,
        "requested_network": status.requested.network_isolation,
        "active_network": status.network_active,
        "filesystem_mode": status.filesystem_mode.as_str(),
        "filesystem_active": status.filesystem_active,
        "allowed_mounts": status.allowed_mounts,
        "markers": status.container_markers,
        "fallback_reason": status.fallback_reason,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// HudAnimator — background braille-spinner pulsing the HUD row-2 activity dot
// ─────────────────────────────────────────────────────────────────────────────

const BRAILLE_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Manages a background thread that pulses an animated activity indicator in
/// the HUD row 2 while a turn is running.  Drop (or call `.stop()`) to halt.
pub(crate) struct HudAnimator {
    stop_tx: Option<std::sync::mpsc::SyncSender<()>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl HudAnimator {
    /// Start the animator.  `label` is a short description shown next to the
    /// spinner (e.g. `"thinking"`).
    pub(crate) fn start(label: impl Into<String>) -> Self {
        use crossterm::{
            cursor,
            style::{self, Color, Stylize},
            terminal,
            QueueableCommand,
        };
        use std::io::{stdout, Write};

        let label = label.into();
        let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel::<()>(1);

        let handle = std::thread::spawn(move || {
            let mut frame_idx: usize = 0;
            loop {
                // Check for stop signal (non-blocking)
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let spinner_char = BRAILLE_FRAMES[frame_idx % BRAILLE_FRAMES.len()];
                let indicator = format!("  {} {} …", spinner_char, label);

                if let Ok((width, height)) = terminal::size() {
                    let w = width as usize;
                    let row2_y = height.saturating_sub(1);
                    let padded = if indicator.len() < w {
                        format!("{indicator:<width$}", width = w)
                    } else {
                        indicator.chars().take(w).collect::<String>()
                    };
                    let mut out = stdout();
                    let _ = out.queue(cursor::SavePosition);
                    let _ = out.queue(cursor::Hide);
                    let _ = out.queue(cursor::MoveTo(0, row2_y));
                    let _ = out.queue(style::PrintStyledContent(
                        padded
                            .with(Color::Rgb { r: 250, g: 204, b: 21 }) // amber
                            .on(Color::Rgb { r: 30, g: 41, b: 59 }),
                    ));
                    let _ = out.queue(cursor::Show);
                    let _ = out.queue(cursor::RestorePosition);
                    let _ = out.flush();
                }

                frame_idx = frame_idx.wrapping_add(1);
                std::thread::sleep(Duration::from_millis(80));
            }
        });

        Self {
            stop_tx: Some(stop_tx),
            join_handle: Some(handle),
        }
    }

    /// Stop the animator and join the thread.
    pub(crate) fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.try_send(());
        }
        if let Some(h) = self.join_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for HudAnimator {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.try_send(());
        }
        if let Some(h) = self.join_handle.take() {
            let _ = h.join();
        }
    }
}
