use crate::*;

#[allow(clippy::needless_pass_by_value)]
pub fn run_repl(
    model: String,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
    base_commit: Option<String>,
    reasoning_effort: Option<String>,
    allow_broad_cwd: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    enforce_broad_cwd_policy(allow_broad_cwd, CliOutputFormat::Text)?;
    run_stale_base_preflight(base_commit.as_deref());
    let resolved_model = resolve_repl_model(model);
    let mut cli = LiveCli::new(resolved_model, true, allowed_tools, permission_mode)?;
    cli.set_reasoning_effort(reasoning_effort);
    let mut editor =
        input::LineEditor::new("> ", cli.repl_completion_candidates().unwrap_or_default());
    println!("{}", cli.startup_banner());
    println!("{}", format_connected_line(&cli.model));

    loop {
        cli.refresh_hud()?;
        editor.set_completions(cli.repl_completion_candidates().unwrap_or_default());
        match editor.read_line()? {
            input::ReadOutcome::Submit(input) => {
                let trimmed = input.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                if matches!(trimmed.as_str(), "/exit" | "/quit") {
                    cli.persist_session()?;
                    cli.clear_hud()?;
                    break;
                }
                match SlashCommand::parse(&trimmed) {
                    Ok(Some(command)) => {
                        if cli.handle_repl_command(command)? {
                            cli.persist_session()?;
                        }
                        continue;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("{error}");
                        continue;
                    }
                }
                let cwd = std::env::current_dir().unwrap_or_default();
                if let Some(prompt) = try_resolve_bare_skill_prompt(&cwd, &trimmed) {
                    editor.push_history(input);
                    cli.record_prompt_history(&trimmed);
                    cli.run_turn(&prompt)?;
                    continue;
                }
                editor.push_history(input);
                cli.record_prompt_history(&trimmed);
                cli.run_turn(&trimmed)?;
            }
            input::ReadOutcome::Cancel => {}
            input::ReadOutcome::Exit => {
                cli.persist_session()?;
                cli.clear_hud()?;
                break;
            }
        }
    }

    Ok(())
}

// --- Extracted from main.rs ---

#[allow(clippy::too_many_lines)]

/// Detect if the current working directory is "broad" (home directory or
/// filesystem root). Returns the cwd path if broad, None otherwise.
pub fn detect_broad_cwd() -> Option<PathBuf> {
    let Ok(cwd) = env::current_dir() else {
        return None;
    };
    let is_home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .is_some_and(|h| Path::new(&h) == cwd);
    let is_root = cwd.parent().is_none();
    if is_home || is_root {
        Some(cwd)
    } else {
        None
    }
}

/// Enforce the broad-CWD policy: when running from home or root, either
/// require the --allow-broad-cwd flag, or prompt for confirmation (interactive),
/// or exit with an error (non-interactive).
pub fn enforce_broad_cwd_policy(
    allow_broad_cwd: bool,
    output_format: CliOutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    if allow_broad_cwd {
        return Ok(());
    }
    let Some(cwd) = detect_broad_cwd() else {
        return Ok(());
    };

    let is_interactive = io::stdin().is_terminal();

    if is_interactive {
        // Interactive mode: print warning and ask for confirmation
        eprintln!(
            "Warning: claw is running from a very broad directory ({}).\n\
             The agent can read and search everything under this path.\n\
             Consider running from inside your project: cd /path/to/project && claw",
            cwd.display()
        );
        eprint!("Continue anyway? [y/N]: ");
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            eprintln!("Aborted.");
            std::process::exit(0);
        }
        Ok(())
    } else {
        // Non-interactive mode: exit with error (JSON or text)
        let message = format!(
            "claw is running from a very broad directory ({}). \
             The agent can read and search everything under this path. \
             Use --allow-broad-cwd to proceed anyway, \
             or run from inside your project: cd /path/to/project && claw",
            cwd.display()
        );
        match output_format {
            CliOutputFormat::Json => {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "type": "error",
                        "error": message,
                    })
                );
            }
            CliOutputFormat::Text => {
                eprintln!("error: {message}");
            }
        }
        std::process::exit(1);
    }
}

pub fn run_stale_base_preflight(flag_value: Option<&str>) {
    let Ok(cwd) = env::current_dir() else {
        return;
    };
    let source = resolve_expected_base(flag_value, &cwd);
    let state = check_base_commit(&cwd, source.as_ref());
    if let Some(warning) = format_stale_base_warning(&state) {
        eprintln!("{warning}");
    }
}

#[allow(clippy::needless_pass_by_value)]


pub struct LiveCli {
    pub model: String,
    pub allowed_tools: Option<AllowedToolSet>,
    pub permission_mode: PermissionMode,
    pub system_prompt: Vec<String>,
    pub runtime: BuiltRuntime,
    pub session: SessionHandle,
    pub prompt_history: Vec<PromptHistoryEntry>,
    pub live_turn_usage: Arc<Mutex<TokenUsage>>,
    pub theme: String,
}

impl LiveCli {
    pub fn refresh_hud(&self) -> io::Result<()> {
        let context = tui::status_bar::status_context(Some(self.session.path.as_path()))
            .unwrap_or_else(|_| tui::status_bar::StatusContext {
                cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                session_path: Some(self.session.path.clone()),
                loaded_config_files: 0,
                discovered_config_files: 0,
                memory_file_count: 0,
                project_root: None,
                git_branch: None,
                git_summary: tui::status_bar::GitWorkspaceSummary::default(),
                sandbox_status: runtime::SandboxStatus::default(),
            });
        let live_usage = *self.live_turn_usage.lock().unwrap();
        let mut cumulative = self.runtime.cumulative_usage();
        cumulative.input_tokens += live_usage.input_tokens;
        cumulative.output_tokens += live_usage.output_tokens;
        cumulative.cache_creation_input_tokens += live_usage.cache_creation_input_tokens;
        cumulative.cache_read_input_tokens += live_usage.cache_read_input_tokens;

        let usage = tui::status_bar::StatusUsage {
            message_count: self.runtime.messages().len(),
            turns: self.runtime.turns(),
            latest: live_usage,
            cumulative,
            estimated_tokens: self.runtime.estimated_tokens(),
        };
        tui::status_bar::draw_hud(&self.model, self.permission_mode.as_str(), usage, &context)?;
        Ok(())
    }

    pub fn clear_hud(&self) -> io::Result<()> {
        tui::status_bar::clear_hud()
    }
}

#[derive(Debug, Clone)]
pub struct PromptHistoryEntry {
    pub timestamp_ms: u64,
    pub text: String,
}

pub struct RuntimePluginState {
    pub feature_config: runtime::RuntimeFeatureConfig,
    pub tool_registry: GlobalToolRegistry,
    pub plugin_registry: PluginRegistry,
    pub mcp_state: Option<Arc<Mutex<RuntimeMcpState>>>,
}

pub struct RuntimeMcpState {
    pub runtime: tokio::runtime::Runtime,
    pub manager: McpServerManager,
    pub pending_servers: Vec<String>,
    pub degraded_report: Option<runtime::McpDegradedReport>,
}

pub struct BuiltRuntime {
    pub runtime: Option<ConversationRuntime<AnthropicRuntimeClient, CliToolExecutor>>,
    pub plugin_registry: PluginRegistry,
    pub plugins_active: bool,
    pub mcp_state: Option<Arc<Mutex<RuntimeMcpState>>>,
    pub mcp_active: bool,
    pub theme: String,
}

impl BuiltRuntime {
    pub fn new(
        runtime: ConversationRuntime<AnthropicRuntimeClient, CliToolExecutor>,
        plugin_registry: PluginRegistry,
        mcp_state: Option<Arc<Mutex<RuntimeMcpState>>>,
        theme: String,
    ) -> Self {
        Self {
            runtime: Some(runtime),
            plugin_registry,
            plugins_active: true,
            mcp_state,
            mcp_active: true,
            theme,
        }
    }

    pub fn with_hook_abort_signal(mut self, hook_abort_signal: runtime::HookAbortSignal) -> Self {
        let runtime = self
            .runtime
            .take()
            .expect("runtime should exist before installing hook abort signal");
        self.runtime = Some(runtime.with_hook_abort_signal(hook_abort_signal));
        self
    }

    pub fn shutdown_plugins(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.plugins_active {
            self.plugin_registry.shutdown()?;
            self.plugins_active = false;
        }
        Ok(())
    }

    pub fn shutdown_mcp(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.mcp_active {
            if let Some(mcp_state) = &self.mcp_state {
                mcp_state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .shutdown()?;
            }
            self.mcp_active = false;
        }
        Ok(())
    }

    pub fn messages(&self) -> Vec<runtime::ConversationMessage> {
        self.deref().session().messages.clone()
    }

    pub fn turns(&self) -> u32 {
        self.deref().usage().turns()
    }

    pub fn latest_usage(&self) -> runtime::TokenUsage {
        self.deref().usage().current_turn_usage()
    }

    pub fn cumulative_usage(&self) -> runtime::TokenUsage {
        self.deref().usage().cumulative_usage()
    }

    pub fn estimated_tokens(&self) -> usize {
        self.deref().estimated_tokens()
    }
}

impl Deref for BuiltRuntime {
    type Target = ConversationRuntime<AnthropicRuntimeClient, CliToolExecutor>;

    fn deref(&self) -> &Self::Target {
        self.runtime
            .as_ref()
            .expect("runtime should exist while built runtime is alive")
    }
}

impl DerefMut for BuiltRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.runtime
            .as_mut()
            .expect("runtime should exist while built runtime is alive")
    }
}

impl Drop for BuiltRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown_mcp();
        let _ = self.shutdown_plugins();
    }
}

#[derive(Debug, Deserialize)]
pub struct ToolSearchRequest {
    pub query: String,
    pub max_results: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct McpToolRequest {
    #[serde(rename = "qualifiedName")]
    pub qualified_name: Option<String>,
    pub tool: Option<String>,
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ListMcpResourcesRequest {
    pub server: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadMcpResourceRequest {
    pub server: String,
    pub uri: String,
}

impl RuntimeMcpState {
    pub fn new(
        runtime_config: &runtime::RuntimeConfig,
    ) -> Result<Option<(Self, runtime::McpToolDiscoveryReport)>, Box<dyn std::error::Error>> {
        let mut manager = McpServerManager::from_runtime_config(runtime_config);
        if manager.server_names().is_empty() && manager.unsupported_servers().is_empty() {
            return Ok(None);
        }

        let runtime = tokio::runtime::Runtime::new()?;
        let discovery = runtime.block_on(manager.discover_tools_best_effort());
        let pending_servers = discovery
            .failed_servers
            .iter()
            .map(|failure| failure.server_name.clone())
            .chain(
                discovery
                    .unsupported_servers
                    .iter()
                    .map(|server| server.server_name.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let available_tools = discovery
            .tools
            .iter()
            .map(|tool| tool.qualified_name.clone())
            .collect::<Vec<_>>();
        let failed_server_names = pending_servers.iter().cloned().collect::<BTreeSet<_>>();
        let working_servers = manager
            .server_names()
            .into_iter()
            .filter(|server_name| !failed_server_names.contains(server_name))
            .collect::<Vec<_>>();
        let failed_servers =
            discovery
                .failed_servers
                .iter()
                .map(|failure| runtime::McpFailedServer {
                    server_name: failure.server_name.clone(),
                    phase: runtime::McpLifecyclePhase::ToolDiscovery,
                    error: runtime::McpErrorSurface::new(
                        runtime::McpLifecyclePhase::ToolDiscovery,
                        Some(failure.server_name.clone()),
                        failure.error.clone(),
                        std::collections::BTreeMap::new(),
                        true,
                    ),
                })
                .chain(discovery.unsupported_servers.iter().map(|server| {
                    runtime::McpFailedServer {
                        server_name: server.server_name.clone(),
                        phase: runtime::McpLifecyclePhase::ServerRegistration,
                        error: runtime::McpErrorSurface::new(
                            runtime::McpLifecyclePhase::ServerRegistration,
                            Some(server.server_name.clone()),
                            server.reason.clone(),
                            std::collections::BTreeMap::from([(
                                "transport".to_string(),
                                format!("{:?}", server.transport).to_ascii_lowercase(),
                            )]),
                            false,
                        ),
                    }
                }))
                .collect::<Vec<_>>();
        let degraded_report = (!failed_servers.is_empty()).then(|| {
            runtime::McpDegradedReport::new(
                working_servers,
                failed_servers,
                available_tools.clone(),
                available_tools,
            )
        });

        Ok(Some((
            Self {
                runtime,
                manager,
                pending_servers,
                degraded_report,
            },
            discovery,
        )))
    }

    pub fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.block_on(self.manager.shutdown())?;
        Ok(())
    }

    pub fn pending_servers(&self) -> Option<Vec<String>> {
        (!self.pending_servers.is_empty()).then(|| self.pending_servers.clone())
    }

    pub fn degraded_report(&self) -> Option<runtime::McpDegradedReport> {
        self.degraded_report.clone()
    }

    pub fn server_names(&self) -> Vec<String> {
        self.manager.server_names()
    }

    pub fn call_tool(
        &mut self,
        qualified_tool_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<String, ToolError> {
        let response = self
            .runtime
            .block_on(self.manager.call_tool(qualified_tool_name, arguments))
            .map_err(|error| ToolError::new(error.to_string()))?;
        if let Some(error) = response.error {
            return Err(ToolError::new(format!(
                "MCP tool `{qualified_tool_name}` returned JSON-RPC error: {} ({})",
                error.message, error.code
            )));
        }

        let result = response.result.ok_or_else(|| {
            ToolError::new(format!(
                "MCP tool `{qualified_tool_name}` returned no result payload"
            ))
        })?;
        serde_json::to_string_pretty(&result).map_err(|error| ToolError::new(error.to_string()))
    }

    pub fn list_resources_for_server(&mut self, server_name: &str) -> Result<String, ToolError> {
        let result = self
            .runtime
            .block_on(self.manager.list_resources(server_name))
            .map_err(|error| ToolError::new(error.to_string()))?;
        serde_json::to_string_pretty(&json!({
            "server": server_name,
            "resources": result.resources,
        }))
        .map_err(|error| ToolError::new(error.to_string()))
    }

    pub fn list_resources_for_all_servers(&mut self) -> Result<String, ToolError> {
        let mut resources = Vec::new();
        let mut failures = Vec::new();

        for server_name in self.server_names() {
            match self
                .runtime
                .block_on(self.manager.list_resources(&server_name))
            {
                Ok(result) => resources.push(json!({
                    "server": server_name,
                    "resources": result.resources,
                })),
                Err(error) => failures.push(json!({
                    "server": server_name,
                    "error": error.to_string(),
                })),
            }
        }

        if resources.is_empty() && !failures.is_empty() {
            let message = failures
                .iter()
                .filter_map(|failure| failure.get("error").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ToolError::new(message));
        }

        serde_json::to_string_pretty(&json!({
            "resources": resources,
            "failures": failures,
        }))
        .map_err(|error| ToolError::new(error.to_string()))
    }

    pub fn read_resource(&mut self, server_name: &str, uri: &str) -> Result<String, ToolError> {
        let result = self
            .runtime
            .block_on(self.manager.read_resource(server_name, uri))
            .map_err(|error| ToolError::new(error.to_string()))?;
        serde_json::to_string_pretty(&json!({
            "server": server_name,
            "contents": result.contents,
        }))
        .map_err(|error| ToolError::new(error.to_string()))
    }
}

pub fn build_runtime_mcp_state(
    runtime_config: &runtime::RuntimeConfig,
) -> Result<RuntimePluginStateBuildOutput, Box<dyn std::error::Error>> {
    let Some((mcp_state, discovery)) = RuntimeMcpState::new(runtime_config)? else {
        return Ok((None, Vec::new()));
    };

    let mut runtime_tools = discovery
        .tools
        .iter()
        .map(mcp_runtime_tool_definition)
        .collect::<Vec<_>>();
    if !mcp_state.server_names().is_empty() {
        runtime_tools.extend(mcp_wrapper_tool_definitions());
    }

    Ok((Some(Arc::new(Mutex::new(mcp_state))), runtime_tools))
}

pub fn mcp_runtime_tool_definition(tool: &runtime::ManagedMcpTool) -> RuntimeToolDefinition {
    RuntimeToolDefinition {
        name: tool.qualified_name.clone(),
        description: Some(
            tool.tool
                .description
                .clone()
                .unwrap_or_else(|| format!("Invoke MCP tool `{}`.", tool.qualified_name)),
        ),
        input_schema: tool
            .tool
            .input_schema
            .clone()
            .unwrap_or_else(|| json!({ "type": "object", "additionalProperties": true })),
        required_permission: permission_mode_for_mcp_tool(&tool.tool),
    }
}

pub fn mcp_wrapper_tool_definitions() -> Vec<RuntimeToolDefinition> {
    vec![
        RuntimeToolDefinition {
            name: "MCPTool".to_string(),
            description: Some(
                "Call a configured MCP tool by its qualified name and JSON arguments.".to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "qualifiedName": { "type": "string" },
                    "arguments": {}
                },
                "required": ["qualifiedName"],
                "additionalProperties": false
            }),
            required_permission: PermissionMode::DangerFullAccess,
        },
        RuntimeToolDefinition {
            name: "ListMcpResourcesTool".to_string(),
            description: Some(
                "List MCP resources from one configured server or from every connected server."
                    .to_string(),
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": { "type": "string" }
                },
                "additionalProperties": false
            }),
            required_permission: PermissionMode::ReadOnly,
        },
        RuntimeToolDefinition {
            name: "ReadMcpResourceTool".to_string(),
            description: Some("Read a specific MCP resource from a configured server.".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "server": { "type": "string" },
                    "uri": { "type": "string" }
                },
                "required": ["server", "uri"],
                "additionalProperties": false
            }),
            required_permission: PermissionMode::ReadOnly,
        },
    ]
}

pub fn permission_mode_for_mcp_tool(tool: &McpTool) -> PermissionMode {
    let read_only = mcp_annotation_flag(tool, "readOnlyHint");
    let destructive = mcp_annotation_flag(tool, "destructiveHint");
    let open_world = mcp_annotation_flag(tool, "openWorldHint");

    if read_only && !destructive && !open_world {
        PermissionMode::ReadOnly
    } else if destructive || open_world {
        PermissionMode::DangerFullAccess
    } else {
        PermissionMode::WorkspaceWrite
    }
}

