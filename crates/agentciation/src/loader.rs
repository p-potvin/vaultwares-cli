use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::catalog::{CatalogRevision, CoordinationCatalog};
use crate::manifest::{
    AgentDefinition, AgentRole, Capability, HeartbeatPolicy, NudgePolicy, SourceRef,
    TaskDefinition, WorkflowDefinition, WorkflowStage,
};

#[derive(Debug, Clone)]
pub struct CatalogLoaderConfig {
    pub repo_root: PathBuf,
    pub submodule_path: PathBuf,
}

impl CatalogLoaderConfig {
    #[must_use]
    pub fn new(repo_root: impl Into<PathBuf>, submodule_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            submodule_path: submodule_path.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubmoduleCatalogLoader {
    config: CatalogLoaderConfig,
}

impl SubmoduleCatalogLoader {
    #[must_use]
    pub fn new(config: CatalogLoaderConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn for_repo(repo_root: impl Into<PathBuf>) -> Self {
        let repo_root = repo_root.into();
        Self::new(CatalogLoaderConfig::new(
            repo_root.clone(),
            repo_root.join("vaultwares-agentciation"),
        ))
    }

    pub fn load(&self) -> Result<CoordinationCatalog> {
        let source_root = self.config.submodule_path.clone();
        let definitions_root = source_root.join("definitions");
        let agents_root = source_root.join("agents");

        let definition_files = list_markdown_files(&definitions_root)?;
        let mut file_inputs = Vec::new();
        let mut agents = Vec::new();
        let mut shared_tasks = Vec::new();

        for definition_path in &definition_files {
            let stem = file_stem(definition_path)?;
            let python_path = agents_root.join(format!("{stem}.py"));
            if !python_path.exists() {
                continue;
            }

            let markdown = fs::read_to_string(definition_path)
                .with_context(|| format!("failed to read {}", definition_path.display()))?;
            let python = fs::read_to_string(&python_path)
                .with_context(|| format!("failed to read {}", python_path.display()))?;

            file_inputs.push((definition_path.clone(), markdown.clone()));
            file_inputs.push((python_path.clone(), python.clone()));

            let agent = parse_agent_definition(
                &source_root,
                definition_path,
                &markdown,
                &python_path,
                &python,
            )?;
            shared_tasks.extend(agent.task_types.iter().cloned());
            agents.push(agent);
        }

        let workflows = load_workflows(&source_root, &mut file_inputs)?;
        let revision = CatalogRevision::new(revision_hash(&file_inputs), file_inputs.len());

        Ok(CoordinationCatalog {
            revision,
            agents,
            workflows,
            shared_tasks,
            source_root: source_root.display().to_string(),
        })
    }
}

fn load_workflows(
    source_root: &Path,
    file_inputs: &mut Vec<(PathBuf, String)>,
) -> Result<Vec<WorkflowDefinition>> {
    let mut workflows = Vec::new();

    let extrovert_path = source_root.join("extrovert_agent.md");
    if extrovert_path.exists() {
        let markdown = fs::read_to_string(&extrovert_path)
            .with_context(|| format!("failed to read {}", extrovert_path.display()))?;
        file_inputs.push((extrovert_path.clone(), markdown.clone()));
        workflows.push(WorkflowDefinition {
            id: "socialization-routine".to_string(),
            name: "Socialization Routine".to_string(),
            stages: vec![
                WorkflowStage {
                    id: "send-heartbeat".to_string(),
                    description: "Send a heartbeat before user-facing work.".to_string(),
                },
                WorkflowStage {
                    id: "broadcast-status".to_string(),
                    description: "Broadcast the current status to peers.".to_string(),
                },
                WorkflowStage {
                    id: "project-recheck".to_string(),
                    description: "Re-evaluate TODO and roadmap context.".to_string(),
                },
                WorkflowStage {
                    id: "acknowledge-peers".to_string(),
                    description: "Acknowledge currently known peers.".to_string(),
                },
            ],
            escalation_rules: vec![
                "Missed heartbeats trigger manager attention.".to_string(),
                "Every user interaction must execute the socialization routine.".to_string(),
            ],
            manager_role: None,
            worker_roles: vec!["worker".to_string(), "specialist".to_string()],
            source: make_source_ref(source_root, &extrovert_path, &markdown),
        });
    }

    let manager_path = source_root.join("lonely_manager.py");
    if manager_path.exists() {
        let python = fs::read_to_string(&manager_path)
            .with_context(|| format!("failed to read {}", manager_path.display()))?;
        file_inputs.push((manager_path.clone(), python.clone()));
        workflows.push(WorkflowDefinition {
            id: "lonely-manager-loop".to_string(),
            name: "Lonely Manager Supervision Loop".to_string(),
            stages: vec![
                WorkflowStage {
                    id: "check-heartbeats".to_string(),
                    description: "Monitor missed heartbeats and mark lost lanes.".to_string(),
                },
                WorkflowStage {
                    id: "request-updates".to_string(),
                    description: "Request periodic worker updates.".to_string(),
                },
                WorkflowStage {
                    id: "realign".to_string(),
                    description: "Nudge quiet or drifting workers back to the roadmap.".to_string(),
                },
            ],
            escalation_rules: vec![
                "Five missed heartbeats produce a lost alert.".to_string(),
                "Silent workers are realigned after the configured threshold.".to_string(),
            ],
            manager_role: Some("manager".to_string()),
            worker_roles: vec!["worker".to_string(), "specialist".to_string()],
            source: make_source_ref(source_root, &manager_path, &python),
        });
    }

    for path in [
        source_root.join("AGENTS.md"),
        source_root.join("INSTRUCTIONS.md"),
    ] {
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        file_inputs.push((path.clone(), content.clone()));
        if content.contains("team-plan -> team-prd -> team-exec -> team-verify -> team-fix") {
            workflows.push(WorkflowDefinition {
                id: "team-pipeline".to_string(),
                name: "VaultWares Team Pipeline".to_string(),
                stages: vec![
                    WorkflowStage {
                        id: "team-plan".to_string(),
                        description: "Plan and split the work.".to_string(),
                    },
                    WorkflowStage {
                        id: "team-prd".to_string(),
                        description: "Freeze the task packet and acceptance criteria.".to_string(),
                    },
                    WorkflowStage {
                        id: "team-exec".to_string(),
                        description: "Run bounded worker lanes.".to_string(),
                    },
                    WorkflowStage {
                        id: "team-verify".to_string(),
                        description: "Verify outputs and evidence.".to_string(),
                    },
                    WorkflowStage {
                        id: "team-fix".to_string(),
                        description: "Iterate on failing lanes until green.".to_string(),
                    },
                ],
                escalation_rules: vec![
                    "Only bounded, verifiable subtasks are delegated.".to_string(),
                    "Leaders own verification and stop conditions.".to_string(),
                ],
                manager_role: Some("manager".to_string()),
                worker_roles: vec!["worker".to_string(), "specialist".to_string()],
                source: make_source_ref(source_root, &path, &content),
            });
            break;
        }
    }

    Ok(workflows)
}

fn parse_agent_definition(
    source_root: &Path,
    markdown_path: &Path,
    markdown: &str,
    python_path: &Path,
    python: &str,
) -> Result<AgentDefinition> {
    let title = markdown_heading(markdown).unwrap_or_else(|| "Unnamed Agent".to_string());
    let agent_id = extract_python_string(python, "AGENT_TYPE")
        .or_else(|| Some(file_stem(markdown_path).ok()?.replace("_agent", "")))
        .unwrap_or_else(|| "worker".to_string());
    let capabilities = extract_python_list(python, "SKILLS")
        .into_iter()
        .map(|name| Capability {
            name,
            aliases: Vec::new(),
            tool_allowlist: Vec::new(),
            model_preferences: Vec::new(),
        })
        .collect::<Vec<_>>();
    let task_types = extract_handler_keys(python)
        .into_iter()
        .map(|task_id| TaskDefinition {
            description: markdown_task_description(markdown, &task_id)
                .unwrap_or_else(|| format!("Task handled by {agent_id}")),
            id: task_id,
            required_capabilities: capabilities
                .iter()
                .map(|value| value.name.clone())
                .collect(),
            default_priority: None,
        })
        .collect::<Vec<_>>();

    Ok(AgentDefinition {
        id: agent_id,
        display_name: title,
        role: AgentRole::Specialist,
        capabilities,
        task_types,
        prompt_refs: vec![make_source_ref(source_root, markdown_path, markdown)],
        workflow_refs: vec!["socialization-routine".to_string()],
        heartbeat_policy: HeartbeatPolicy {
            interval_secs: 5,
            max_missed_heartbeats: 5,
        },
        nudge_policy: Some(NudgePolicy {
            silence_threshold_secs: 120,
            realign_after_secs: 120,
        }),
        source: make_source_ref(source_root, python_path, python),
    })
}

fn markdown_heading(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        line.trim()
            .strip_prefix("# ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn markdown_task_description(markdown: &str, task_id: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.contains(task_id) {
            return None;
        }
        let parts = trimmed.split('|').map(str::trim).collect::<Vec<_>>();
        if parts.len() >= 3 && parts[1].contains(task_id) {
            Some(parts[2].trim_matches('`').to_string())
        } else {
            None
        }
    })
}

fn extract_python_string(python: &str, constant: &str) -> Option<String> {
    python.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with(constant) {
            return None;
        }
        trimmed
            .split_once('=')
            .map(|(_, value)| {
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string()
            })
            .filter(|value| !value.is_empty())
    })
}

