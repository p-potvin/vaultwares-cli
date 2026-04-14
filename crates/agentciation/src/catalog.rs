use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::manifest::{AgentDefinition, TaskDefinition, WorkflowDefinition};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogRevision {
    pub id: String,
    pub generated_at: String,
    pub file_count: usize,
}

impl CatalogRevision {
    #[must_use]
    pub fn new(id: impl Into<String>, file_count: usize) -> Self {
        Self {
            id: id.into(),
            generated_at: Utc::now().to_rfc3339(),
            file_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinationCatalog {
    pub revision: CatalogRevision,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<AgentDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflows: Vec<WorkflowDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_tasks: Vec<TaskDefinition>,
    pub source_root: String,
}
