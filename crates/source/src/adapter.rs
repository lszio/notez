use domain::{Resource, ResourceRelation};
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfig {
    pub id: String,
    pub kind: SourceKind,
    pub path: PathBuf,
    pub read_only: bool,
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
            exclude_paths: Vec::new(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct ScannedSource {
    pub source_id: String,
    pub resources: Vec<Resource>,
    pub relations: Vec<ResourceRelation>,
}

pub trait SourceAdapter {
    fn config(&self) -> &SourceConfig;
    fn scan(&self) -> Result<ScannedSource, SourceError>;
}