fn extract_python_list(python: &str, constant: &str) -> Vec<String> {
    let mut in_list = false;
    let mut values = Vec::new();

    for line in python.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(constant) && trimmed.contains('[') {
            in_list = true;
            continue;
        }
        if in_list && trimmed.starts_with(']') {
            break;
        }
        if in_list {
            let value = trimmed
                .trim_end_matches(',')
                .trim_matches('"')
                .trim_matches('\'')
                .trim();
            if !value.is_empty() {
                values.push(value.to_string());
            }
        }
    }

    values
}

fn extract_handler_keys(python: &str) -> Vec<String> {
    let mut in_handlers = false;
    let mut task_ids = Vec::new();

    for line in python.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("handlers = {") {
            in_handlers = true;
            continue;
        }
        if in_handlers && trimmed.starts_with('}') {
            break;
        }
        if in_handlers && trimmed.starts_with('"') {
            if let Some((task_id, _)) = trimmed.split_once(':') {
                task_ids.push(task_id.trim().trim_matches('"').to_string());
            }
        }
    }

    task_ids
}

fn make_source_ref(source_root: &Path, path: &Path, content: &str) -> SourceRef {
    SourceRef {
        repo_subpath: path
            .strip_prefix(source_root)
            .unwrap_or(path)
            .display()
            .to_string(),
        git_revision: "submodule-local".to_string(),
        content_hash: short_hash(content),
    }
}

