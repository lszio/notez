use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use source::{SourceConfig, SourceKind};
use std::collections::BTreeMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("TOML parse error: {0}")]
    Toml(toml::de::Error),
    #[error("TOML parse error: {message}")]
    TomlField { message: String },
    #[error("Unsupported configuration version: {0}; this build understands version 1")]
    UnsupportedVersion(u32),
    #[error("Configuration required field `{0}` is missing")]
    MissingField(&'static str),
    #[error("Invalid value for `{0}`: {1}")]
    Invalid(&'static str, String),
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        // `toml::de::Error::span` is empty for "unknown field" errors. Fall
        // back to the full Display string so callers see a useful field path.
        ConfigError::TomlField { message: err.message().to_string() }
    }

}

const SUPPORTED_VERSION: u32 = 1;

/// Top-level XDG-global configuration: $XDG_CONFIG_HOME/notez/config.toml
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlobalConfig {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_space: Option<String>,
    #[serde(default)]
    pub spaces: BTreeMap<String, SpaceRegistration>,
    #[serde(default)]
    pub preferences: Preferences,
}

impl GlobalConfig {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let cfg: GlobalConfig = toml::from_str(text)?;
        if cfg.version != SUPPORTED_VERSION {
            return Err(ConfigError::UnsupportedVersion(cfg.version));
        }
        Ok(cfg)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRegistration {
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

/// Per-space configuration: <space>/notez.toml
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceConfig {
    pub version: u32,
    #[serde(default)]
    pub space: SpaceIdentity,
    #[serde(default)]
    pub workflow: WorkflowConfig,
    #[serde(default)]
    pub sources: Vec<SpaceSourceConfig>,
    #[serde(default)]
    pub link_overrides: JsonValue,
}

impl SpaceConfig {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let cfg: SpaceConfig = toml::from_str(text)?;
        if cfg.version != SUPPORTED_VERSION {
            return Err(ConfigError::UnsupportedVersion(cfg.version));
        }
        if cfg.space.name.is_empty() {
            return Err(ConfigError::MissingField("space.name"));
        }
        Ok(cfg)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceIdentity {
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_database")]
    pub database: PathBuf,
}

fn default_database() -> PathBuf {
    PathBuf::from(".notez/index.sqlite")
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowConfig {
    #[serde(default = "default_todo")]
    pub todo: Vec<String>,
    #[serde(default = "default_done")]
    pub done: Vec<String>,
}

fn default_todo() -> Vec<String> {
    vec!["TODO".into(), "NEXT".into(), "WAIT".into()]
}
fn default_done() -> Vec<String> {
    vec!["DONE".into(), "QUIT".into()]
}

/// Source entry inside `[[sources]]`. Mirrors [`source::SourceConfig`] but
/// keeps an `id` separate from the on-disk name so future migration can
/// re-key without breaking the project identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceSourceConfig {
    pub id: String,
    pub kind: SourceKind,
    pub path: PathBuf,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub include_paths: Vec<PathBuf>,
    #[serde(default)]
    pub exclude_paths: Vec<PathBuf>,
}

impl SpaceSourceConfig {
    pub fn into_source_config(self) -> SourceConfig {
        SourceConfig {
            id: self.id,
            kind: self.kind,
            path: self.path,
            read_only: self.read_only,
            include_paths: self.include_paths,
            exclude_paths: self.exclude_paths,
        }
    }
}

/// Resolved runtime configuration assembled from global + space + CLI.
/// Lives in `config` so CLI can compute it without taking a dependency on
/// `application`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub space_root: PathBuf,
    pub space_name: String,
    pub database: PathBuf,
    pub workflow: WorkflowConfig,
    pub sources: Vec<SourceConfig>,
    pub preferences: Preferences,
}