pub fn mcp_annotation_flag(tool: &McpTool, key: &str) -> bool {
    tool.annotations
        .as_ref()
        .and_then(|annotations| annotations.get(key))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub struct HookAbortMonitor {
    pub stop_tx: Option<Sender<()>>,
    pub join_handle: Option<JoinHandle<()>>,
}

impl HookAbortMonitor {
    pub fn spawn(abort_signal: runtime::HookAbortSignal) -> Self {
        Self::spawn_with_waiter(abort_signal, move |stop_rx, abort_signal| {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };

            runtime.block_on(async move {
                let wait_for_stop = tokio::task::spawn_blocking(move || {
                    let _ = stop_rx.recv();
                });

                tokio::select! {
                    result = tokio::signal::ctrl_c() => {
                        if result.is_ok() {
                            abort_signal.abort();
                        }
                    }
                    _ = wait_for_stop => {}
                }
            });
        })
    }

    pub fn spawn_with_waiter<F>(abort_signal: runtime::HookAbortSignal, wait_for_interrupt: F) -> Self
    where
        F: FnOnce(Receiver<()>, runtime::HookAbortSignal) + Send + 'static,
    {
        let (stop_tx, stop_rx) = mpsc::channel();
        let join_handle = thread::spawn(move || wait_for_interrupt(stop_rx, abort_signal));

        Self {
            stop_tx: Some(stop_tx),
            join_handle: Some(join_handle),
        }
    }

    pub fn stop(mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

impl LiveCli {
    pub fn new(
        model: String,
        enable_tools: bool,
        allowed_tools: Option<AllowedToolSet>,
        permission_mode: PermissionMode,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let system_prompt = build_system_prompt()?;
        let session_state = new_cli_session()?;
        let session = create_managed_session_handle(&session_state.session_id)?;
        let live_turn_usage = Arc::new(Mutex::new(TokenUsage::default()));
        let runtime = build_runtime(
            session_state.with_persistence_path(session.path.clone()),
            &session.id,
            model.clone(),
            system_prompt.clone(),
            enable_tools,
            true,
            allowed_tools.clone(),
            permission_mode,
            None,
            None,
        )?;
        let theme = runtime.theme.clone();
        let cli = Self {
            model,
            allowed_tools,
            permission_mode,
            system_prompt,
            runtime,
            session,
            prompt_history: Vec::new(),
            live_turn_usage,
            theme,
        };
        cli.persist_session()?;
        Ok(cli)
    }

    pub fn set_reasoning_effort(&mut self, effort: Option<String>) {
        if let Some(rt) = self.runtime.runtime.as_mut() {
            rt.api_client_mut().set_reasoning_effort(effort);
        }
    }

    pub fn startup_banner(&self) -> String {
        let cwd = env::current_dir().map_or_else(
            |_| "<unknown>".to_string(),
            |path| path.display().to_string(),
        );
        let status = status_context(None).ok();
        let git_branch = status
            .as_ref()
            .and_then(|context| context.git_branch.as_deref())
            .unwrap_or("unknown");
        let workspace = status.as_ref().map_or_else(
            || "unknown".to_string(),
            |context| context.git_summary.headline(),
        );
        let session_path = self.session.path.strip_prefix(Path::new(&cwd)).map_or_else(
            |_| self.session.path.display().to_string(),
            |path| path.display().to_string(),
        );
        format!(
            "\x1b[38;5;196m\
 ██████╗██╗      █████╗ ██╗    ██╗\n\
██╔════╝██║     ██╔══██╗██║    ██║\n\
██║     ██║     ███████║██║ █╗ ██║\n\
██║     ██║     ██╔══██║██║███╗██║\n\
╚██████╗███████╗██║  ██║╚███╔███╔╝\n\
 ╚═════╝╚══════╝╚═╝  ╚═╝ ╚══╝╚══╝\x1b[0m \x1b[38;5;208mCode\x1b[0m 🦞\n\n\
  \x1b[2mModel\x1b[0m            {}\n\
  \x1b[2mPermissions\x1b[0m      {}\n\
  \x1b[2mBranch\x1b[0m           {}\n\
  \x1b[2mWorkspace\x1b[0m        {}\n\
  \x1b[2mDirectory\x1b[0m        {}\n\
  \x1b[2mSession\x1b[0m          {}\n\
  \x1b[2mAuto-save\x1b[0m        {}\n\n\
  Type \x1b[1m/help\x1b[0m for commands · \x1b[1m/status\x1b[0m for live context · \x1b[2m/resume latest\x1b[0m jumps back to the newest session · \x1b[1m/diff\x1b[0m then \x1b[1m/commit\x1b[0m to ship · \x1b[2mTab\x1b[0m for workflow completions · \x1b[2mShift+Enter\x1b[0m for newline",
            self.model,
            self.permission_mode.as_str(),
            git_branch,
            workspace,
            cwd,
            self.session.id,
            session_path,
        )
    }

    pub fn repl_completion_candidates(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Ok(slash_command_completion_candidates_with_sessions(
            &self.model,
            Some(&self.session.id),
            list_managed_sessions()?
                .into_iter()
                .map(|session| session.id)
                .collect(),
        ))
    }

    pub fn prepare_turn_runtime(
        &self,
        emit_output: bool,
    ) -> Result<(BuiltRuntime, HookAbortMonitor), Box<dyn std::error::Error>> {
        let hook_abort_signal = runtime::HookAbortSignal::new();
        if let Ok(mut state) = self.live_turn_usage.lock() {
            *state = TokenUsage::default();
        }
        let live_turn_usage = self.live_turn_usage.clone();
        let model = self.model.clone();
        let permission_mode = self.permission_mode.as_str().to_string();
        let session_path = self.session.path.clone();
        let base_cumulative = self.runtime.cumulative_usage();
        let message_count = self.runtime.messages().len();
        let turns = self.runtime.turns();
        let estimated_tokens = self.runtime.estimated_tokens();

        let callback = Arc::new(move |usage: TokenUsage| {
            if let Ok(mut state) = live_turn_usage.lock() {
                *state = usage;
            }
            
            // Trigger HUD refresh
            let context = tui::status_bar::status_context(Some(&session_path)).unwrap_or_else(|_| tui::status_bar::StatusContext::default());
            let mut cumulative = base_cumulative;
            cumulative.input_tokens += usage.input_tokens;
            cumulative.output_tokens += usage.output_tokens;
            cumulative.cache_creation_input_tokens += usage.cache_creation_input_tokens;
            cumulative.cache_read_input_tokens += usage.cache_read_input_tokens;

            let status_usage = tui::status_bar::StatusUsage {
                message_count,
                turns,
                latest: usage,
                cumulative,
                estimated_tokens,
            };
            let _ = tui::status_bar::draw_hud(&model, &permission_mode, status_usage, &context).ok();
        });

        let runtime = build_runtime(
            self.runtime.session().clone(),
            &self.session.id,
            self.model.clone(),
            self.system_prompt.clone(),
            true,
            emit_output,
            self.allowed_tools.clone(),
            self.permission_mode,
            None,
            Some(callback),
        )?
        .with_hook_abort_signal(hook_abort_signal.clone());
        let hook_abort_monitor = HookAbortMonitor::spawn(hook_abort_signal);

        Ok((runtime, hook_abort_monitor))
    }

    pub fn replace_runtime(&mut self, runtime: BuiltRuntime) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.shutdown_plugins()?;
        self.runtime = runtime;
        Ok(())
    }

    pub fn run_turn(&mut self, input: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (mut runtime, hook_abort_monitor) = self.prepare_turn_runtime(true)?;
        let mut spinner = Spinner::new();
        let mut stdout = io::stdout();
        let started = Instant::now();
        // Start the HUD row-2 braille animator before the turn.
        let hud_anim = tui::status_bar::HudAnimator::start("thinking");
        spinner.tick(
            "🦀 Thinking...",
            TerminalRenderer::new().with_vaultwares_theme(&self.theme).color_theme(),
            &mut stdout,
        )?;
        let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
        let result = runtime.run_turn(input, Some(&mut permission_prompter));
        hook_abort_monitor.stop();
        // Stop the HUD animator before printing the footer so the lines are clean.
        hud_anim.stop();
        match result {
            Ok(summary) => {
                self.replace_runtime(runtime)?;
                spinner.finish(
                    "✨ Done",
                    TerminalRenderer::new().with_vaultwares_theme(&self.theme).color_theme(),
                    &mut stdout,
                )?;
                println!(
                    "{}",
                    format_turn_footer(
                        &self.model,
                        self.permission_mode.as_str(),
                        &self.session.id,
                        summary.usage,
                        started.elapsed(),
                    )
                );
                if let Some(timeline) = format_tool_timeline(&summary, started.elapsed()) {
                    println!("{timeline}");
                }
                println!();
                if let Some(event) = summary.auto_compaction {
                    println!(
                        "{}",
                        format_auto_compaction_notice(event.removed_message_count)
                    );
                }
                self.persist_session()?;
                Ok(())
            }
            Err(error) => {
                runtime.shutdown_plugins()?;
                let failure_label = format!(
                    "Request failed after {}",
                    format_duration_compact(started.elapsed())
                );
                spinner.fail(
                    &failure_label,
                    TerminalRenderer::new().with_vaultwares_theme(&self.theme).color_theme(),
                    &mut stdout,
                )?;
                Err(Box::new(error))
            }
        }
    }

    pub fn run_turn_with_output(
        &mut self,
        input: &str,
        output_format: CliOutputFormat,
        compact: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match output_format {
            CliOutputFormat::Text if compact => self.run_prompt_compact(input),
            CliOutputFormat::Text => self.run_turn(input),
            CliOutputFormat::Json => self.run_prompt_json(input),
        }
    }

    pub fn run_prompt_compact(&mut self, input: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (mut runtime, hook_abort_monitor) = self.prepare_turn_runtime(false)?;
        let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
        let result = runtime.run_turn(input, Some(&mut permission_prompter));
        hook_abort_monitor.stop();
        let summary = result?;
        self.replace_runtime(runtime)?;
        self.persist_session()?;
        let final_text = final_assistant_text(&summary);
        println!("{final_text}");
        Ok(())
    }

    pub fn run_prompt_json(&mut self, input: &str) -> Result<(), Box<dyn std::error::Error>> {
        let (mut runtime, hook_abort_monitor) = self.prepare_turn_runtime(false)?;
        let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
        let result = runtime.run_turn(input, Some(&mut permission_prompter));
        hook_abort_monitor.stop();
        let summary = result?;
        self.replace_runtime(runtime)?;
        self.persist_session()?;
        println!(
            "{}",
            json!({
                "message": final_assistant_text(&summary),
                "model": self.model,
                "iterations": summary.iterations,
                "auto_compaction": summary.auto_compaction.map(|event| json!({
                    "removed_messages": event.removed_message_count,
                    "notice": format_auto_compaction_notice(event.removed_message_count),
                })),
                "tool_uses": collect_tool_uses(&summary),
                "tool_results": collect_tool_results(&summary),
                "prompt_cache_events": collect_prompt_cache_events(&summary),
                "usage": {
                    "input_tokens": summary.usage.input_tokens,
                    "output_tokens": summary.usage.output_tokens,
                    "cache_creation_input_tokens": summary.usage.cache_creation_input_tokens,
                    "cache_read_input_tokens": summary.usage.cache_read_input_tokens,
                },
                "estimated_cost": format_usd(
                    summary.usage.estimate_cost_usd_with_pricing(
                        pricing_for_model(&self.model)
                            .unwrap_or_else(runtime::ModelPricing::default_sonnet_tier)
                    ).total_cost_usd()
                )
            })
        );
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub fn handle_repl_command(
        &mut self,
        command: SlashCommand,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(match command {
            SlashCommand::Help => {
                println!("{}", render_repl_help());
                false
            }
            SlashCommand::Status => {
                self.print_status();
                false
            }
            SlashCommand::Bughunter { scope } => {
                self.run_bughunter(scope.as_deref())?;
                false
            }
            SlashCommand::Commit => {
                self.run_commit(None)?;
                false
            }
            SlashCommand::Pr { context } => {
                self.run_pr(context.as_deref())?;
                false
            }
            SlashCommand::Issue { context } => {
                self.run_issue(context.as_deref())?;
                false
            }
            SlashCommand::Ultraplan { task } => {
                self.run_ultraplan(task.as_deref())?;
                false
            }
            SlashCommand::Teleport { target } => {
                Self::run_teleport(target.as_deref())?;
                false
            }
            SlashCommand::DebugToolCall => {
                self.run_debug_tool_call(None)?;
                false
            }
            SlashCommand::Sandbox => {
                Self::print_sandbox_status();
                false
            }
            SlashCommand::Compact => {
                self.compact()?;
                false
            }
            SlashCommand::Model { model } => self.set_model(model)?,
            SlashCommand::Permissions { mode } => self.set_permissions(mode)?,
            SlashCommand::Clear { confirm } => self.clear_session(confirm)?,
            SlashCommand::Cost => {
                self.print_cost();
                false
            }
            SlashCommand::Resume { session_path } => self.resume_session(session_path)?,
            SlashCommand::Config { section } => {
                Self::print_config(section.as_deref())?;
                false
            }
            SlashCommand::Mcp { action, target } => {
                let args = match (action.as_deref(), target.as_deref()) {
                    (None, None) => None,
                    (Some(action), None) => Some(action.to_string()),
                    (Some(action), Some(target)) => Some(format!("{action} {target}")),
                    (None, Some(target)) => Some(target.to_string()),
                };
                Self::print_mcp(args.as_deref(), CliOutputFormat::Text)?;
                false
            }
            SlashCommand::Memory => {
                Self::print_memory()?;
                false
            }
            SlashCommand::Init => {
                run_init(CliOutputFormat::Text)?;
                false
            }
            SlashCommand::Diff => {
                Self::print_diff()?;
                false
            }
            SlashCommand::Version => {
                Self::print_version(CliOutputFormat::Text);
                false
            }
            SlashCommand::Export { path } => {
                self.export_session(path.as_deref())?;
                false
            }
            SlashCommand::Session { action, target } => {
                self.handle_session_command(action.as_deref(), target.as_deref())?
            }
            SlashCommand::Plugins { action, target } => {
                self.handle_plugins_command(action.as_deref(), target.as_deref())?
            }
            SlashCommand::Agents { args } => {
                Self::print_agents(args.as_deref(), CliOutputFormat::Text)?;
                false
            }
            SlashCommand::Skills { args } => {
                match classify_skills_slash_command(args.as_deref()) {
                    SkillSlashDispatch::Invoke(prompt) => self.run_turn(&prompt)?,
                    SkillSlashDispatch::Local => {
                        Self::print_skills(args.as_deref(), CliOutputFormat::Text)?;
                    }
                }
                false
            }
            SlashCommand::Doctor => {
                println!("{}", render_doctor_report()?.render());
                false
            }
            SlashCommand::History { count } => {
                self.print_prompt_history(count.as_deref());
                false
            }
            SlashCommand::Search { query } => {
                self.print_conversation_search(&query)?;
                false
            }
            SlashCommand::Undo => {
                self.undo_last_file_change()?;
                false
            }
            SlashCommand::Stats => {
                let usage = UsageTracker::from_session(self.runtime.session()).cumulative_usage();
                println!("{}", format_cost_report(usage));
                false
            }
            SlashCommand::Login
            | SlashCommand::Logout
            | SlashCommand::Vim
            | SlashCommand::Upgrade
            | SlashCommand::Share
            | SlashCommand::Feedback
            | SlashCommand::Files
            | SlashCommand::Fast
            | SlashCommand::Exit
            | SlashCommand::Summary
            | SlashCommand::Desktop
            | SlashCommand::Brief
            | SlashCommand::Advisor
            | SlashCommand::Stickers
            | SlashCommand::Insights
            | SlashCommand::Thinkback
            | SlashCommand::ReleaseNotes
            | SlashCommand::SecurityReview
            | SlashCommand::Keybindings
            | SlashCommand::PrivacySettings
            | SlashCommand::Plan { .. }
            | SlashCommand::Review { .. }
            | SlashCommand::Tasks { .. }
            | SlashCommand::Theme { .. }
            | SlashCommand::Voice { .. }
            | SlashCommand::Usage { .. }
            | SlashCommand::Rename { .. }
            | SlashCommand::Copy { .. }
            | SlashCommand::Hooks { .. }
            | SlashCommand::Context { .. }
            | SlashCommand::Color { .. }
            | SlashCommand::Effort { .. }
            | SlashCommand::Branch { .. }
            | SlashCommand::Rewind { .. }
            | SlashCommand::Ide { .. }
            | SlashCommand::Tag { .. }
            | SlashCommand::OutputStyle { .. }
            | SlashCommand::AddDir { .. } => {
                let cmd_name = command.slash_name();
                eprintln!("{cmd_name} is not yet implemented in this build.");
                false
            }
            SlashCommand::Unknown(name) => {
                eprintln!("{}", format_unknown_slash_command(&name));
                false
            }
        })
    }

    pub fn persist_session(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.session().save_to_path(&self.session.path)?;
        Ok(())
    }

    pub fn print_status(&self) {
        let cumulative = self.runtime.usage().cumulative_usage();
        let latest = self.runtime.usage().current_turn_usage();
        let report = format_status_report(
            &self.model,
            StatusUsage {
                message_count: self.runtime.session().messages.len(),
                turns: self.runtime.usage().turns(),
                latest,
                cumulative,
                estimated_tokens: self.runtime.estimated_tokens(),
            },
            self.permission_mode.as_str(),
            &status_context(Some(&self.session.path)).expect("status context should load"),
        );
        if let Err(error) = print_report_or_page(&report) {
            eprintln!("warning: failed to render status report: {error}");
        }
    }

    pub fn record_prompt_history(&mut self, prompt: &str) {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map_or(self.runtime.session().updated_at_ms, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            });
        let entry = PromptHistoryEntry {
            timestamp_ms,
            text: prompt.to_string(),
        };
        self.prompt_history.push(entry);
        if let Err(error) = self.runtime.session_mut().push_prompt_entry(prompt) {
            eprintln!("warning: failed to persist prompt history: {error}");
        }
    }

    pub fn print_prompt_history(&self, count: Option<&str>) {
        let limit = match parse_history_count(count) {
            Ok(limit) => limit,
            Err(message) => {
                eprintln!("{message}");
                return;
            }
        };
        let session_entries = &self.runtime.session().prompt_history;
        let entries = if session_entries.is_empty() {
            if self.prompt_history.is_empty() {
                collect_session_prompt_history(self.runtime.session())
            } else {
                self.prompt_history
                    .iter()
                    .map(|entry| PromptHistoryEntry {
                        timestamp_ms: entry.timestamp_ms,
                        text: entry.text.clone(),
                    })
                    .collect()
            }
        } else {
            session_entries
                .iter()
                .map(|entry| PromptHistoryEntry {
                    timestamp_ms: entry.timestamp_ms,
                    text: entry.text.clone(),
                })
                .collect()
        };
        let report = render_prompt_history_report(&entries, limit);
        if let Err(error) = print_report_or_page(&report) {
            eprintln!("warning: failed to render prompt history: {error}");
        }
    }

    pub fn print_conversation_search(&self, query: &str) -> Result<(), Box<dyn std::error::Error>> {
        let report = render_conversation_search_report(self.runtime.session(), query);
        print_report_or_page(&report)?;
        Ok(())
    }

    pub fn undo_last_file_change(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let report = match find_last_file_undo_action(self.runtime.session()) {
            Some(action) => apply_file_undo_action(&action, self.runtime.session())?,
            None => {
                "Undo\n  Result           no reversible file edits found\n  Detail           /undo supports the latest write_file or edit_file result with originalFile data"
                    .to_string()
            }
        };
        print_report_or_page(&report)?;
        Ok(())
    }

    pub fn print_sandbox_status() {
        let cwd = env::current_dir().expect("current dir");
        let loader = ConfigLoader::default_for(&cwd);
        let runtime_config = loader
            .load()
            .unwrap_or_else(|_| runtime::RuntimeConfig::empty());
        println!(
            "{}",
            format_sandbox_report(&resolve_sandbox_status(runtime_config.sandbox(), &cwd))
        );
    }

    pub fn set_model(&mut self, model: Option<String>) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(model) = model else {
            println!(
                "{}",
                format_model_report(
                    &self.model,
                    self.runtime.session().messages.len(),
                    self.runtime.usage().turns(),
                )
            );
            return Ok(false);
        };

        let model = resolve_model_alias_with_config(&model);

        if model == self.model {
            println!(
                "{}",
                format_model_report(
                    &self.model,
                    self.runtime.session().messages.len(),
                    self.runtime.usage().turns(),
                )
            );
            return Ok(false);
        }

        let previous = self.model.clone();
        let session = self.runtime.session().clone();
        let message_count = session.messages.len();
        let runtime = build_runtime(
            session,
            &self.session.id,
            model.clone(),
            self.system_prompt.clone(),
            true,
            true,
            self.allowed_tools.clone(),
            self.permission_mode,
            None,
            None,
        )?;
        self.replace_runtime(runtime)?;
        self.model.clone_from(&model);
        println!(
            "{}",
            format_model_switch_report(&previous, &model, message_count)
        );
        Ok(true)
    }

    pub fn set_permissions(
        &mut self,
        mode: Option<String>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(mode) = mode else {
            println!(
                "{}",
                format_permissions_report(self.permission_mode.as_str())
            );
            return Ok(false);
        };

        let normalized = normalize_permission_mode(&mode).ok_or_else(|| {
            format!(
                "unsupported permission mode '{mode}'. Use read-only, workspace-write, or danger-full-access."
            )
        })?;

        if normalized == self.permission_mode.as_str() {
            println!("{}", format_permissions_report(normalized));
            return Ok(false);
        }

        let previous = self.permission_mode.as_str().to_string();
        let session = self.runtime.session().clone();
        self.permission_mode = permission_mode_from_label(normalized);
        let runtime = build_runtime(
            session,
            &self.session.id,
            self.model.clone(),
            self.system_prompt.clone(),
            true,
            true,
            self.allowed_tools.clone(),
            self.permission_mode,
            None,
            None,
        )?;
        self.replace_runtime(runtime)?;
        println!(
            "{}",
            format_permissions_switch_report(&previous, normalized)
        );
        Ok(true)
    }

    pub fn clear_session(&mut self, confirm: bool) -> Result<bool, Box<dyn std::error::Error>> {
        if !confirm {
            println!(
                "clear: confirmation required; run /clear --confirm to start a fresh session."
            );
            return Ok(false);
        }

        let previous_session = self.session.clone();
        let session_state = new_cli_session()?;
        self.session = create_managed_session_handle(&session_state.session_id)?;
        let runtime = build_runtime(
            session_state.with_persistence_path(self.session.path.clone()),
            &self.session.id,
            self.model.clone(),
            self.system_prompt.clone(),
            true,
            true,
            self.allowed_tools.clone(),
            self.permission_mode,
            None,
            None,
        )?;
        self.replace_runtime(runtime)?;
        println!(
            "Session cleared\n  Mode             fresh session\n  Previous session {}\n  Resume previous  /resume {}\n  Preserved model  {}\n  Permission mode  {}\n  New session      {}\n  Session file     {}",
            previous_session.id,
            previous_session.id,
            self.model,
            self.permission_mode.as_str(),
            self.session.id,
            self.session.path.display(),
        );
        Ok(true)
    }

    pub fn print_cost(&self) {
        let cumulative = self.runtime.usage().cumulative_usage();
        println!("{}", format_cost_report(cumulative));
    }

    pub fn resume_session(
        &mut self,
        session_path: Option<String>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(session_ref) = session_path else {
            println!("{}", render_resume_usage());
            return Ok(false);
        };

        let (handle, session) = load_session_reference(&session_ref)?;
        let message_count = session.messages.len();
        let session_id = session.session_id.clone();
        let runtime = build_runtime(
            session,
            &handle.id,
            self.model.clone(),
            self.system_prompt.clone(),
            true,
            true,
            self.allowed_tools.clone(),
            self.permission_mode,
            None,
            None,
        )?;
        self.replace_runtime(runtime)?;
        self.session = SessionHandle {
            id: session_id,
            path: handle.path,
        };
        println!(
            "{}",
            format_resume_report(
                &self.session.path.display().to_string(),
                message_count,
                self.runtime.usage().turns(),
            )
        );
        Ok(true)
    }

    pub fn print_config(section: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let report = render_config_report(section)?;
        print_report_or_page(&report)?;
        Ok(())
    }

    pub fn print_memory() -> Result<(), Box<dyn std::error::Error>> {
        let report = render_memory_report()?;
        print_report_or_page(&report)?;
        Ok(())
    }

    pub fn print_agents(
        args: Option<&str>,
        output_format: CliOutputFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cwd = env::current_dir()?;
        match output_format {
            CliOutputFormat::Text => println!("{}", handle_agents_slash_command(args, &cwd)?),
            CliOutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&handle_agents_slash_command_json(args, &cwd)?)?
            ),
        }
        Ok(())
    }

    pub fn print_mcp(
        args: Option<&str>,
        output_format: CliOutputFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // `claw mcp serve` starts a stdio MCP server exposing claw's built-in
        // tools. All other `mcp` subcommands fall through to the existing
        // configured-server reporter (`list`, `status`, ...).
        if matches!(args.map(str::trim), Some("serve")) {
            return run_mcp_serve();
        }
        let cwd = env::current_dir()?;
        match output_format {
            CliOutputFormat::Text => println!("{}", handle_mcp_slash_command(args, &cwd)?),
            CliOutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&handle_mcp_slash_command_json(args, &cwd)?)?
            ),
        }
        Ok(())
    }

    pub fn print_skills(
        args: Option<&str>,
        output_format: CliOutputFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cwd = env::current_dir()?;
        match output_format {
            CliOutputFormat::Text => println!("{}", handle_skills_slash_command(args, &cwd)?),
            CliOutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&handle_skills_slash_command_json(args, &cwd)?)?
            ),
        }
        Ok(())
    }

    pub fn print_plugins(
        action: Option<&str>,
        target: Option<&str>,
        output_format: CliOutputFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cwd = env::current_dir()?;
        let loader = ConfigLoader::default_for(&cwd);
        let runtime_config = loader.load()?;
        let mut manager = build_plugin_manager(&cwd, &loader, &runtime_config);
        let result = handle_plugins_slash_command(action, target, &mut manager)?;
        match output_format {
            CliOutputFormat::Text => println!("{}", result.message),
            CliOutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "kind": "plugin",
                    "action": action.unwrap_or("list"),
                    "target": target,
                    "message": result.message,
                    "reload_runtime": result.reload_runtime,
                }))?
            ),
        }
        Ok(())
    }

    pub fn print_diff() -> Result<(), Box<dyn std::error::Error>> {
        let report = render_diff_report()?;
        print_report_or_page(&report)?;
        Ok(())
    }

    pub fn print_version(output_format: CliOutputFormat) {
        let _ = crate::print_version(output_format);
    }

    pub fn export_session(
        &self,
        requested_path: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let export_path = resolve_export_path(requested_path, self.runtime.session())?;
        fs::write(&export_path, render_export_text(self.runtime.session()))?;
        println!(
            "Export\n  Result           wrote transcript\n  File             {}\n  Messages         {}",
            export_path.display(),
            self.runtime.session().messages.len(),
        );
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub fn handle_session_command(
        &mut self,
        action: Option<&str>,
        target: Option<&str>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        match action {
            None | Some("list") => {
                let sessions = list_managed_sessions()?;
                if let Some(picked) = tui::session_picker::pick_session(&sessions)? {
                    self.handle_session_command(Some("switch"), Some(&picked))
                } else {
                    Ok(false)
                }
            }
            Some("switch") => {
                let target_id = if let Some(target) = target {
                    target.to_string()
                } else {
                    let sessions = list_managed_sessions()?;
                    if let Some(picked) = tui::session_picker::pick_session(&sessions)? {
                        picked
                    } else {
                        return Ok(false);
                    }
                };
                let (handle, session) = load_session_reference(&target_id)?;
                let message_count = session.messages.len();
                let session_id = session.session_id.clone();
                let runtime = build_runtime(
                    session,
                    &handle.id,
                    self.model.clone(),
                    self.system_prompt.clone(),
                    true,
                    true,
                    self.allowed_tools.clone(),
                    self.permission_mode,
                    None,
                    None,
                )?;
                self.replace_runtime(runtime)?;
                self.session = SessionHandle {
                    id: session_id,
                    path: handle.path,
                };
                println!(
                    "Session switched\n  Active session   {}\n  File             {}\n  Messages         {}",
                    self.session.id,
                    self.session.path.display(),
                    message_count,
                );
                Ok(true)
            }
            Some("fork") => {
                let forked = self.runtime.fork_session(target.map(ToOwned::to_owned));
                let parent_session_id = self.session.id.clone();
                let handle = create_managed_session_handle(&forked.session_id)?;
                let branch_name = forked
                    .fork
                    .as_ref()
                    .and_then(|fork| fork.branch_name.clone());
                let forked = forked.with_persistence_path(handle.path.clone());
                let message_count = forked.messages.len();
                forked.save_to_path(&handle.path)?;
                let runtime = build_runtime(
                    forked,
                    &handle.id,
                    self.model.clone(),
                    self.system_prompt.clone(),
                    true,
                    true,
                    self.allowed_tools.clone(),
                    self.permission_mode,
                    None,
                    None,
                )?;
                self.replace_runtime(runtime)?;
                self.session = handle;
                println!(
                    "Session forked\n  Parent session   {}\n  Active session   {}\n  Branch           {}\n  File             {}\n  Messages         {}",
                    parent_session_id,
                    self.session.id,
                    branch_name.as_deref().unwrap_or("(unnamed)"),
                    self.session.path.display(),
                    message_count,
                );
                Ok(true)
            }
            Some("delete") => {
                let Some(target) = target else {
                    println!("Usage: /session delete <session-id> [--force]");
                    return Ok(false);
                };
                let handle = resolve_session_reference(target)?;
                if handle.id == self.session.id {
                    println!(
                        "delete: refusing to delete the active session '{}'.\nSwitch to another session first with /session switch <session-id>.",
                        handle.id
                    );
                    return Ok(false);
                }
                if !confirm_session_deletion(&handle.id) {
                    println!("delete: cancelled.");
                    return Ok(false);
                }
                delete_managed_session(&handle.path)?;
                println!(
                    "Session deleted\n  Deleted session  {}\n  File             {}",
                    handle.id,
                    handle.path.display(),
                );
                Ok(false)
            }
            Some("delete-force") => {
                let Some(target) = target else {
                    println!("Usage: /session delete <session-id> [--force]");
                    return Ok(false);
                };
                let handle = resolve_session_reference(target)?;
                if handle.id == self.session.id {
                    println!(
                        "delete: refusing to delete the active session '{}'.\nSwitch to another session first with /session switch <session-id>.",
                        handle.id
                    );
                    return Ok(false);
                }
                delete_managed_session(&handle.path)?;
                println!(
                    "Session deleted\n  Deleted session  {}\n  File             {}",
                    handle.id,
                    handle.path.display(),
                );
                Ok(false)
            }
            Some(other) => {
                println!(
                    "Unknown /session action '{other}'. Use /session list, /session switch <session-id>, /session fork [branch-name], or /session delete <session-id> [--force]."
                );
                Ok(false)
            }
        }
    }

    pub fn handle_plugins_command(
        &mut self,
        action: Option<&str>,
        target: Option<&str>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let cwd = env::current_dir()?;
        let loader = ConfigLoader::default_for(&cwd);
        let runtime_config = loader.load()?;
        let mut manager = build_plugin_manager(&cwd, &loader, &runtime_config);
        let result = handle_plugins_slash_command(action, target, &mut manager)?;
        println!("{}", result.message);
        if result.reload_runtime {
            self.reload_runtime_features()?;
        }
        Ok(false)
    }

    pub fn reload_runtime_features(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let runtime = build_runtime(
            self.runtime.session().clone(),
            &self.session.id,
            self.model.clone(),
            self.system_prompt.clone(),
            true,
            true,
            self.allowed_tools.clone(),
            self.permission_mode,
            None,
            None,
        )?;
        self.replace_runtime(runtime)?;
        self.persist_session()
    }

    pub fn compact(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.runtime.compact(CompactionConfig::default());
        let removed = result.removed_message_count;
        let kept = result.compacted_session.messages.len();
        let skipped = removed == 0;
        let runtime = build_runtime(
            result.compacted_session,
            &self.session.id,
            self.model.clone(),
            self.system_prompt.clone(),
            true,
            true,
            self.allowed_tools.clone(),
            self.permission_mode,
            None,
            None,
        )?;
        self.replace_runtime(runtime)?;
        self.persist_session()?;
        println!("{}", format_compact_report(removed, kept, skipped));
        Ok(())
    }

    pub fn run_internal_prompt_text_with_progress(
        &self,
        prompt: &str,
        enable_tools: bool,
        progress: Option<InternalPromptProgressReporter>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let session = self.runtime.session().clone();
        let mut runtime = build_runtime(
            session,
            &self.session.id,
            self.model.clone(),
            self.system_prompt.clone(),
            enable_tools,
            false,
            self.allowed_tools.clone(),
            self.permission_mode,
            progress,
            None,
        )?;
        let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
        let summary = runtime.run_turn(prompt, Some(&mut permission_prompter))?;
        let text = final_assistant_text(&summary).trim().to_string();
        runtime.shutdown_plugins()?;
        Ok(text)
    }

    pub fn run_internal_prompt_text(
        &self,
        prompt: &str,
        enable_tools: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        self.run_internal_prompt_text_with_progress(prompt, enable_tools, None)
    }

    pub fn run_bughunter(&self, scope: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", format_bughunter_report(scope));
        Ok(())
    }

    pub fn run_ultraplan(&self, task: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", format_ultraplan_report(task));
        Ok(())
    }

    pub fn run_teleport(target: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) else {
            println!("Usage: /teleport <symbol-or-path>");
            return Ok(());
        };

        println!("{}", render_teleport_report(target)?);
        Ok(())
    }

    pub fn run_debug_tool_call(&self, args: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        validate_no_args("/debug-tool-call", args)?;
        println!("{}", render_last_tool_debug_report(self.runtime.session())?);
        Ok(())
    }

    pub fn run_commit(&mut self, args: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        validate_no_args("/commit", args)?;
        let status = git_output(&["status", "--short", "--branch"])?;
        let summary = parse_git_workspace_summary(Some(&status));
        let branch = parse_git_status_branch(Some(&status));
        if summary.is_clean() {
            println!("{}", format_commit_skipped_report());
            return Ok(());
        }

        println!(
            "{}",
            format_commit_preflight_report(branch.as_deref(), summary)
        );
        Ok(())
    }

    pub fn run_pr(&self, context: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let branch =
            resolve_git_branch_for(&env::current_dir()?).unwrap_or_else(|| "unknown".to_string());
        println!("{}", format_pr_report(&branch, context));
        Ok(())
    }

    pub fn run_issue(&self, context: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", format_issue_report(context));
        Ok(())
    }
}
