fn revision_hash(files: &[(PathBuf, String)]) -> String {
    let mut hasher = DefaultHasher::new();
    for (path, content) in files {
        path.display().to_string().hash(&mut hasher);
        content.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

fn short_hash(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn list_markdown_files(root: &Path) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(root)
        .with_context(|| format!("failed to read {}", root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut files = entries
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("md"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn file_stem(path: &Path) -> Result<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .with_context(|| format!("missing file stem for {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{
        extract_handler_keys, extract_python_list, extract_python_string, markdown_heading,
    };
    use crate::{CatalogLoaderConfig, SubmoduleCatalogLoader};

    #[test]
    fn parses_python_constants_and_handlers() {
        let python = r#"
AGENT_TYPE = "text"
SKILLS = [
    "captioning",
    "prompt_engineering",
]
handlers = {
    "generate_caption": self._generate_caption,
    "enhance_prompt": self._enhance_prompt,
}
"#;
        assert_eq!(
            extract_python_string(python, "AGENT_TYPE").as_deref(),
            Some("text")
        );
        assert_eq!(
            extract_python_list(python, "SKILLS"),
            vec!["captioning".to_string(), "prompt_engineering".to_string()]
        );
        assert_eq!(
            extract_handler_keys(python),
            vec!["generate_caption".to_string(), "enhance_prompt".to_string()]
        );
    }

    #[test]
    fn parses_markdown_heading() {
        assert_eq!(
            markdown_heading("# Text Agent\n\nBody").as_deref(),
            Some("Text Agent")
        );
    }

    #[test]
    fn loads_catalog_from_temp_fixture() {
        let temp_root = std::env::temp_dir().join(format!(
            "agentciation-loader-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let submodule_root = temp_root.join("vaultwares-agentciation");
        let definitions = submodule_root.join("definitions");
        let agents = submodule_root.join("agents");
        fs::create_dir_all(&definitions).expect("definitions dir");
        fs::create_dir_all(&agents).expect("agents dir");

        fs::write(
            definitions.join("text_agent.md"),
            "# Text Agent\n\n| Task ID | Description |\n|---|---|\n| `generate_text` | Generate text |\n",
        )
        .expect("write markdown");
        fs::write(
            agents.join("text_agent.py"),
            "AGENT_TYPE = \"text\"\nSKILLS = [\n    \"captioning\",\n]\nhandlers = {\n    \"generate_text\": self._generate_text,\n}\n",
        )
        .expect("write python");
        fs::write(submodule_root.join("extrovert_agent.md"), "# Extrovert\n")
            .expect("write extrovert");
        fs::write(
            submodule_root.join("AGENTS.md"),
            "team-plan -> team-prd -> team-exec -> team-verify -> team-fix",
        )
        .expect("write agents");

        let loader = SubmoduleCatalogLoader::new(CatalogLoaderConfig::new(
            PathBuf::from(&temp_root),
            PathBuf::from(&submodule_root),
        ));
        let catalog = loader.load().expect("catalog should load");

        assert_eq!(catalog.agents.len(), 1);
        assert_eq!(catalog.agents[0].id, "text");
        assert_eq!(catalog.workflows.len(), 2);

        let _ = fs::remove_dir_all(temp_root);
    }
}
