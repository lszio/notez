use serde::{Deserialize, Serialize};
use crate::domain::{ProjectionReader, ProjectionWrite};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("TOML parse error: {0}")]
    Toml(toml::de::Error),
    #[error("TOML parse error: {message}")]
    TomlField { message: String },
    #[error("Unsupported configuration version: {0}; this build understands version 2")]
    UnsupportedVersion(u32),
    #[error("Configuration required field `{0}` is missing")]
    MissingField(&'static str),
    #[error("Invalid value for `{0}`: {1}")]
    Invalid(&'static str, String),
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::TomlField {
            message: err.message().to_string(),
        }
    }
}

/// Supported on-disk schema version. v1 was the `Space`/`[space]` schema;
/// v2 collapses every `Source` (a named, persisted configuration) into one
/// concept. See `migrate.rs` for the v1 → v2 upgrade path.
pub const CURRENT_VERSION: u32 = 2;

/// Top-level XDG-global configuration: `$XDG_CONFIG_HOME/notez/config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalConfig {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_source: Option<String>,
    /// Named sources registered globally (cross-machine aliases).
    #[serde(default)]
    pub sources: BTreeMap<String, SourceRegistration>,
    #[serde(default)]
    pub preferences: Preferences,
}

impl GlobalConfig {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let cfg: GlobalConfig = toml::from_str(text)?;
        if cfg.version != CURRENT_VERSION {
            return Err(ConfigError::UnsupportedVersion(cfg.version));
        }
        Ok(cfg)
    }
}

/// A named entry in the global registry. A `Source` is anything you can
/// open: a local folder of Org/Markdown files, a remote Notion workspace,
/// a Jira project. The `path` points at a local notez.toml root; remote
/// endpoints will be added once the URL/credential model lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRegistration {
    pub path: PathBuf,
    #[serde(default)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_output() -> String {
    "human".to_string()
}
fn default_log_level() -> String {
    "warn".to_string()
}

/// On-disk per-source configuration: `<source>/notez.toml`.
///
/// `SourceConfig` is the **only** structural concept: a named source
/// owns a root directory, an optional endpoint binding, a set of
/// capabilities, and an overlay (tags/views/PARA/communities). No
/// separate `Space` type exists. See docs/architecture.org §3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    pub version: u32,
    pub source: SourceIdentity,
    #[serde(default)]
    pub workflow: WorkflowConfig,
    /// Sub-sources this `Source` aggregates (e.g. local `docs/` plus a
    /// remote Notion workspace). An empty list is allowed: the Source
    /// itself acts as a single "root" source.
    #[serde(default)]
    pub sources: Vec<SourceInstanceConfig>,
    #[serde(default, skip_serializing_if = "JsonValue::is_null")]
    pub link_overrides: JsonValue,
}

impl SourceConfig {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let cfg: SourceConfig = toml::from_str(text)?;
        if cfg.version != CURRENT_VERSION {
            return Err(ConfigError::UnsupportedVersion(cfg.version));
        }
        if cfg.source.name.is_empty() {
            return Err(ConfigError::MissingField("source.name"));
        }
        Ok(cfg)
    }
}

pub fn default_database() -> PathBuf {
    PathBuf::from(".notez/index.sqlite")
}

pub fn default_todo() -> Vec<String> {
    vec!["TODO".into(), "NEXT".into(), "WAIT".into()]
}

pub fn default_done() -> Vec<String> {
    vec!["DONE".into(), "QUIT".into()]
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConfig {
    #[serde(default = "default_todo")]
    pub todo: Vec<String>,
    #[serde(default = "default_done")]
    pub done: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_database")]
    pub database: PathBuf,
}

/// Sub-source instance declared inside a `SourceConfig`. Mirrors the
/// adapter-side `crate::source::SourceConfig` (kept distinct to allow the
/// on-disk schema to evolve without entangling the adapter type).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInstanceConfig {
    pub id: String,
    pub kind: crate::source::SourceKind,
    pub path: PathBuf,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub include_paths: Vec<PathBuf>,
    #[serde(default)]
    pub exclude_paths: Vec<PathBuf>,
}

impl From<crate::source::SourceConfig> for SourceInstanceConfig {
    fn from(s: crate::source::SourceConfig) -> Self {
        Self {
            id: s.id,
            kind: s.kind,
            path: s.path,
            read_only: s.read_only,
            include_paths: s.include_paths,
            exclude_paths: s.exclude_paths,
        }
    }
}

impl SourceInstanceConfig {
    pub fn into_source_config(self) -> crate::source::SourceConfig {
        crate::source::SourceConfig {
            id: self.id,
            kind: self.kind,
            path: self.path,
            read_only: self.read_only,
            include_paths: self.include_paths,
            exclude_paths: self.exclude_paths,
        }
    }
}