pub fn render_repl_help() -> String {
    [
        "REPL".to_string(),
        "  /exit                Quit the REPL".to_string(),
        "  /quit                Quit the REPL".to_string(),
        "  Up/Down              Navigate prompt history".to_string(),
        "  Ctrl-R               Reverse-search prompt history".to_string(),
        "  Tab                  Complete commands, modes, and recent sessions".to_string(),
        "  Ctrl-C               Clear input (or exit on empty prompt)".to_string(),
        "  Shift+Enter/Ctrl+J   Insert a newline".to_string(),
        "  Auto-save            .claw/sessions/<session-id>.jsonl".to_string(),
        "  Resume latest        /resume latest".to_string(),
        "  Browse sessions      /session list".to_string(),
        "  Show prompt history  /history [count]".to_string(),
        String::new(),
        render_slash_command_help_filtered(STUB_COMMANDS),
    ]
    .join(
        "
",
    )
}

pub fn print_status_snapshot(
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

pub fn status_json_value(
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
            "session_id": context.session_path.as_ref().and_then(|path| {
                // Session files are named <session-id>.jsonl directly under
                // .claw/sessions/. Extract the stem (drop the .jsonl extension).
                path.file_stem().map(|n| n.to_string_lossy().into_owned())
            }),
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

pub fn status_context(
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

pub fn format_status_report(
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
    .join(
        "

",
    )
}


pub fn format_commit_preflight_report(branch: Option<&str>, summary: GitWorkspaceSummary) -> String {
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

pub fn format_commit_skipped_report() -> String {
    "Commit
  Result           skipped
  Reason           no workspace changes
  Action           create a git commit from the current workspace changes
  Next             /status to inspect context · /diff to inspect repo changes"
        .to_string()
}



pub fn render_help_topic(topic: LocalHelpTopic) -> String {
    match topic {
        LocalHelpTopic::Status => "Status
  Usage            claw status
  Purpose          show the local workspace snapshot without entering the REPL
  Output           model, permissions, git state, config files, and sandbox status
  Related          /status · claw --resume latest /status"
            .to_string(),
        LocalHelpTopic::Sandbox => "Sandbox
  Usage            claw sandbox
  Purpose          inspect the resolved sandbox and isolation state for the current directory
  Output           namespace, network, filesystem, and fallback details
  Related          /sandbox · claw status"
            .to_string(),
        LocalHelpTopic::Doctor => "Doctor
  Usage            claw doctor
  Purpose          diagnose local auth, config, workspace, sandbox, and build metadata
  Output           local-only health report; no provider request or session resume required
  Related          /doctor · claw --resume latest /doctor"
            .to_string(),
    }
}

pub fn print_help_topic(topic: LocalHelpTopic) {
    println!("{}", render_help_topic(topic));
}

pub fn render_config_report(section: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let loader = ConfigLoader::default_for(&cwd);
    let discovered = loader.discover();
    let runtime_config = loader.load()?;

    let mut lines = vec![
        format!(
            "Config
  Working directory {}
  Loaded files      {}
  Merged keys       {}",
            cwd.display(),
            runtime_config.loaded_entries().len(),
            runtime_config.merged().len()
        ),
        "Discovered files".to_string(),
    ];
    for entry in discovered {
        let source = match entry.source {
            ConfigSource::User => "user",
            ConfigSource::Project => "project",
            ConfigSource::Local => "local",
        };
        let status = if runtime_config
            .loaded_entries()
            .iter()
            .any(|loaded_entry| loaded_entry.path == entry.path)
        {
            "loaded"
        } else {
            "missing"
        };
        lines.push(format!(
            "  {source:<7} {status:<7} {}",
            entry.path.display()
        ));
    }

    if let Some(section) = section {
        lines.push(format!("Merged section: {section}"));
        let value = match section {
            "env" => runtime_config.get("env"),
            "hooks" => runtime_config.get("hooks"),
            "model" => runtime_config.get("model"),
            "plugins" => runtime_config
                .get("plugins")
                .or_else(|| runtime_config.get("enabledPlugins")),
            other => {
                lines.push(format!(
                    "  Unsupported config section '{other}'. Use env, hooks, model, or plugins."
                ));
                return Ok(lines.join(
                    "
",
                ));
            }
        };
        lines.push(format!(
            "  {}",
            match value {
                Some(value) => value.render(),
                None => "<unset>".to_string(),
            }
        ));
        return Ok(lines.join(
            "
",
        ));
    }

    lines.push("Merged JSON".to_string());
    lines.push(format!("  {}", runtime_config.as_json().render()));
    Ok(lines.join(
        "
",
    ))
}

pub fn render_config_json(
    _section: Option<&str>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let loader = ConfigLoader::default_for(&cwd);
    let discovered = loader.discover();
    let runtime_config = loader.load()?;

    let loaded_paths: Vec<_> = runtime_config
        .loaded_entries()
        .iter()
        .map(|e| e.path.display().to_string())
        .collect();

    let files: Vec<_> = discovered
        .iter()
        .map(|e| {
            let source = match e.source {
                ConfigSource::User => "user",
                ConfigSource::Project => "project",
                ConfigSource::Local => "local",
            };
            let is_loaded = runtime_config
                .loaded_entries()
                .iter()
                .any(|le| le.path == e.path);
            serde_json::json!({
                "path": e.path.display().to_string(),
                "source": source,
                "loaded": is_loaded,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "kind": "config",
        "cwd": cwd.display().to_string(),
        "loaded_files": loaded_paths.len(),
        "merged_keys": runtime_config.merged().len(),
        "files": files,
    }))
}

pub fn render_memory_report() -> Result<String, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let project_context = ProjectContext::discover(&cwd, DEFAULT_DATE)?;
    let mut lines = vec![format!(
        "Memory
  Working directory {}
  Instruction files {}",
        cwd.display(),
        project_context.instruction_files.len()
    )];
    if project_context.instruction_files.is_empty() {
        lines.push("Discovered files".to_string());
        lines.push(
            "  No CLAUDE instruction files discovered in the current directory ancestry."
                .to_string(),
        );
    } else {
        lines.push("Discovered files".to_string());
        for (index, file) in project_context.instruction_files.iter().enumerate() {
            let preview = file.content.lines().next().unwrap_or("").trim();
            let preview = if preview.is_empty() {
                "<empty>"
            } else {
                preview
            };
            lines.push(format!("  {}. {}", index + 1, file.path.display(),));
            lines.push(format!(
                "     lines={} preview={}",
                file.content.lines().count(),
                preview
            ));
        }
    }
    Ok(lines.join(
        "
",
    ))
}

pub fn render_memory_json() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let project_context = ProjectContext::discover(&cwd, DEFAULT_DATE)?;
    let files: Vec<_> = project_context
        .instruction_files
        .iter()
        .map(|f| {
            json!({
                "path": f.path.display().to_string(),
                "lines": f.content.lines().count(),
                "preview": f.content.lines().next().unwrap_or("").trim(),
            })
        })
        .collect();
    Ok(json!({
        "kind": "memory",
        "cwd": cwd.display().to_string(),
        "instruction_files": files.len(),
        "files": files,
    }))
}

pub fn init_claude_md() -> Result<String, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    Ok(initialize_repo(&cwd)?.render())
}

pub fn run_init(output_format: CliOutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    let message = init_claude_md()?;
    match output_format {
        CliOutputFormat::Text => println!("{message}"),
        CliOutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&init_json_value(&message))?
        ),
    }
    Ok(())
}

