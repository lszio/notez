use domain::{LinkOccurrence, Resource, ResourceRelation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SourceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Document error: {0}")]
    Document(#[from] document::DocumentError),
    #[error("Source error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Native,
    Git,
    Obsidian,
    Anytype,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCapabilities {
    pub can_read: bool,
    pub can_write: bool,
    pub can_import: bool,
    pub can_watch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedWrite {
    pub target_ref: String,
    pub payload: String,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteResult {
    pub target_ref: String,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfig {
    pub id: String,
    pub kind: SourceKind,
    pub path: PathBuf,
    pub read_only: bool,
    #[serde(default)]
    pub include_paths: Vec<PathBuf>,
    #[serde(default)]
    pub exclude_paths: Vec<PathBuf>,
}
impl SourceConfig {
    pub fn new(
        id: impl Into<String>,
        kind: SourceKind,
        path: impl Into<PathBuf>,
        read_only: bool,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            path: path.into(),
            read_only,
            include_paths: Vec::new(),
            exclude_paths: Vec::new(),
        }
    }
}

pub struct ScannedSource {
    pub source_id: String,
    pub resources: Vec<Resource>,
    pub relations: Vec<ResourceRelation>,
    pub link_occurrences: Vec<LinkOccurrence>,
}

pub trait SourceAdapter {
    fn config(&self) -> &SourceConfig;
    fn scan(&self) -> Result<ScannedSource, SourceError>;

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            can_read: true,
            can_write: false,
            can_import: false,
            can_watch: false,
        }
    }

    fn prepare_write(&self, target_ref: &str, payload: &str) -> Result<PreparedWrite, SourceError> {
        Ok(PreparedWrite {
            target_ref: target_ref.to_string(),
            payload: payload.to_string(),
            ready: true,
        })
    }

    fn commit_write(&self, prep: &PreparedWrite) -> Result<WriteResult, SourceError> {
        Ok(WriteResult {
            target_ref: prep.target_ref.clone(),
            committed: true,
        })
    }
}
