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