pub fn init_json_value(message: &str) -> serde_json::Value {
    json!({
        "kind": "init",
        "message": message,
    })
}

pub fn normalize_permission_mode(mode: &str) -> Option<&'static str> {
    match mode.trim() {
        "read-only" => Some("read-only"),
        "workspace-write" => Some("workspace-write"),
        "danger-full-access" => Some("danger-full-access"),
        _ => None,
    }
}





pub fn render_teleport_report(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;

    let file_list = Command::new("rg")
        .args(["--files"])
        .current_dir(&cwd)
        .output()?;
    let file_matches = if file_list.status.success() {
        String::from_utf8(file_list.stdout)?
            .lines()
            .filter(|line| line.contains(target))
            .take(10)
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let content_output = Command::new("rg")
        .args(["-n", "-S", "--color", "never", target, "."])
        .current_dir(&cwd)
        .output()?;

    let mut lines = vec![
        "Teleport".to_string(),
        format!("  Target           {target}"),
        "  Action           search workspace files and content for the target".to_string(),
    ];
    if !file_matches.is_empty() {
        lines.push(String::new());
        lines.push("File matches".to_string());
        lines.extend(file_matches.into_iter().map(|path| format!("  {path}")));
    }

    if content_output.status.success() {
        let matches = String::from_utf8(content_output.stdout)?;
        if !matches.trim().is_empty() {
            lines.push(String::new());
            lines.push("Content matches".to_string());
            lines.push(truncate_for_prompt(&matches, 4_000));
        }
    }

    if lines.len() == 1 {
        lines.push("  Result           no matches found".to_string());
    }

    Ok(lines.join("\n"))
}

pub fn render_last_tool_debug_report(session: &Session) -> Result<String, Box<dyn std::error::Error>> {
    let last_tool_use = session
        .messages
        .iter()
        .rev()
        .find_map(|message| {
            message.blocks.iter().rev().find_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.clone(), name.clone(), input.clone()))
                }
                _ => None,
            })
        })
        .ok_or_else(|| "no prior tool call found in session".to_string())?;

    let tool_result = session.messages.iter().rev().find_map(|message| {
        message.blocks.iter().rev().find_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                tool_name,
                output,
                is_error,
            } if tool_use_id == &last_tool_use.0 => {
                Some((tool_name.clone(), output.clone(), *is_error))
            }
            _ => None,
        })
    });

    let mut lines = vec![
        "Debug tool call".to_string(),
        "  Action           inspect the last recorded tool call and its result".to_string(),
        format!("  Tool id          {}", last_tool_use.0),
        format!("  Tool name        {}", last_tool_use.1),
        "  Input".to_string(),
        indent_block(&last_tool_use.2, 4),
    ];

    match tool_result {
        Some((tool_name, output, is_error)) => {
            lines.push("  Result".to_string());
            lines.push(format!("    name           {tool_name}"));
            lines.push(format!(
                "    status         {}",
                if is_error { "error" } else { "ok" }
            ));
            lines.push(indent_block(&output, 4));
        }
        None => lines.push("  Result           missing tool result".to_string()),
    }

    Ok(lines.join("\n"))
}

pub fn indent_block(value: &str, spaces: usize) -> String {
    let indent = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn validate_no_args(
    command_name: &str,
    args: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(args) = args.map(str::trim).filter(|value| !value.is_empty()) {
        return Err(format!(
            "{command_name} does not accept arguments. Received: {args}\nUsage: {command_name}"
        )
        .into());
    }
    Ok(())
}

pub fn format_bughunter_report(scope: Option<&str>) -> String {
    format!(
        "Bughunter
  Scope            {}
  Action           inspect the selected code for likely bugs and correctness issues
  Output           findings should include file paths, severity, and suggested fixes",
        scope.unwrap_or("the current repository")
    )
}

pub fn format_ultraplan_report(task: Option<&str>) -> String {
    format!(
        "Ultraplan
  Task             {}
  Action           break work into a multi-step execution plan
  Output           plan should cover goals, risks, sequencing, verification, and rollback",
        task.unwrap_or("the current repo work")
    )
}

pub fn format_pr_report(branch: &str, context: Option<&str>) -> String {
    format!(
        "PR
  Branch           {branch}
  Context          {}
  Action           draft or create a pull request for the current branch
  Output           title and markdown body suitable for GitHub",
        context.unwrap_or("none")
    )
}

pub fn format_issue_report(context: Option<&str>) -> String {
    format!(
        "Issue
  Context          {}
  Action           draft or create a GitHub issue from the current context
  Output           title and markdown body suitable for GitHub",
        context.unwrap_or("none")
    )
}

pub fn git_output(args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(env::current_dir()?)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("git {} failed: {stderr}", args.join(" ")).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

pub fn git_status_ok(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(env::current_dir()?)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("git {} failed: {stderr}", args.join(" ")).into());
    }
    Ok(())
}

pub fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn write_temp_text_file(
    filename: &str,
    contents: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = env::temp_dir().join(filename);
    fs::write(&path, contents)?;
    Ok(path)
}

pub const DEFAULT_HISTORY_LIMIT: usize = 20;

pub fn parse_history_count(raw: Option<&str>) -> Result<usize, String> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_HISTORY_LIMIT);
    };
    let parsed: usize = raw
        .parse()
        .map_err(|_| format!("history: invalid count '{raw}'. Expected a positive integer."))?;
    if parsed == 0 {
        return Err("history: count must be greater than 0.".to_string());
    }
    Ok(parsed)
}

pub fn format_history_timestamp(timestamp_ms: u64) -> String {
    let secs = timestamp_ms / 1_000;
    let subsec_ms = timestamp_ms % 1_000;
    let days_since_epoch = secs / 86_400;
    let seconds_of_day = secs % 86_400;
    let hours = seconds_of_day / 3_600;
    let minutes = (seconds_of_day % 3_600) / 60;
    let seconds = seconds_of_day % 60;

    let (year, month, day) = civil_from_days(i64::try_from(days_since_epoch).unwrap_or(0));
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{subsec_ms:03}Z")
}

// Computes civil (Gregorian) year/month/day from days since the Unix epoch
// (1970-01-01) using Howard Hinnant's `civil_from_days` algorithm.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation
)]
pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64; // [0, 146_096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = y + i64::from(m <= 2);
    (y as i32, m as u32, d as u32)
}

pub fn render_prompt_history_report(entries: &[PromptHistoryEntry], limit: usize) -> String {
    if entries.is_empty() {
        return "Prompt history\n  Result           no prompts recorded yet".to_string();
    }

    let total = entries.len();
    let start = total.saturating_sub(limit);
    let shown = &entries[start..];
    let mut lines = vec![
        "Prompt history".to_string(),
        format!("  Total            {total}"),
        format!("  Showing          {} most recent", shown.len()),
        format!("  Reverse search   Ctrl-R in the REPL"),
        String::new(),
    ];
    for (offset, entry) in shown.iter().enumerate() {
        let absolute_index = start + offset + 1;
        let timestamp = format_history_timestamp(entry.timestamp_ms);
        let first_line = entry.text.lines().next().unwrap_or("").trim();
        let display = if first_line.chars().count() > 80 {
            let truncated: String = first_line.chars().take(77).collect();
            format!("{truncated}...")
        } else {
            first_line.to_string()
        };
        lines.push(format!("  {absolute_index:>3}. [{timestamp}] {display}"));
    }
    lines.join("\n")
}

pub fn collect_session_prompt_history(session: &Session) -> Vec<PromptHistoryEntry> {
    if !session.prompt_history.is_empty() {
        return session
            .prompt_history
            .iter()
            .map(|entry| PromptHistoryEntry {
                timestamp_ms: entry.timestamp_ms,
                text: entry.text.clone(),
            })
            .collect();
    }
    let timestamp_ms = session.updated_at_ms;
    session
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .filter_map(|message| {
            message.blocks.iter().find_map(|block| match block {
                ContentBlock::Text { text } => Some(PromptHistoryEntry {
                    timestamp_ms,
                    text: text.clone(),
                }),
                _ => None,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationSearchHit {
    pub message_index: usize,
    pub role: &'static str,
    pub block_kind: &'static str,
    pub snippet: String,
}

pub fn render_conversation_search_report(session: &Session, query: &str) -> String {
    let query = query.trim();
    if query.is_empty() {
        return "Conversation search\n  Usage            /search <keyword>\n  Scope            current conversation history"
            .to_string();
    }

    let limit = 20;
    let hits = collect_conversation_search_hits(session, query, limit);
    if hits.is_empty() {
        return format!(
            "Conversation search\n  Query            {query}\n  Result           no matches in current conversation history"
        );
    }

    let mut lines = vec![
        "Conversation search".to_string(),
        format!("  Query            {query}"),
        format!("  Matches          {} shown", hits.len()),
        String::new(),
    ];
    for hit in hits {
        lines.push(format!(
            "  {:>3}. {} {}",
            hit.message_index, hit.role, hit.block_kind
        ));
        lines.push(format!("       {}", hit.snippet));
    }
    lines.join("\n")
}

pub fn collect_conversation_search_hits(
    session: &Session,
    query: &str,
    limit: usize,
) -> Vec<ConversationSearchHit> {
    let normalized_query = query.to_ascii_lowercase();
    let mut hits = Vec::new();
    for (index, message) in session.messages.iter().enumerate() {
        for block in &message.blocks {
            let Some((block_kind, text)) = searchable_block_text(block) else {
                continue;
            };
            if !text.to_ascii_lowercase().contains(&normalized_query) {
                continue;
            }
            hits.push(ConversationSearchHit {
                message_index: index + 1,
                role: message_role_label(message.role),
                block_kind,
                snippet: search_snippet(&text, &normalized_query),
            });
            if hits.len() >= limit {
                return hits;
            }
        }
    }
    hits
}

pub fn searchable_block_text(block: &ContentBlock) -> Option<(&'static str, String)> {
    match block {
        ContentBlock::Text { text } => Some(("text", text.clone())),
        ContentBlock::ToolUse { name, input, .. } => Some(("tool-call", format!("{name} {input}"))),
        ContentBlock::ToolResult {
            tool_name, output, ..
        } => Some(("tool-result", format!("{tool_name} {output}"))),
    }
}

pub fn message_role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

pub fn search_snippet(text: &str, normalized_query: &str) -> String {
    let line = text
        .lines()
        .find(|line| line.to_ascii_lowercase().contains(normalized_query))
        .unwrap_or(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    truncate_for_summary(&line, 140)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileUndoKind {
    RestoreOriginal,
    DeleteCreatedFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileUndoAction {
    pub tool_name: String,
    pub file_path: String,
    pub original_file: Option<String>,
    pub expected_current: Option<String>,
    pub kind: FileUndoKind,
}

pub fn find_last_file_undo_action(session: &Session) -> Option<FileUndoAction> {
    session
        .messages
        .iter()
        .rev()
        .flat_map(|message| message.blocks.iter().rev())
        .find_map(|block| match block {
            ContentBlock::ToolResult {
                tool_name,
                output,
                is_error,
                ..
            } if !is_error => parse_file_undo_action(tool_name, output),
            _ => None,
        })
}

pub fn parse_file_undo_action(tool_name: &str, output: &str) -> Option<FileUndoAction> {
    let parsed = serde_json::from_str::<serde_json::Value>(output).ok()?;
    let normalized_tool = match tool_name {
        "write_file" | "Write" => "write_file",
        "edit_file" | "Edit" => "edit_file",
        _ => return None,
    };
    let file_path = json_string_field(&parsed, &["filePath", "file_path", "path"])?;
    match normalized_tool {
        "write_file" => {
            let original_file = json_string_field(&parsed, &["originalFile", "original_file"]);
            let expected_current = json_string_field(&parsed, &["content"]);
            let kind = if original_file.is_some() {
                FileUndoKind::RestoreOriginal
            } else if parsed
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| value == "create")
            {
                FileUndoKind::DeleteCreatedFile
            } else {
                return None;
            };
            Some(FileUndoAction {
                tool_name: tool_name.to_string(),
                file_path,
                original_file,
                expected_current,
                kind,
            })
        }
        "edit_file" => {
            let original_file = json_string_field(&parsed, &["originalFile", "original_file"])?;
            let old_string = json_string_field(&parsed, &["oldString", "old_string"])?;
            let new_string = json_string_field(&parsed, &["newString", "new_string"])?;
            let replace_all = parsed
                .get("replaceAll")
                .or_else(|| parsed.get("replace_all"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let expected_current = if replace_all {
                original_file.replace(&old_string, &new_string)
            } else {
                original_file.replacen(&old_string, &new_string, 1)
            };
            Some(FileUndoAction {
                tool_name: tool_name.to_string(),
                file_path,
                original_file: Some(original_file),
                expected_current: Some(expected_current),
                kind: FileUndoKind::RestoreOriginal,
            })
        }
        _ => None,
    }
}

pub fn json_string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(ToOwned::to_owned)
}

pub fn apply_file_undo_action(
    action: &FileUndoAction,
    session: &Session,
) -> Result<String, Box<dyn std::error::Error>> {
    let path = resolve_undo_path(&action.file_path, session)?;
    let Some(expected_current) = action.expected_current.as_deref() else {
        return Ok(format!(
            "Undo\n  Result           refused\n  Reason           recorded {} result does not include the expected current file content\n  File             {}",
            action.tool_name,
            path.display()
        ));
    };
    let current = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(error) => {
            return Ok(format!(
                "Undo\n  Result           refused\n  Reason           could not read target file: {error}\n  File             {}",
                path.display()
            ));
        }
    };
    if current != expected_current {
        return Ok(format!(
            "Undo\n  Result           refused\n  Reason           target file changed after the recorded {} result\n  File             {}",
            action.tool_name,
            path.display()
        ));
    }

    match action.kind {
        FileUndoKind::RestoreOriginal => {
            let original = action.original_file.as_deref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "restore undo action is missing originalFile",
                )
            })?;
            fs::write(&path, original)?;
            Ok(format!(
                "Undo\n  Result           restored original file\n  Tool             {}\n  File             {}\n  Bytes restored   {}",
                action.tool_name,
                path.display(),
                original.len()
            ))
        }
        FileUndoKind::DeleteCreatedFile => {
            fs::remove_file(&path)?;
            Ok(format!(
                "Undo\n  Result           removed created file\n  Tool             {}\n  File             {}",
                action.tool_name,
                path.display()
            ))
        }
    }
}

pub fn resolve_undo_path(
    raw_path: &str,
    session: &Session,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        return Ok(path);
    }
    if let Some(root) = session.workspace_root() {
        return Ok(root.join(path));
    }
    Ok(env::current_dir()?.join(path))
}

pub fn recent_user_context(session: &Session, limit: usize) -> String {
    let requests = session
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .filter_map(|message| {
            message.blocks.iter().find_map(|block| match block {
                ContentBlock::Text { text } => Some(text.trim().to_string()),
                _ => None,
            })
        })
        .rev()
        .take(limit)
        .collect::<Vec<_>>();

    if requests.is_empty() {
        "<no prior user messages>".to_string()
    } else {
        requests
            .into_iter()
            .rev()
            .enumerate()
            .map(|(index, text)| format!("{}. {}", index + 1, text))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn truncate_for_prompt(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.trim().to_string()
    } else {
        let truncated = value.chars().take(limit).collect::<String>();
        format!("{}\n…[truncated]", truncated.trim_end())
    }
}

pub fn sanitize_generated_message(value: &str) -> String {
    value.trim().trim_matches('`').trim().replace("\r\n", "\n")
}

pub fn parse_titled_body(value: &str) -> Option<(String, String)> {
    let normalized = sanitize_generated_message(value);
    let title = normalized
        .lines()
        .find_map(|line| line.strip_prefix("TITLE:").map(str::trim))?;
    let body_start = normalized.find("BODY:")?;
    let body = normalized[body_start + "BODY:".len()..].trim();
    Some((title.to_string(), body.to_string()))
}

pub fn render_version_report() -> String {
    let git_sha = GIT_SHA.unwrap_or("unknown");
    let target = BUILD_TARGET.unwrap_or("unknown");
    format!(
        "Claw Code\n  Version          {VERSION}\n  Git SHA          {git_sha}\n  Target           {target}\n  Build date       {DEFAULT_DATE}"
    )
}

pub fn render_export_text(session: &Session) -> String {
    let mut lines = vec!["# Conversation Export".to_string(), String::new()];
    for (index, message) in session.messages.iter().enumerate() {
        let role = match message.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };
        lines.push(format!("## {}. {role}", index + 1));
        for block in &message.blocks {
            match block {
                ContentBlock::Text { text } => lines.push(text.clone()),
                ContentBlock::ToolUse { id, name, input } => {
                    lines.push(format!("[tool_use id={id} name={name}] {input}"));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    tool_name,
                    output,
                    is_error,
                } => {
                    lines.push(format!(
                        "[tool_result id={tool_use_id} name={tool_name} error={is_error}] {output}"
                    ));
                }
            }
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

pub fn default_export_filename(session: &Session) -> String {
    let stem = session
        .messages
        .iter()
        .find_map(|message| match message.role {
            MessageRole::User => message.blocks.iter().find_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            }),
            _ => None,
        })
        .map_or("conversation", |text| {
            text.lines().next().unwrap_or("conversation")
        })
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    let fallback = if stem.is_empty() {
        "conversation"
    } else {
        &stem
    };
    format!("{fallback}.txt")
}

pub fn resolve_export_path(
    requested_path: Option<&str>,
    session: &Session,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let file_name =
        requested_path.map_or_else(|| default_export_filename(session), ToOwned::to_owned);
    let final_name = if Path::new(&file_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
    {
        file_name
    } else {
        format!("{file_name}.txt")
    };
    Ok(cwd.join(final_name))
}

pub const SESSION_MARKDOWN_TOOL_SUMMARY_LIMIT: usize = 280;

pub fn summarize_tool_payload_for_markdown(payload: &str) -> String {
    let compact = match serde_json::from_str::<serde_json::Value>(payload) {
        Ok(value) => value.to_string(),
        Err(_) => payload.split_whitespace().collect::<Vec<_>>().join(" "),
    };
    if compact.is_empty() {
        return String::new();
    }
    truncate_for_summary(&compact, SESSION_MARKDOWN_TOOL_SUMMARY_LIMIT)
}

pub fn run_export(
    session_reference: &str,
    output_path: Option<&Path>,
    output_format: CliOutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let (handle, session) = load_session_reference(session_reference)?;
    let markdown = render_session_markdown(&session, &handle.id, &handle.path);

    if let Some(path) = output_path {
        fs::write(path, &markdown)?;
        let report = format!(
            "Export\n  Result           wrote markdown transcript\n  File             {}\n  Session          {}\n  Messages         {}",
            path.display(),
            handle.id,
            session.messages.len(),
        );
        match output_format {
            CliOutputFormat::Text => println!("{report}"),
            CliOutputFormat::Json => println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "kind": "export",
                    "message": report,
                    "session_id": handle.id,
                    "file": path.display().to_string(),
                    "messages": session.messages.len(),
                }))?
            ),
        }
        return Ok(());
    }

    match output_format {
        CliOutputFormat::Text => {
            print!("{markdown}");
            if !markdown.ends_with('\n') {
                println!();
            }
        }
        CliOutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": "export",
                "session_id": handle.id,
                "file": handle.path.display().to_string(),
                "messages": session.messages.len(),
                "markdown": markdown,
            }))?
        ),
    }
    Ok(())
}

pub fn render_session_markdown(session: &Session, session_id: &str, session_path: &Path) -> String {
    let mut lines = vec![
        "# Conversation Export".to_string(),
        String::new(),
        format!("- **Session**: `{session_id}`"),
        format!("- **File**: `{}`", session_path.display()),
        format!("- **Messages**: {}", session.messages.len()),
    ];
    if let Some(workspace_root) = session.workspace_root() {
        lines.push(format!("- **Workspace**: `{}`", workspace_root.display()));
    }
    if let Some(fork) = &session.fork {
        let branch = fork.branch_name.as_deref().unwrap_or("(unnamed)");
        lines.push(format!(
            "- **Forked from**: `{}` (branch `{branch}`)",
            fork.parent_session_id
        ));
    }
    if let Some(compaction) = &session.compaction {
        lines.push(format!(
            "- **Compactions**: {} (last removed {} messages)",
            compaction.count, compaction.removed_message_count
        ));
    }
    lines.push(String::new());
    lines.push("---".to_string());
    lines.push(String::new());

    for (index, message) in session.messages.iter().enumerate() {
        let role = match message.role {
            MessageRole::System => "System",
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::Tool => "Tool",
        };
        lines.push(format!("## {}. {role}", index + 1));
        lines.push(String::new());
        for block in &message.blocks {
            match block {
                ContentBlock::Text { text } => {
                    let trimmed = text.trim_end();
                    if !trimmed.is_empty() {
                        lines.push(trimmed.to_string());
                        lines.push(String::new());
                    }
                }
                ContentBlock::ToolUse { id, name, input } => {
                    lines.push(format!(
                        "**Tool call** `{name}` _(id `{}`)_",
                        short_tool_id(id)
                    ));
                    let summary = summarize_tool_payload_for_markdown(input);
                    if !summary.is_empty() {
                        lines.push(format!("> {summary}"));
                    }
                    lines.push(String::new());
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    tool_name,
                    output,
                    is_error,
                } => {
                    let status = if *is_error { "error" } else { "ok" };
                    lines.push(format!(
                        "**Tool result** `{tool_name}` _(id `{}`, {status})_",
                        short_tool_id(tool_use_id)
                    ));
                    let summary = summarize_tool_payload_for_markdown(output);
                    if !summary.is_empty() {
                        lines.push(format!("> {summary}"));
                    }
                    lines.push(String::new());
                }
            }
        }
        if let Some(usage) = message.usage {
            lines.push(format!(
                "_tokens: in={} out={} cache_create={} cache_read={}_",
                usage.input_tokens,
                usage.output_tokens,
                usage.cache_creation_input_tokens,
                usage.cache_read_input_tokens,
            ));
            lines.push(String::new());
        }
    }
    lines.join("\n")
}

pub fn short_tool_id(id: &str) -> String {
    let char_count = id.chars().count();
    if char_count <= 12 {
        return id.to_string();
    }
    let prefix: String = id.chars().take(12).collect();
    format!("{prefix}…")
}

pub fn build_system_prompt() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(load_system_prompt(
        env::current_dir()?,
        DEFAULT_DATE,
        env::consts::OS,
        "unknown",
    )?)
}

pub fn build_runtime_plugin_state() -> Result<RuntimePluginState, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let loader = ConfigLoader::default_for(&cwd);
    let runtime_config = loader.load()?;
    build_runtime_plugin_state_with_loader(&cwd, &loader, &runtime_config)
}

pub fn build_runtime_plugin_state_with_loader(
    cwd: &Path,
    loader: &ConfigLoader,
    runtime_config: &runtime::RuntimeConfig,
) -> Result<RuntimePluginState, Box<dyn std::error::Error>> {
    let plugin_manager = build_plugin_manager(cwd, loader, runtime_config);
    let plugin_registry = plugin_manager.plugin_registry()?;
    let plugin_hook_config =
        runtime_hook_config_from_plugin_hooks(plugin_registry.aggregated_hooks()?);
    let feature_config = runtime_config
        .feature_config()
        .clone()
        .with_hooks(runtime_config.hooks().merged(&plugin_hook_config));
    let (mcp_state, runtime_tools) = build_runtime_mcp_state(runtime_config)?;
    let tool_registry = GlobalToolRegistry::with_plugin_tools(plugin_registry.aggregated_tools()?)?
        .with_runtime_tools(runtime_tools)?;
    Ok(RuntimePluginState {
        feature_config,
        tool_registry,
        plugin_registry,
        mcp_state,
    })
}

pub fn build_plugin_manager(
    cwd: &Path,
    loader: &ConfigLoader,
    runtime_config: &runtime::RuntimeConfig,
) -> PluginManager {
    let plugin_settings = runtime_config.plugins();
    let mut plugin_config = PluginManagerConfig::new(loader.config_home().to_path_buf());
    plugin_config.enabled_plugins = plugin_settings.enabled_plugins().clone();
    plugin_config.external_dirs = plugin_settings
        .external_directories()
        .iter()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path))
        .collect();
    plugin_config.install_root = plugin_settings
        .install_root()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    plugin_config.registry_path = plugin_settings
        .registry_path()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    plugin_config.bundled_root = plugin_settings
        .bundled_root()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    PluginManager::new(plugin_config)
}

pub fn resolve_plugin_path(cwd: &Path, config_home: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else if value.starts_with('.') {
        cwd.join(path)
    } else {
        config_home.join(path)
    }
}

pub fn runtime_hook_config_from_plugin_hooks(hooks: PluginHooks) -> runtime::RuntimeHookConfig {
    runtime::RuntimeHookConfig::new(
        hooks.pre_tool_use,
        hooks.post_tool_use,
        hooks.post_tool_use_failure,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalPromptProgressState {
    pub command_label: &'static str,
    pub task_label: String,
    pub step: usize,
    pub phase: String,
    pub detail: Option<String>,
    pub saw_final_text: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalPromptProgressEvent {
    Started,
    Update,
    Heartbeat,
    Complete,
    Failed,
}

#[derive(Debug)]
pub struct InternalPromptProgressShared {
    pub state: Mutex<InternalPromptProgressState>,
    pub output_lock: Mutex<()>,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub struct InternalPromptProgressReporter {
    pub shared: Arc<InternalPromptProgressShared>,
}

#[derive(Debug)]
pub struct InternalPromptProgressRun {
    pub reporter: InternalPromptProgressReporter,
    pub heartbeat_stop: Option<mpsc::Sender<()>>,
    pub heartbeat_handle: Option<thread::JoinHandle<()>>,
}

impl InternalPromptProgressReporter {
    pub fn ultraplan(task: &str) -> Self {
        Self {
            shared: Arc::new(InternalPromptProgressShared {
                state: Mutex::new(InternalPromptProgressState {
                    command_label: "Ultraplan",
                    task_label: task.to_string(),
                    step: 0,
                    phase: "planning started".to_string(),
                    detail: Some(format!("task: {task}")),
                    saw_final_text: false,
                }),
                output_lock: Mutex::new(()),
                started_at: Instant::now(),
            }),
        }
    }

    pub fn emit(&self, event: InternalPromptProgressEvent, error: Option<&str>) {
        let snapshot = self.snapshot();
        let line = format_internal_prompt_progress_line(event, &snapshot, self.elapsed(), error);
        self.write_line(&line);
    }

    pub fn mark_model_phase(&self) {
        let snapshot = {
            let mut state = self
                .shared
                .state
                .lock()
                .expect("internal prompt progress state poisoned");
            state.step += 1;
            state.phase = if state.step == 1 {
                "analyzing request".to_string()
            } else {
                "reviewing findings".to_string()
            };
            state.detail = Some(format!("task: {}", state.task_label));
            state.clone()
        };
        self.write_line(&format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Update,
            &snapshot,
            self.elapsed(),
            None,
        ));
    }

    pub fn mark_tool_phase(&self, name: &str, input: &str) {
        let detail = describe_tool_progress(name, input);
        let snapshot = {
            let mut state = self
                .shared
                .state
                .lock()
                .expect("internal prompt progress state poisoned");
            state.step += 1;
            state.phase = format!("running {name}");
            state.detail = Some(detail);
            state.clone()
        };
        self.write_line(&format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Update,
            &snapshot,
            self.elapsed(),
            None,
        ));
    }

    pub fn mark_text_phase(&self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let detail = truncate_for_summary(first_visible_line(trimmed), 120);
        let snapshot = {
            let mut state = self
                .shared
                .state
                .lock()
                .expect("internal prompt progress state poisoned");
            if state.saw_final_text {
                return;
            }
            state.saw_final_text = true;
            state.step += 1;
            state.phase = "drafting final plan".to_string();
            state.detail = (!detail.is_empty()).then_some(detail);
            state.clone()
        };
        self.write_line(&format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Update,
            &snapshot,
            self.elapsed(),
            None,
        ));
    }

    pub fn emit_heartbeat(&self) {
        let snapshot = self.snapshot();
        self.write_line(&format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Heartbeat,
            &snapshot,
            self.elapsed(),
            None,
        ));
    }

    pub fn snapshot(&self) -> InternalPromptProgressState {
        self.shared
            .state
            .lock()
            .expect("internal prompt progress state poisoned")
            .clone()
    }

    pub fn elapsed(&self) -> Duration {
        self.shared.started_at.elapsed()
    }

    pub fn write_line(&self, line: &str) {
        let _guard = self
            .shared
            .output_lock
            .lock()
            .expect("internal prompt progress output lock poisoned");
        let mut stdout = io::stdout();
        let _ = writeln!(stdout, "{line}");
        let _ = stdout.flush();
    }
}

impl InternalPromptProgressRun {
    pub fn start_ultraplan(task: &str) -> Self {
        let reporter = InternalPromptProgressReporter::ultraplan(task);
        reporter.emit(InternalPromptProgressEvent::Started, None);

        let (heartbeat_stop, heartbeat_rx) = mpsc::channel();
        let heartbeat_reporter = reporter.clone();
        let heartbeat_handle = thread::spawn(move || loop {
            match heartbeat_rx.recv_timeout(INTERNAL_PROGRESS_HEARTBEAT_INTERVAL) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => heartbeat_reporter.emit_heartbeat(),
            }
        });

        Self {
            reporter,
            heartbeat_stop: Some(heartbeat_stop),
            heartbeat_handle: Some(heartbeat_handle),
        }
    }

    pub fn reporter(&self) -> InternalPromptProgressReporter {
        self.reporter.clone()
    }

    pub fn finish_success(&mut self) {
        self.stop_heartbeat();
        self.reporter
            .emit(InternalPromptProgressEvent::Complete, None);
    }

    pub fn finish_failure(&mut self, error: &str) {
        self.stop_heartbeat();
        self.reporter
            .emit(InternalPromptProgressEvent::Failed, Some(error));
    }

    pub fn stop_heartbeat(&mut self) {
        if let Some(sender) = self.heartbeat_stop.take() {
            let _ = sender.send(());
        }
        if let Some(handle) = self.heartbeat_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for InternalPromptProgressRun {
    fn drop(&mut self) {
        self.stop_heartbeat();
    }
}

pub fn format_internal_prompt_progress_line(
    event: InternalPromptProgressEvent,
    snapshot: &InternalPromptProgressState,
    elapsed: Duration,
    error: Option<&str>,
) -> String {
    let elapsed_seconds = elapsed.as_secs();
    let step_label = if snapshot.step == 0 {
        "current step pending".to_string()
    } else {
        format!("current step {}", snapshot.step)
    };
    let mut status_bits = vec![step_label, format!("phase {}", snapshot.phase)];
    if let Some(detail) = snapshot
        .detail
        .as_deref()
        .filter(|detail| !detail.is_empty())
    {
        status_bits.push(detail.to_string());
    }
    let status = status_bits.join(" · ");
    match event {
        InternalPromptProgressEvent::Started => {
            format!(
                "🧭 {} status · planning started · {status}",
                snapshot.command_label
            )
        }
        InternalPromptProgressEvent::Update => {
            format!("… {} status · {status}", snapshot.command_label)
        }
        InternalPromptProgressEvent::Heartbeat => format!(
            "… {} heartbeat · {elapsed_seconds}s elapsed · {status}",
            snapshot.command_label
        ),
        InternalPromptProgressEvent::Complete => format!(
            "✔ {} status · completed · {elapsed_seconds}s elapsed · {} steps total",
            snapshot.command_label, snapshot.step
        ),
        InternalPromptProgressEvent::Failed => format!(
            "✘ {} status · failed · {elapsed_seconds}s elapsed · {}",
            snapshot.command_label,
            error.unwrap_or("unknown error")
        ),
    }
}

pub fn describe_tool_progress(name: &str, input: &str) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(input).unwrap_or(serde_json::Value::String(input.to_string()));
    match name {
        "bash" | "Bash" => {
            let command = parsed
                .get("command")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if command.is_empty() {
                "running shell command".to_string()
            } else {
                format!("command {}", truncate_for_summary(command.trim(), 100))
            }
        }
        "read_file" | "Read" => format!("reading {}", extract_tool_path(&parsed)),
        "write_file" | "Write" => format!("writing {}", extract_tool_path(&parsed)),
        "edit_file" | "Edit" => format!("editing {}", extract_tool_path(&parsed)),
        "glob_search" | "Glob" => {
            let pattern = parsed
                .get("pattern")
                .and_then(|value| value.as_str())
                .unwrap_or("?");
            let scope = parsed
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or(".");
            format!("glob `{pattern}` in {scope}")
        }
        "grep_search" | "Grep" => {
            let pattern = parsed
                .get("pattern")
                .and_then(|value| value.as_str())
                .unwrap_or("?");
            let scope = parsed
                .get("path")
                .and_then(|value| value.as_str())
                .unwrap_or(".");
            format!("grep `{pattern}` in {scope}")
        }
        "web_search" | "WebSearch" => parsed
            .get("query")
            .and_then(|value| value.as_str())
            .map_or_else(
                || "running web search".to_string(),
                |query| format!("query {}", truncate_for_summary(query, 100)),
            ),
        _ => {
            let summary = summarize_tool_payload(input);
            if summary.is_empty() {
                format!("running {name}")
            } else {
                format!("{name}: {summary}")
            }
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::too_many_arguments)]
pub fn build_runtime(
    session: Session,
    session_id: &str,
    model: String,
    system_prompt: Vec<String>,
    enable_tools: bool,
    emit_output: bool,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
    progress_reporter: Option<InternalPromptProgressReporter>,
    usage_callback: Option<Arc<dyn Fn(TokenUsage) + Send + Sync>>,
) -> Result<BuiltRuntime, Box<dyn std::error::Error>> {
    let runtime_plugin_state = build_runtime_plugin_state()?;
    build_runtime_with_plugin_state(
        session,
        session_id,
        model,
        system_prompt,
        enable_tools,
        emit_output,
        allowed_tools,
        permission_mode,
        progress_reporter,
        usage_callback,
        runtime_plugin_state,
    )
}

#[allow(clippy::needless_pass_by_value)]
#[allow(clippy::too_many_arguments)]
pub fn build_runtime_with_plugin_state(
    mut session: Session,
    session_id: &str,
    model: String,
    system_prompt: Vec<String>,
    enable_tools: bool,
    emit_output: bool,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
    progress_reporter: Option<InternalPromptProgressReporter>,
    usage_callback: Option<Arc<dyn Fn(TokenUsage) + Send + Sync>>,
    runtime_plugin_state: RuntimePluginState,
) -> Result<BuiltRuntime, Box<dyn std::error::Error>> {
    // Persist the model in session metadata so resumed sessions can report it.
    if session.model.is_none() {
        session.model = Some(model.clone());
    }
    let RuntimePluginState {
        feature_config,
        tool_registry,
        plugin_registry,
        mcp_state,
    } = runtime_plugin_state;
    plugin_registry.initialize()?;
    let policy = permission_policy(permission_mode, &feature_config, &tool_registry)
        .map_err(std::io::Error::other)?;
    let mut runtime = ConversationRuntime::new_with_features(
        session,
        AnthropicRuntimeClient::new(
            session_id,
            model,
            enable_tools,
            emit_output,
            allowed_tools.clone(),
            tool_registry.clone(),
            progress_reporter,
            usage_callback,
        )?,
        CliToolExecutor::new(
            allowed_tools.clone(),
            emit_output,
            tool_registry.clone(),
            mcp_state.clone(),
        ),
        policy,
        system_prompt,
        &feature_config,
    );
    if emit_output {
        runtime = runtime.with_hook_progress_reporter(Box::new(CliHookProgressReporter));
    }
    let theme = feature_config.theme().unwrap_or("space").to_string();
    Ok(BuiltRuntime::new(runtime, plugin_registry, mcp_state, theme))
}

pub struct CliHookProgressReporter;

impl runtime::HookProgressReporter for CliHookProgressReporter {
    fn on_event(&mut self, event: &runtime::HookProgressEvent) {
        match event {
            runtime::HookProgressEvent::Started {
                event,
                tool_name,
                command,
            } => eprintln!(
                "[hook {event_name}] {tool_name}: {command}",
                event_name = event.as_str()
            ),
            runtime::HookProgressEvent::Completed {
                event,
                tool_name,
                command,
            } => eprintln!(
                "[hook done {event_name}] {tool_name}: {command}",
                event_name = event.as_str()
            ),
            runtime::HookProgressEvent::Cancelled {
                event,
                tool_name,
                command,
            } => eprintln!(
                "[hook cancelled {event_name}] {tool_name}: {command}",
                event_name = event.as_str()
            ),
        }
    }
}

pub struct CliPermissionPrompter {
    pub current_mode: PermissionMode,
}

impl CliPermissionPrompter {
    pub fn new(current_mode: PermissionMode) -> Self {
        Self { current_mode }
    }
}

impl runtime::PermissionPrompter for CliPermissionPrompter {
    fn decide(
        &mut self,
        request: &runtime::PermissionRequest,
    ) -> runtime::PermissionPromptDecision {
        println!();
        println!("{}", format_permission_prompt(request, self.current_mode));
        print!("Approve this tool call? [y/N]: ");
        let _ = io::stdout().flush();

        let mut response = String::new();
        match io::stdin().read_line(&mut response) {
            Ok(_) => {
                let normalized = response.trim().to_ascii_lowercase();
                if matches!(normalized.as_str(), "y" | "yes") {
                    runtime::PermissionPromptDecision::Allow
                } else {
                    runtime::PermissionPromptDecision::Deny {
                        reason: format!(
                            "tool '{}' denied by user approval prompt",
                            request.tool_name
                        ),
                    }
                }
            }
            Err(error) => runtime::PermissionPromptDecision::Deny {
                reason: format!("permission approval failed: {error}"),
            },
        }
    }
}

pub fn format_permission_prompt(
    request: &runtime::PermissionRequest,
    current_mode: PermissionMode,
) -> String {
    let reason = request
        .reason
        .as_deref()
        .unwrap_or("tool requires approval");
    let input = truncate_for_summary(&request.input.replace('\n', " "), 220);
    format!(
        "\x1b[38;5;245m╭─ \x1b[1;33mPermission approval required\x1b[0;38;5;245m ─╮\x1b[0m\n\
         \x1b[38;5;245m│\x1b[0m Tool             \x1b[1;36m{}\x1b[0m\n\
         \x1b[38;5;245m│\x1b[0m Current mode     {}\n\
         \x1b[38;5;245m│\x1b[0m Required mode    \x1b[1;33m{}\x1b[0m\n\
         \x1b[38;5;245m│\x1b[0m Reason           {}\n\
         \x1b[38;5;245m│\x1b[0m Input            {}\n\
         \x1b[38;5;245m╰────────────────────────────────────╯\x1b[0m",
        request.tool_name,
        current_mode.as_str(),
        request.required_mode.as_str(),
        reason,
        input,
    )
}

// NOTE: Despite the historical name `AnthropicRuntimeClient`, this struct
// now holds an `ApiProviderClient` which dispatches to Anthropic, xAI,
// OpenAI, or DashScope at construction time based on
// `detect_provider_kind(&model)`. The struct name is kept to avoid
// churning `BuiltRuntime` and every Deref/DerefMut site that references
// it. See ROADMAP #29 for the provider-dispatch routing fix.
pub struct AnthropicRuntimeClient {
    pub runtime: tokio::runtime::Runtime,
    pub client: ApiProviderClient,
    pub session_id: String,
    pub model: String,
    pub enable_tools: bool,
    pub emit_output: bool,
    pub allowed_tools: Option<AllowedToolSet>,
    pub tool_registry: GlobalToolRegistry,
    pub progress_reporter: Option<InternalPromptProgressReporter>,
    pub reasoning_effort: Option<String>,
    pub usage_callback: Option<Arc<dyn Fn(TokenUsage) + Send + Sync>>,
}

impl AnthropicRuntimeClient {
    pub fn new(
        session_id: &str,
        model: String,
        enable_tools: bool,
        emit_output: bool,
        allowed_tools: Option<AllowedToolSet>,
        tool_registry: GlobalToolRegistry,
        progress_reporter: Option<InternalPromptProgressReporter>,
        usage_callback: Option<Arc<dyn Fn(TokenUsage) + Send + Sync>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Dispatch to the correct provider at construction time.
        // `ApiProviderClient` (exposed by the api crate as
        // `ProviderClient`) is an enum over Anthropic / xAI / OpenAI
        // variants, where xAI and OpenAI both use the OpenAI-compat
        // wire format under the hood. We consult
        // `detect_provider_kind(&resolved_model)` so model-name prefix
        // routing (`openai/`, `gpt-`, `grok`, `qwen/`) wins over
        // env-var presence.
        //
        // For Anthropic we build the client directly instead of going
        // through `ApiProviderClient::from_model_with_anthropic_auth`
        // so we can explicitly apply `api::read_base_url()` — that
        // reads `ANTHROPIC_BASE_URL` and is required for the local
        // mock-server test harness
        // (`crates/rusty-claude-cli/tests/compact_output.rs`) to point
        // claw at its fake Anthropic endpoint. We also attach a
        // session-scoped prompt cache on the Anthropic path; the
        // prompt cache is Anthropic-only so non-Anthropic variants
        // skip it.
        let resolved_model = api::resolve_model_alias(&model);
        let client = match detect_provider_kind(&resolved_model) {
            ProviderKind::Anthropic => {
                let auth = resolve_cli_auth_source()?;
                let inner = AnthropicClient::from_auth(auth)
                    .with_base_url(api::read_base_url())
                    .with_prompt_cache(PromptCache::new(session_id));
                ApiProviderClient::Anthropic(inner)
            }
            ProviderKind::Xai | ProviderKind::OpenAi => {
                // The api crate's `ProviderClient::from_model_with_anthropic_auth`
                // with `None` for the anthropic auth routes via
                // `detect_provider_kind` and builds an
                // `OpenAiCompatClient::from_env` with the matching
                // `OpenAiCompatConfig` (openai / xai / dashscope).
                // That reads the correct API-key env var and BASE_URL
                // override internally, so this one call covers OpenAI,
                // OpenRouter, xAI, DashScope, Ollama, and any other
                // OpenAI-compat endpoint users configure via
                // `OPENAI_BASE_URL` / `XAI_BASE_URL` / `DASHSCOPE_BASE_URL`.
                ApiProviderClient::from_model_with_anthropic_auth(&resolved_model, None)?
            }
        };
        Ok(Self {
            runtime: tokio::runtime::Runtime::new()?,
            client,
            session_id: session_id.to_string(),
            model,
            enable_tools,
            emit_output,
            allowed_tools,
            tool_registry,
            progress_reporter,
            reasoning_effort: None,
            usage_callback,
        })
    }

    pub fn set_reasoning_effort(&mut self, effort: Option<String>) {
        self.reasoning_effort = effort;
    }
}

pub fn resolve_cli_auth_source() -> Result<AuthSource, Box<dyn std::error::Error>> {
    Ok(resolve_cli_auth_source_for_cwd()?)
}

pub fn resolve_cli_auth_source_for_cwd() -> Result<AuthSource, api::ApiError> {
    resolve_startup_auth_source(|| Ok(None))
}

impl ApiClient for AnthropicRuntimeClient {
    #[allow(clippy::too_many_lines)]
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        if let Some(progress_reporter) = &self.progress_reporter {
            progress_reporter.mark_model_phase();
        }
        let is_post_tool = request_ends_with_tool_result(&request);
        let message_request = MessageRequest {
            model: self.model.clone(),
            max_tokens: max_tokens_for_model(&self.model),
            messages: convert_messages(&request.messages),
            system: (!request.system_prompt.is_empty()).then(|| request.system_prompt.join("\n\n")),
            tools: self
                .enable_tools
                .then(|| filter_tool_specs(&self.tool_registry, self.allowed_tools.as_ref())),
            tool_choice: self.enable_tools.then_some(ToolChoice::Auto),
            stream: true,
            reasoning_effort: self.reasoning_effort.clone(),
            ..Default::default()
        };

        self.runtime.block_on(async {
            // When resuming after tool execution, apply a stall timeout on the
            // first stream event.  If the model does not respond within the
            // deadline we drop the stalled connection and re-send the request as
            // a continuation nudge (one retry only).
            let max_attempts: usize = if is_post_tool { 2 } else { 1 };

            for attempt in 1..=max_attempts {
                let result = self
                    .consume_stream(&message_request, is_post_tool && attempt == 1)
                    .await;
                match result {
                    Ok(events) => return Ok(events),
                    Err(error)
                        if error.to_string().contains("post-tool stall")
                            && attempt < max_attempts =>
                    {
                        // Stalled after tool completion — nudge the model by
                        // re-sending the same request.
                    }
                    Err(error) => return Err(error),
                }
            }

            Err(RuntimeError::new("post-tool continuation nudge exhausted"))
        })
    }
}

impl AnthropicRuntimeClient {
    /// Consume a single streaming response, optionally applying a stall
    /// timeout on the first event for post-tool continuations.
    #[allow(clippy::too_many_lines)]
    async fn consume_stream(
        &self,
        message_request: &MessageRequest,
        apply_stall_timeout: bool,
    ) -> Result<Vec<AssistantEvent>, RuntimeError> {
        let mut stream = self
            .client
            .stream_message(message_request)
            .await
            .map_err(|error| {
                RuntimeError::new(format_user_visible_api_error(&self.session_id, &error))
            })?;
        let mut stdout = io::stdout();
        let mut sink = io::sink();
        let out: &mut dyn Write = if self.emit_output {
            &mut stdout
        } else {
            &mut sink
        };
        let renderer = TerminalRenderer::new();
        let mut markdown_stream = MarkdownStreamState::default();
        let mut events = Vec::new();
        let mut pending_tool: Option<(String, String, String)> = None;
        let mut block_has_thinking_summary = false;
        let mut saw_stop = false;
        let mut received_any_event = false;

        loop {
            let next = if apply_stall_timeout && !received_any_event {
                match tokio::time::timeout(POST_TOOL_STALL_TIMEOUT, stream.next_event()).await {
                    Ok(inner) => inner.map_err(|error| {
                        RuntimeError::new(format_user_visible_api_error(&self.session_id, &error))
                    })?,
                    Err(_elapsed) => {
                        return Err(RuntimeError::new(
                            "post-tool stall: model did not respond within timeout",
                        ));
                    }
                }
            } else {
                stream.next_event().await.map_err(|error| {
                    RuntimeError::new(format_user_visible_api_error(&self.session_id, &error))
                })?
            };

            let Some(event) = next else {
                break;
            };
            received_any_event = true;

            match event {
                ApiStreamEvent::MessageStart(start) => {
                    for block in start.message.content {
                        push_output_block(
                            block,
                            out,
                            &mut events,
                            &mut pending_tool,
                            true,
                            &mut block_has_thinking_summary,
                        )?;
                    }
                }
                ApiStreamEvent::ContentBlockStart(start) => {
                    push_output_block(
                        start.content_block,
                        out,
                        &mut events,
                        &mut pending_tool,
                        true,
                        &mut block_has_thinking_summary,
                    )?;
                }
                ApiStreamEvent::ContentBlockDelta(delta) => match delta.delta {
                    ContentBlockDelta::TextDelta { text } => {
                        if !text.is_empty() {
                            if let Some(progress_reporter) = &self.progress_reporter {
                                progress_reporter.mark_text_phase(&text);
                            }
                            if let Some(rendered) = markdown_stream.push(&renderer, &text) {
                                write!(out, "{rendered}")
                                    .and_then(|()| out.flush())
                                    .map_err(|error| RuntimeError::new(error.to_string()))?;
                            }
                            events.push(AssistantEvent::TextDelta(text));
                        }
                    }
                    ContentBlockDelta::InputJsonDelta { partial_json } => {
                        if let Some((_, _, input)) = &mut pending_tool {
                            input.push_str(&partial_json);
                        }
                    }
                    ContentBlockDelta::ThinkingDelta { .. } => {
                        if !block_has_thinking_summary {
                            render_thinking_block_summary(out, None, false)?;
                            block_has_thinking_summary = true;
                        }
                    }
                    ContentBlockDelta::SignatureDelta { .. } => {}
                },
                ApiStreamEvent::ContentBlockStop(_) => {
                    block_has_thinking_summary = false;
                    if let Some(rendered) = markdown_stream.flush(&renderer) {
                        write!(out, "{rendered}")
                            .and_then(|()| out.flush())
                            .map_err(|error| RuntimeError::new(error.to_string()))?;
                    }
                    if let Some((id, name, input)) = pending_tool.take() {
                        if let Some(progress_reporter) = &self.progress_reporter {
                            progress_reporter.mark_tool_phase(&name, &input);
                        }
                        // Display tool call now that input is fully accumulated
                        writeln!(out, "\n{}", format_tool_call_start(&name, &input))
                            .and_then(|()| out.flush())
                            .map_err(|error| RuntimeError::new(error.to_string()))?;
                        events.push(AssistantEvent::ToolUse { id, name, input });
                    }
                }
                ApiStreamEvent::MessageDelta(delta) => {
                    let usage = delta.usage.token_usage();
                    if let Some(callback) = &self.usage_callback {
                        callback(usage);
                    }
                    events.push(AssistantEvent::Usage(usage));
                }
                ApiStreamEvent::MessageStop(_) => {
                    saw_stop = true;
                    if let Some(rendered) = markdown_stream.flush(&renderer) {
                        write!(out, "{rendered}")
                            .and_then(|()| out.flush())
                            .map_err(|error| RuntimeError::new(error.to_string()))?;
                    }
                    events.push(AssistantEvent::MessageStop);
                }
            }
        }

        push_prompt_cache_record(&self.client, &mut events);

        if !saw_stop
            && events.iter().any(|event| {
                matches!(event, AssistantEvent::TextDelta(text) if !text.is_empty())
                    || matches!(event, AssistantEvent::ToolUse { .. })
            })
        {
            events.push(AssistantEvent::MessageStop);
        }

        if events
            .iter()
            .any(|event| matches!(event, AssistantEvent::MessageStop))
        {
            return Ok(events);
        }

        let response = self
            .client
            .send_message(&MessageRequest {
                stream: false,
                ..message_request.clone()
            })
            .await
            .map_err(|error| {
                RuntimeError::new(format_user_visible_api_error(&self.session_id, &error))
            })?;
        let mut events = response_to_events(response, out)?;
        push_prompt_cache_record(&self.client, &mut events);
        Ok(events)
    }
}

/// Returns `true` when the conversation ends with a tool-result message,
/// meaning the model is expected to continue after tool execution.
pub fn request_ends_with_tool_result(request: &ApiRequest) -> bool {
    request
        .messages
        .last()
        .is_some_and(|message| message.role == MessageRole::Tool)
}

pub fn format_user_visible_api_error(session_id: &str, error: &api::ApiError) -> String {
    if error.is_context_window_failure() {
        format_context_window_blocked_error(session_id, error)
    } else if error.is_generic_fatal_wrapper() {
        let mut qualifiers = vec![format!("session {session_id}")];
        if let Some(request_id) = error.request_id() {
            qualifiers.push(format!("trace {request_id}"));
        }
        format!(
            "{} ({}): {}",
            error.safe_failure_class(),
            qualifiers.join(", "),
            error
        )
    } else {
        error.to_string()
    }
}

pub fn format_context_window_blocked_error(session_id: &str, error: &api::ApiError) -> String {
    let mut lines = vec![
        "Context window blocked".to_string(),
        "  Failure class    context_window_blocked".to_string(),
        format!("  Session          {session_id}"),
    ];

    if let Some(request_id) = error.request_id() {
        lines.push(format!("  Trace            {request_id}"));
    }

    match error {
        api::ApiError::ContextWindowExceeded {
            model,
            estimated_input_tokens,
            requested_output_tokens,
            estimated_total_tokens,
            context_window_tokens,
        } => {
            lines.push(format!("  Model            {model}"));
            lines.push(format!(
                "  Input estimate   ~{estimated_input_tokens} tokens (heuristic)"
            ));
            lines.push(format!(
                "  Requested output {requested_output_tokens} tokens"
            ));
            lines.push(format!(
                "  Total estimate   ~{estimated_total_tokens} tokens (heuristic)"
            ));
            lines.push(format!("  Context window   {context_window_tokens} tokens"));
        }
        api::ApiError::Api { message, body, .. } => {
            let detail = message.as_deref().unwrap_or(body).trim();
            if !detail.is_empty() {
                lines.push(format!(
                    "  Detail           {}",
                    truncate_for_summary(detail, 120)
                ));
            }
        }
        api::ApiError::RetriesExhausted { last_error, .. } => {
            let detail = match last_error.as_ref() {
                api::ApiError::Api { message, body, .. } => message.as_deref().unwrap_or(body),
                other => return format_context_window_blocked_error(session_id, other),
            }
            .trim();
            if !detail.is_empty() {
                lines.push(format!(
                    "  Detail           {}",
                    truncate_for_summary(detail, 120)
                ));
            }
        }
        _ => {}
    }

    lines.push(String::new());
    lines.push("Recovery".to_string());
    lines.push("  Compact          /compact".to_string());
    lines.push(format!(
        "  Resume compact   claw --resume {session_id} /compact"
    ));
    lines.push("  Fresh session    /clear --confirm".to_string());
    lines.push(
        "  Reduce scope     remove large pasted context/files or ask for a smaller slice"
            .to_string(),
    );
    lines.push("  Retry            rerun after compacting or reducing the request".to_string());

    lines.join("\n")
}

pub fn final_assistant_text(summary: &runtime::TurnSummary) -> String {
    summary
        .assistant_messages
        .last()
        .map(|message| {
            message
                .blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

pub fn collect_tool_uses(summary: &runtime::TurnSummary) -> Vec<serde_json::Value> {
    summary
        .assistant_messages
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some(json!({
                "id": id,
                "name": name,
                "input": input,
            })),
            _ => None,
        })
        .collect()
}

pub fn collect_tool_results(summary: &runtime::TurnSummary) -> Vec<serde_json::Value> {
    summary
        .tool_results
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                tool_name,
                output,
                is_error,
            } => Some(json!({
                "tool_use_id": tool_use_id,
                "tool_name": tool_name,
                "output": output,
                "is_error": is_error,
            })),
            _ => None,
        })
        .collect()
}

pub fn format_tool_timeline(summary: &runtime::TurnSummary, elapsed: Duration) -> Option<String> {
    let tool_uses = collect_tool_uses(summary);
    if tool_uses.is_empty() {
        return None;
    }
    let tool_results = collect_tool_results(summary);
    let segments = tool_uses
        .iter()
        .map(|tool_use| {
            let id = tool_use
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let name = tool_use
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool");
            let status = tool_results
                .iter()
                .find(|result| {
                    result
                        .get("tool_use_id")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|tool_use_id| tool_use_id == id)
                })
                .map(|result| {
                    if result
                        .get("is_error")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                    {
                        "error"
                    } else {
                        "ok"
                    }
                })
                .unwrap_or("pending");
            format!("{name} -> {status}")
        })
        .collect::<Vec<_>>();
    Some(format!(
        "\x1b[2mTool timeline  {} ({} tools, {})\x1b[0m",
        segments.join(" | "),
        segments.len(),
        format_duration_compact(elapsed)
    ))
}

pub fn collect_prompt_cache_events(summary: &runtime::TurnSummary) -> Vec<serde_json::Value> {
    summary
        .prompt_cache_events
        .iter()
        .map(|event| {
            json!({
                "unexpected": event.unexpected,
                "reason": event.reason,
                "previous_cache_read_input_tokens": event.previous_cache_read_input_tokens,
                "current_cache_read_input_tokens": event.current_cache_read_input_tokens,
                "token_drop": event.token_drop,
            })
        })
        .collect()
}

pub fn slash_command_completion_candidates_with_sessions(
    model: &str,
    active_session_id: Option<&str>,
    recent_session_ids: Vec<String>,
) -> Vec<String> {
    let mut completions = BTreeSet::new();

    for spec in slash_command_specs() {
        if STUB_COMMANDS.contains(&spec.name) {
            continue;
        }
        completions.insert(format!("/{}", spec.name));
        for alias in spec.aliases {
            if !STUB_COMMANDS.contains(alias) {
                completions.insert(format!("/{alias}"));
            }
        }
    }

    for candidate in [
        "/bughunter ",
        "/clear --confirm",
        "/config ",
        "/config env",
        "/config hooks",
        "/config model",
        "/config plugins",
        "/mcp ",
        "/mcp list",
        "/mcp show ",
        "/export ",
        "/issue ",
        "/model ",
        "/model opus",
        "/model sonnet",
        "/model haiku",
        "/permissions ",
        "/permissions read-only",
        "/permissions workspace-write",
        "/permissions danger-full-access",
        "/plugin list",
        "/plugin install ",
        "/plugin enable ",
        "/plugin disable ",
        "/plugin uninstall ",
        "/plugin update ",
        "/plugins list",
        "/pr ",
        "/resume ",
        "/session list",
        "/session switch ",
        "/session fork ",
        "/teleport ",
        "/ultraplan ",
        "/agents help",
        "/mcp help",
        "/skills help",
    ] {
        completions.insert(candidate.to_string());
    }

    if !model.trim().is_empty() {
        completions.insert(format!("/model {}", resolve_model_alias(model)));
        completions.insert(format!("/model {model}"));
    }

    if let Some(active_session_id) = active_session_id.filter(|value| !value.trim().is_empty()) {
        completions.insert(format!("/resume {active_session_id}"));
        completions.insert(format!("/session switch {active_session_id}"));
    }

    for session_id in recent_session_ids
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .take(10)
    {
        completions.insert(format!("/resume {session_id}"));
        completions.insert(format!("/session switch {session_id}"));
    }

    completions.into_iter().collect()
}

pub fn format_tool_call_start(name: &str, input: &str) -> String {
    let parsed: serde_json::Value =
        serde_json::from_str(input).unwrap_or(serde_json::Value::String(input.to_string()));

    let detail = match name {
        "bash" | "Bash" => format_bash_call(&parsed),
        "read_file" | "Read" => {
            let path = extract_tool_path(&parsed);
            format!("\x1b[2m📄 Reading {path}…\x1b[0m")
        }
        "write_file" | "Write" => {
            let path = extract_tool_path(&parsed);
            let lines = parsed
                .get("content")
                .and_then(|value| value.as_str())
                .map_or(0, |content| content.lines().count());
            format!("\x1b[1;32m✏️ Writing {path}\x1b[0m \x1b[2m({lines} lines)\x1b[0m")
        }
        "edit_file" | "Edit" => {
            let path = extract_tool_path(&parsed);
            let old_value = parsed
                .get("old_string")
                .or_else(|| parsed.get("oldString"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let new_value = parsed
                .get("new_string")
                .or_else(|| parsed.get("newString"))
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            format!(
                "\x1b[1;33m📝 Editing {path}\x1b[0m{}",
                format_patch_preview(old_value, new_value)
                    .map(|preview| format!("\n{preview}"))
                    .unwrap_or_default()
            )
        }
        "glob_search" | "Glob" => format_search_start("🔎 Glob", &parsed),
        "grep_search" | "Grep" => format_search_start("🔎 Grep", &parsed),
        "web_search" | "WebSearch" => parsed
            .get("query")
            .and_then(|value| value.as_str())
            .unwrap_or("?")
            .to_string(),
        _ => summarize_tool_payload(input),
    };

    let border = "─".repeat(name.len() + 8);
    format!(
        "\x1b[38;5;245m╭─ \x1b[1;36m{name}\x1b[0;38;5;245m ─╮\x1b[0m\n\x1b[38;5;245m│\x1b[0m {detail}\n\x1b[38;5;245m╰{border}╯\x1b[0m"
    )
}

pub fn format_tool_result(name: &str, output: &str, is_error: bool) -> String {
    let icon = if is_error {
        "\x1b[1;31m✗\x1b[0m"
    } else {
        "\x1b[1;32m✓\x1b[0m"
    };
    if is_error {
        let summary = truncate_for_summary(output.trim(), 160);
        return if summary.is_empty() {
            format!("{icon} \x1b[38;5;245m{name}\x1b[0m")
        } else {
            format!("{icon} \x1b[38;5;245m{name}\x1b[0m\n\x1b[38;5;203m{summary}\x1b[0m")
        };
    }

    let parsed: serde_json::Value =
        serde_json::from_str(output).unwrap_or(serde_json::Value::String(output.to_string()));
    match name {
        "bash" | "Bash" => format_bash_result(icon, &parsed),
        "read_file" | "Read" => format_read_result(icon, &parsed),
        "write_file" | "Write" => format_write_result(icon, &parsed),
        "edit_file" | "Edit" => format_edit_result(icon, &parsed),
        "glob_search" | "Glob" => format_glob_result(icon, &parsed),
        "grep_search" | "Grep" => format_grep_result(icon, &parsed),
        _ => format_generic_tool_result(icon, name, &parsed),
    }
}

pub const DISPLAY_TRUNCATION_NOTICE: &str =
    "\x1b[2m… output truncated for display; full result preserved in session.\x1b[0m";
pub const READ_DISPLAY_MAX_LINES: usize = 80;
pub const READ_DISPLAY_MAX_CHARS: usize = 6_000;
pub const TOOL_OUTPUT_DISPLAY_MAX_LINES: usize = 60;
pub const TOOL_OUTPUT_DISPLAY_MAX_CHARS: usize = 4_000;

pub fn extract_tool_path(parsed: &serde_json::Value) -> String {
    parsed
        .get("file_path")
        .or_else(|| parsed.get("filePath"))
        .or_else(|| parsed.get("path"))
        .and_then(|value| value.as_str())
        .unwrap_or("?")
        .to_string()
}

pub fn format_search_start(label: &str, parsed: &serde_json::Value) -> String {
    let pattern = parsed
        .get("pattern")
        .and_then(|value| value.as_str())
        .unwrap_or("?");
    let scope = parsed
        .get("path")
        .and_then(|value| value.as_str())
        .unwrap_or(".");
    format!("{label} {pattern}\n\x1b[2min {scope}\x1b[0m")
}

pub fn format_patch_preview(old_value: &str, new_value: &str) -> Option<String> {
    if old_value.is_empty() && new_value.is_empty() {
        return None;
    }
    Some(format!(
        "\x1b[38;5;203m- {}\x1b[0m\n\x1b[38;5;70m+ {}\x1b[0m",
        truncate_for_summary(first_visible_line(old_value), 72),
        truncate_for_summary(first_visible_line(new_value), 72)
    ))
}

pub fn format_bash_call(parsed: &serde_json::Value) -> String {
    let command = parsed
        .get("command")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if command.is_empty() {
        String::new()
    } else {
        format!(
            "\x1b[48;5;236;38;5;255m $ {} \x1b[0m",
            truncate_for_summary(command, 160)
        )
    }
}

pub fn first_visible_line(text: &str) -> &str {
    text.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(text)
}

pub fn format_bash_result(icon: &str, parsed: &serde_json::Value) -> String {
    use std::fmt::Write as _;

    let mut lines = vec![format!("{icon} \x1b[38;5;245mbash\x1b[0m")];
    if let Some(task_id) = parsed
        .get("backgroundTaskId")
        .and_then(|value| value.as_str())
    {
        write!(&mut lines[0], " backgrounded ({task_id})").expect("write to string");
    } else if let Some(status) = parsed
        .get("returnCodeInterpretation")
        .and_then(|value| value.as_str())
        .filter(|status| !status.is_empty())
    {
        write!(&mut lines[0], " {status}").expect("write to string");
    }

    if let Some(stdout) = parsed.get("stdout").and_then(|value| value.as_str()) {
        if !stdout.trim().is_empty() {
            lines.push(truncate_output_for_display(
                stdout,
                TOOL_OUTPUT_DISPLAY_MAX_LINES,
                TOOL_OUTPUT_DISPLAY_MAX_CHARS,
            ));
        }
    }
    if let Some(stderr) = parsed.get("stderr").and_then(|value| value.as_str()) {
        if !stderr.trim().is_empty() {
            lines.push(format!(
                "\x1b[38;5;203m{}\x1b[0m",
                truncate_output_for_display(
                    stderr,
                    TOOL_OUTPUT_DISPLAY_MAX_LINES,
                    TOOL_OUTPUT_DISPLAY_MAX_CHARS,
                )
            ));
        }
    }

    lines.join("\n\n")
}

pub fn format_read_result(icon: &str, parsed: &serde_json::Value) -> String {
    let file = parsed.get("file").unwrap_or(parsed);
    let path = extract_tool_path(file);
    let start_line = file
        .get("startLine")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(1);
    let num_lines = file
        .get("numLines")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_lines = file
        .get("totalLines")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(num_lines);
    let content = file
        .get("content")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let end_line = start_line.saturating_add(num_lines.saturating_sub(1));

    format!(
        "{icon} \x1b[2m📄 Read {path} (lines {}-{} of {})\x1b[0m\n{}",
        start_line,
        end_line.max(start_line),
        total_lines,
        truncate_output_for_display(content, READ_DISPLAY_MAX_LINES, READ_DISPLAY_MAX_CHARS)
    )
}

pub fn format_write_result(icon: &str, parsed: &serde_json::Value) -> String {
    let path = extract_tool_path(parsed);
    let kind = parsed
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("write");
    let line_count = parsed
        .get("content")
        .and_then(|value| value.as_str())
        .map_or(0, |content| content.lines().count());
    format!(
        "{icon} \x1b[1;32m✏️ {} {path}\x1b[0m \x1b[2m({line_count} lines)\x1b[0m",
        if kind == "create" { "Wrote" } else { "Updated" },
    )
}

pub fn format_structured_patch_preview(parsed: &serde_json::Value) -> Option<String> {
    let hunks = parsed.get("structuredPatch")?.as_array()?;
    let mut preview = Vec::new();
    for hunk in hunks.iter().take(2) {
        let lines = hunk.get("lines")?.as_array()?;
        for line in lines.iter().filter_map(|value| value.as_str()).take(6) {
            match line.chars().next() {
                Some('+') => preview.push(format!("\x1b[38;5;70m{line}\x1b[0m")),
                Some('-') => preview.push(format!("\x1b[38;5;203m{line}\x1b[0m")),
                _ => preview.push(line.to_string()),
            }
        }
    }
    if preview.is_empty() {
        None
    } else {
        Some(preview.join("\n"))
    }
}

pub fn format_edit_result(icon: &str, parsed: &serde_json::Value) -> String {
    let path = extract_tool_path(parsed);
    let suffix = if parsed
        .get("replaceAll")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        " (replace all)"
    } else {
        ""
    };
    let preview = format_structured_patch_preview(parsed).or_else(|| {
        let old_value = parsed
            .get("oldString")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let new_value = parsed
            .get("newString")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        format_patch_preview(old_value, new_value)
    });

    match preview {
        Some(preview) => format!("{icon} \x1b[1;33m📝 Edited {path}{suffix}\x1b[0m\n{preview}"),
        None => format!("{icon} \x1b[1;33m📝 Edited {path}{suffix}\x1b[0m"),
    }
}

pub fn format_glob_result(icon: &str, parsed: &serde_json::Value) -> String {
    let num_files = parsed
        .get("numFiles")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let filenames = parsed
        .get("filenames")
        .and_then(|value| value.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|value| value.as_str())
                .take(8)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    if filenames.is_empty() {
        format!("{icon} \x1b[38;5;245mglob_search\x1b[0m matched {num_files} files")
    } else {
        format!("{icon} \x1b[38;5;245mglob_search\x1b[0m matched {num_files} files\n{filenames}")
    }
}

pub fn format_grep_result(icon: &str, parsed: &serde_json::Value) -> String {
    let num_matches = parsed
        .get("numMatches")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let num_files = parsed
        .get("numFiles")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let content = parsed
        .get("content")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let filenames = parsed
        .get("filenames")
        .and_then(|value| value.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|value| value.as_str())
                .take(8)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let summary = format!(
        "{icon} \x1b[38;5;245mgrep_search\x1b[0m {num_matches} matches across {num_files} files"
    );
    if !content.trim().is_empty() {
        format!(
            "{summary}\n{}",
            truncate_output_for_display(
                content,
                TOOL_OUTPUT_DISPLAY_MAX_LINES,
                TOOL_OUTPUT_DISPLAY_MAX_CHARS,
            )
        )
    } else if !filenames.is_empty() {
        format!("{summary}\n{filenames}")
    } else {
        summary
    }
}

pub fn format_generic_tool_result(icon: &str, name: &str, parsed: &serde_json::Value) -> String {
    let rendered_output = match parsed {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            serde_json::to_string_pretty(parsed).unwrap_or_else(|_| parsed.to_string())
        }
        _ => parsed.to_string(),
    };
    let preview = truncate_output_for_display(
        &rendered_output,
        TOOL_OUTPUT_DISPLAY_MAX_LINES,
        TOOL_OUTPUT_DISPLAY_MAX_CHARS,
    );

    if preview.is_empty() {
        format!("{icon} \x1b[38;5;245m{name}\x1b[0m")
    } else if preview.contains('\n') {
        format!("{icon} \x1b[38;5;245m{name}\x1b[0m\n{preview}")
    } else {
        format!("{icon} \x1b[38;5;245m{name}:\x1b[0m {preview}")
    }
}

pub fn summarize_tool_payload(payload: &str) -> String {
    let compact = match serde_json::from_str::<serde_json::Value>(payload) {
        Ok(value) => value.to_string(),
        Err(_) => payload.trim().to_string(),
    };
    truncate_for_summary(&compact, 96)
}

pub fn truncate_for_summary(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

pub fn truncate_output_for_display(content: &str, max_lines: usize, max_chars: usize) -> String {
    let original = content.trim_end_matches('\n');
    if original.is_empty() {
        return String::new();
    }

    let mut preview_lines = Vec::new();
    let mut used_chars = 0usize;
    let mut truncated = false;

    for (index, line) in original.lines().enumerate() {
        if index >= max_lines {
            truncated = true;
            break;
        }

        let newline_cost = usize::from(!preview_lines.is_empty());
        let available = max_chars.saturating_sub(used_chars + newline_cost);
        if available == 0 {
            truncated = true;
            break;
        }

        let line_chars = line.chars().count();
        if line_chars > available {
            preview_lines.push(line.chars().take(available).collect::<String>());
            truncated = true;
            break;
        }

        preview_lines.push(line.to_string());
        used_chars += newline_cost + line_chars;
    }

    let mut preview = preview_lines.join("\n");
    if truncated {
        if !preview.is_empty() {
            preview.push('\n');
        }
        preview.push_str(DISPLAY_TRUNCATION_NOTICE);
    }
    preview
}

pub fn render_thinking_block_summary(
    out: &mut (impl Write + ?Sized),
    char_count: Option<usize>,
    redacted: bool,
) -> Result<(), RuntimeError> {
    let summary = if redacted {
        "\n▶ Thinking block hidden by provider\n".to_string()
    } else if let Some(char_count) = char_count {
        format!("\n▶ Thinking ({char_count} chars hidden)\n")
    } else {
        "\n▶ Thinking hidden\n".to_string()
    };
    write!(out, "{summary}")
        .and_then(|()| out.flush())
        .map_err(|error| RuntimeError::new(error.to_string()))
}

pub fn push_output_block(
    block: OutputContentBlock,
    out: &mut (impl Write + ?Sized),
    events: &mut Vec<AssistantEvent>,
    pending_tool: &mut Option<(String, String, String)>,
    streaming_tool_input: bool,
    block_has_thinking_summary: &mut bool,
) -> Result<(), RuntimeError> {
    match block {
        OutputContentBlock::Text { text } => {
            if !text.is_empty() {
                let rendered = TerminalRenderer::new().markdown_to_ansi(&text);
                write!(out, "{rendered}")
                    .and_then(|()| out.flush())
                    .map_err(|error| RuntimeError::new(error.to_string()))?;
                events.push(AssistantEvent::TextDelta(text));
            }
        }
        OutputContentBlock::ToolUse { id, name, input } => {
            // During streaming, the initial content_block_start has an empty input ({}).
            // The real input arrives via input_json_delta events. In
            // non-streaming responses, preserve a legitimate empty object.
            let initial_input = if streaming_tool_input
                && input.is_object()
                && input.as_object().is_some_and(serde_json::Map::is_empty)
            {
                String::new()
            } else {
                input.to_string()
            };
            *pending_tool = Some((id, name, initial_input));
        }
        OutputContentBlock::Thinking { thinking, .. } => {
            render_thinking_block_summary(out, Some(thinking.chars().count()), false)?;
            *block_has_thinking_summary = true;
        }
        OutputContentBlock::RedactedThinking { .. } => {
            render_thinking_block_summary(out, None, true)?;
            *block_has_thinking_summary = true;
        }
    }
    Ok(())
}

pub fn response_to_events(
    response: MessageResponse,
    out: &mut (impl Write + ?Sized),
) -> Result<Vec<AssistantEvent>, RuntimeError> {
    let mut events = Vec::new();
    let mut pending_tool = None;

    for block in response.content {
        let mut block_has_thinking_summary = false;
        push_output_block(
            block,
            out,
            &mut events,
            &mut pending_tool,
            false,
            &mut block_has_thinking_summary,
        )?;
        if let Some((id, name, input)) = pending_tool.take() {
            events.push(AssistantEvent::ToolUse { id, name, input });
        }
    }

    events.push(AssistantEvent::Usage(response.usage.token_usage()));
    events.push(AssistantEvent::MessageStop);
    Ok(events)
}

pub fn push_prompt_cache_record(client: &ApiProviderClient, events: &mut Vec<AssistantEvent>) {
    // `ApiProviderClient::take_last_prompt_cache_record` is a pass-through
    // to the Anthropic variant and returns `None` for OpenAI-compat /
    // xAI variants, which do not have a prompt cache. So this helper
    // remains a no-op on non-Anthropic providers without any extra
    // branching here.
    if let Some(record) = client.take_last_prompt_cache_record() {
        if let Some(event) = prompt_cache_record_to_runtime_event(record) {
            events.push(AssistantEvent::PromptCache(event));
        }
    }
}

pub fn prompt_cache_record_to_runtime_event(
    record: api::PromptCacheRecord,
) -> Option<PromptCacheEvent> {
    let cache_break = record.cache_break?;
    Some(PromptCacheEvent {
        unexpected: cache_break.unexpected,
        reason: cache_break.reason,
        previous_cache_read_input_tokens: cache_break.previous_cache_read_input_tokens,
        current_cache_read_input_tokens: cache_break.current_cache_read_input_tokens,
        token_drop: cache_break.token_drop,
    })
}

pub struct CliToolExecutor {
    pub renderer: TerminalRenderer,
    pub emit_output: bool,
    pub allowed_tools: Option<AllowedToolSet>,
    pub tool_registry: GlobalToolRegistry,
    pub mcp_state: Option<Arc<Mutex<RuntimeMcpState>>>,
}

impl CliToolExecutor {
    pub fn new(
        allowed_tools: Option<AllowedToolSet>,
        emit_output: bool,
        tool_registry: GlobalToolRegistry,
        mcp_state: Option<Arc<Mutex<RuntimeMcpState>>>,
    ) -> Self {
        Self {
            renderer: TerminalRenderer::new(),
            emit_output,
            allowed_tools,
            tool_registry,
            mcp_state,
        }
    }

    pub fn execute_search_tool(&self, value: serde_json::Value) -> Result<String, ToolError> {
        let input: ToolSearchRequest = serde_json::from_value(value)
            .map_err(|error| ToolError::new(format!("invalid tool input JSON: {error}")))?;
        let (pending_mcp_servers, mcp_degraded) =
            self.mcp_state.as_ref().map_or((None, None), |state| {
                let state = state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                (state.pending_servers(), state.degraded_report())
            });
        serde_json::to_string_pretty(&self.tool_registry.search(
            &input.query,
            input.max_results.unwrap_or(5),
            pending_mcp_servers,
            mcp_degraded,
        ))
        .map_err(|error| ToolError::new(error.to_string()))
    }

    pub fn execute_runtime_tool(
        &self,
        tool_name: &str,
        value: serde_json::Value,
    ) -> Result<String, ToolError> {
        let Some(mcp_state) = &self.mcp_state else {
            return Err(ToolError::new(format!(
                "runtime tool `{tool_name}` is unavailable without configured MCP servers"
            )));
        };
        let mut mcp_state = mcp_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        match tool_name {
            "MCPTool" => {
                let input: McpToolRequest = serde_json::from_value(value)
                    .map_err(|error| ToolError::new(format!("invalid tool input JSON: {error}")))?;
                let qualified_name = input
                    .qualified_name
                    .or(input.tool)
                    .ok_or_else(|| ToolError::new("missing required field `qualifiedName`"))?;
                mcp_state.call_tool(&qualified_name, input.arguments)
            }
            "ListMcpResourcesTool" => {
                let input: ListMcpResourcesRequest = serde_json::from_value(value)
                    .map_err(|error| ToolError::new(format!("invalid tool input JSON: {error}")))?;
                match input.server {
                    Some(server_name) => mcp_state.list_resources_for_server(&server_name),
                    None => mcp_state.list_resources_for_all_servers(),
                }
            }
            "ReadMcpResourceTool" => {
                let input: ReadMcpResourceRequest = serde_json::from_value(value)
                    .map_err(|error| ToolError::new(format!("invalid tool input JSON: {error}")))?;
                mcp_state.read_resource(&input.server, &input.uri)
            }
            _ => mcp_state.call_tool(tool_name, Some(value)),
        }
    }
}

impl ToolExecutor for CliToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        if self
            .allowed_tools
            .as_ref()
            .is_some_and(|allowed: &crate::AllowedToolSet| !allowed.contains(tool_name))
        {
            return Err(ToolError::new(format!(
                "tool `{tool_name}` is not enabled by the current --allowedTools setting"
            )));
        }
        let value = serde_json::from_str(input)
            .map_err(|error| ToolError::new(format!("invalid tool input JSON: {error}")))?;
        let result = if tool_name == "ToolSearch" {
            self.execute_search_tool(value)
        } else if self.tool_registry.has_runtime_tool(tool_name) {
            self.execute_runtime_tool(tool_name, value)
        } else {
            self.tool_registry
                .execute(tool_name, &value)
                .map_err(ToolError::new)
        };
        match result {
            Ok(output) => {
                if self.emit_output {
                    let markdown = format_tool_result(tool_name, &output, false);
                    self.renderer
                        .stream_markdown(&markdown, &mut io::stdout())
                        .map_err(|error| ToolError::new(error.to_string()))?;
                }
                Ok(output)
            }
            Err(error) => {
                if self.emit_output {
                    let markdown = format_tool_result(tool_name, &error.to_string(), true);
                    self.renderer
                        .stream_markdown(&markdown, &mut io::stdout())
                        .map_err(|stream_error| ToolError::new(stream_error.to_string()))?;
                }
                Err(error)
            }
        }
    }
}

pub fn permission_policy(
    mode: PermissionMode,
    feature_config: &runtime::RuntimeFeatureConfig,
    tool_registry: &GlobalToolRegistry,
) -> Result<PermissionPolicy, String> {
    Ok(tool_registry.permission_specs(None)?.into_iter().fold(
        PermissionPolicy::new(mode).with_permission_rules(feature_config.permission_rules()),
        |policy, (name, required_permission)| {
            policy.with_tool_requirement(name, required_permission)
        },
    ))
}

pub fn convert_messages(messages: &[ConversationMessage]) -> Vec<InputMessage> {
    messages
        .iter()
        .filter_map(|message| {
            let role = match message.role {
                MessageRole::System | MessageRole::User | MessageRole::Tool => "user",
                MessageRole::Assistant => "assistant",
            };
            let content = message
                .blocks
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => InputContentBlock::Text { text: text.clone() },
                    ContentBlock::ToolUse { id, name, input } => InputContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: serde_json::from_str(input)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": input })),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        output,
                        is_error,
                        ..
                    } => InputContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: vec![ToolResultContentBlock::Text {
                            text: output.clone(),
                        }],
                        is_error: *is_error,
                    },
                })
                .collect::<Vec<_>>();
            (!content.is_empty()).then(|| InputMessage {
                role: role.to_string(),
                content,
            })
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
pub fn print_help_to(out: &mut impl Write) -> io::Result<()> {
    writeln!(out, "claw v{VERSION}")?;
    writeln!(out)?;
    writeln!(out, "Usage:")?;
    writeln!(
        out,
        "  claw [--model MODEL] [--allowedTools TOOL[,TOOL...]]"
    )?;
    writeln!(out, "      Start the interactive REPL")?;
    writeln!(
        out,
        "  claw [--model MODEL] [--output-format text|json] prompt TEXT"
    )?;
    writeln!(out, "      Send one prompt and exit")?;
    writeln!(
        out,
        "  claw [--model MODEL] [--output-format text|json] TEXT"
    )?;
    writeln!(out, "      Shorthand non-interactive prompt mode")?;
    writeln!(
        out,
        "  claw --resume [SESSION.jsonl|session-id|latest] [/status] [/compact] [...]"
    )?;
    writeln!(
        out,
        "      Inspect or maintain a saved session without entering the REPL"
    )?;
    writeln!(out, "  claw help")?;
    writeln!(out, "      Alias for --help")?;
    writeln!(out, "  claw version")?;
    writeln!(out, "      Alias for --version")?;
    writeln!(out, "  claw status")?;
    writeln!(
        out,
        "      Show the current local workspace status snapshot"
    )?;
    writeln!(out, "  claw sandbox")?;
    writeln!(out, "      Show the current sandbox isolation snapshot")?;
    writeln!(out, "  claw doctor")?;
    writeln!(
        out,
        "      Diagnose local auth, config, workspace, and sandbox health"
    )?;
    writeln!(out, "  claw dump-manifests [--manifests-dir PATH]")?;
    writeln!(out, "  claw bootstrap-plan")?;
    writeln!(out, "  claw agents")?;
    writeln!(out, "  claw mcp")?;
    writeln!(out, "  claw skills")?;
    writeln!(out, "  claw system-prompt [--cwd PATH] [--date YYYY-MM-DD]")?;
    writeln!(out, "  claw init")?;
    writeln!(
        out,
        "  claw export [PATH] [--session SESSION] [--output PATH]"
    )?;
    writeln!(
        out,
        "      Dump the latest (or named) session as markdown; writes to PATH or stdout"
    )?;
    writeln!(out)?;
    writeln!(out, "Flags:")?;
    writeln!(
        out,
        "  --model MODEL              Override the active model"
    )?;
    writeln!(
        out,
        "  --output-format FORMAT     Non-interactive output format: text or json"
    )?;
    writeln!(
        out,
        "  --compact                  Strip tool call details; print only the final assistant text (text mode only; useful for piping)"
    )?;
    writeln!(
        out,
        "  --permission-mode MODE     Set read-only, workspace-write, or danger-full-access"
    )?;
    writeln!(
        out,
        "  --dangerously-skip-permissions  Skip all permission checks"
    )?;
    writeln!(out, "  --allowedTools TOOLS       Restrict enabled tools (repeatable; comma-separated aliases supported)")?;
    writeln!(
        out,
        "  --version, -V              Print version and build information locally"
    )?;
    writeln!(out)?;
    writeln!(out, "Interactive slash commands:")?;
    writeln!(out, "{}", render_slash_command_help_filtered(STUB_COMMANDS))?;
    writeln!(out)?;
    let resume_commands = resume_supported_slash_commands()
        .into_iter()
        .map(|spec| match spec.argument_hint {
            Some(argument_hint) => format!("/{} {}", spec.name, argument_hint),
            None => format!("/{}", spec.name),
        })
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(out, "Resume-safe commands: {resume_commands}")?;
    writeln!(out)?;
    writeln!(out, "Session shortcuts:")?;
    writeln!(
        out,
        "  REPL turns auto-save to .claw/sessions/<session-id>.{PRIMARY_SESSION_EXTENSION}"
    )?;
    writeln!(
        out,
        "  Use `{LATEST_SESSION_REFERENCE}` with --resume, /resume, or /session switch to target the newest saved session"
    )?;
    writeln!(
        out,
        "  Use /session list in the REPL to browse managed sessions"
    )?;
    writeln!(out, "Examples:")?;
    writeln!(out, "  claw --model claude-opus \"summarize this repo\"")?;
    writeln!(
        out,
        "  claw --output-format json prompt \"explain src/main.rs\""
    )?;
    writeln!(out, "  claw --compact \"summarize Cargo.toml\" | wc -l")?;
    writeln!(
        out,
        "  claw --allowedTools read,glob \"summarize Cargo.toml\""
    )?;
    writeln!(out, "  claw --resume {LATEST_SESSION_REFERENCE}")?;
    writeln!(
        out,
        "  claw --resume {LATEST_SESSION_REFERENCE} /status /diff /export notes.txt"
    )?;
    writeln!(out, "  claw agents")?;
    writeln!(out, "  claw mcp show my-server")?;
    writeln!(out, "  claw /skills")?;
    writeln!(out, "  claw doctor")?;
    writeln!(out, "  claw init")?;
    writeln!(out, "  claw export")?;
    writeln!(out, "  claw export conversation.md")?;
    Ok(())
}

pub fn print_help(output_format: CliOutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = Vec::new();
    print_help_to(&mut buffer)?;
    let message = String::from_utf8(buffer)?;
    match output_format {
        CliOutputFormat::Text => print!("{message}"),
        CliOutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "kind": "help",
                "message": message,
            }))?
        ),
    }
    Ok(())
}

