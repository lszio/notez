use crate::domain::{LinkOccurrence, Resource, ResourceRelation};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SourceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Document error: {0}")]
    Document(#[from] crate::document::OrgDocumentError),
    #[error("Markdown document error: {0}")]
    MarkdownDocument(#[from] crate::document::MarkdownDocumentError),
    #[error(
        "no format parser registered for MIME `{mime}` from source `{source_id}` at `{locator}`"
    )]
    ParserNotFound {
        source_id: String,
        mime: String,
        locator: String,
    },
    #[error("format parser error: {0}")]
    Parse(String),
    #[error("Source error: {0}")]
    Other(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Native,
    Git,
    Obsidian,
    Anytype,
    AppleNotes,
    AppleCalendar,
    NotezRest,
    Other(String),
}

impl SourceKind {
    pub fn as_str(&self) -> &str {
        match self {
            SourceKind::Native => "native",
            SourceKind::Git => "git",
            SourceKind::Obsidian => "obsidian",
            SourceKind::Anytype => "anytype",
            SourceKind::AppleNotes => "apple_notes",
            SourceKind::AppleCalendar => "apple_calendar",
            SourceKind::NotezRest => "notez-rest",
            SourceKind::Other(name) => name.as_str(),
        }
    }
}

impl serde::Serialize for SourceKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for SourceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "native" => SourceKind::Native,
            "git" => SourceKind::Git,
            "obsidian" => SourceKind::Obsidian,
            "anytype" => SourceKind::Anytype,
            "apple_notes" => SourceKind::AppleNotes,
            "apple_calendar" => SourceKind::AppleCalendar,
            "notez-rest" => SourceKind::NotezRest,
            other => {
                if other.is_empty() {
                    return Err(serde::de::Error::custom(
                        "SourceKind string must be non-empty",
                    ));
                }
                SourceKind::Other(s)
            }
        })
    }
}

impl std::fmt::Display for SourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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
    pub url: Option<String>,
    #[serde(default)]
    pub include_paths: Vec<PathBuf>,
    #[serde(default)]
    pub exclude_paths: Vec<PathBuf>,
    /// File-inclusion policy for this source (defaults for configs
    /// that predate the `[scan]` section).
    #[serde(default)]
    pub scan: crate::config::model::ScanConfig,
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
            url: None,
            include_paths: Vec::new(),
            exclude_paths: Vec::new(),
            scan: crate::config::model::ScanConfig::default(),
        }
    }
}
#[derive(Debug)]
pub struct ScannedSource {
    pub source_id: String,
    pub resources: Vec<Resource>,
    pub relations: Vec<ResourceRelation>,
    pub link_occurrences: Vec<LinkOccurrence>,
    /// Why files were skipped, by reason. Zero total means the scan
    /// saw no rejection, not that no file exists.
    pub ignored: crate::source::policy::IgnoreCounts,
}

pub trait SourceAdapter {
    fn config(&self) -> &SourceConfig;
    fn scan(&self) -> Result<ScannedSource, SourceError>;

    /// Structured reads are capability-gated; unsupported operations fail.
    fn list_resources(&self) -> Result<Vec<Resource>, SourceError> {
        Err(SourceError::Other("list operation unsupported".into()))
    }
    fn read_resource(&self, _locator: &str) -> Result<Option<Resource>, SourceError> {
        Err(SourceError::Other("read operation unsupported".into()))
    }
    fn search_resources(&self, _query: &str, _limit: usize) -> Result<Vec<Resource>, SourceError> {
        Err(SourceError::Other("search operation unsupported".into()))
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            can_read: true,
            can_write: false,
            can_import: false,
            can_watch: false,
        }
    }

    fn prepare_write(
        &self,
        _target_ref: &str,
        _payload: &str,
    ) -> Result<PreparedWrite, SourceError> {
        Err(SourceError::Other(
            "write not supported by this source adapter".into(),
        ))
    }

    fn commit_write(&self, _prep: &PreparedWrite) -> Result<WriteResult, SourceError> {
        Err(SourceError::Other(
            "write not supported by this source adapter".into(),
        ))
    }

    /// Whether `path` is this adapter's native location syntax (a file
    /// path, a `file://` URL, `notez://object/…`, a Notion URL, a Jira
    /// key, etc.). Defaults to `false`; adapters override to claim their
    /// own path forms.
    fn recognizes_path(&self, path: &str) -> bool {
        let _ = path;
        false
    }

    /// Resolve an adapter-native path to an object identity. Returns
    /// `Ok(None)` when `path` is not this adapter's form; `Err` when it
    /// is this adapter's form but cannot be resolved.
    fn resolve_path(
        &self,
        path: &str,
    ) -> Result<Option<crate::domain::ObjectIdentity>, SourceError> {
        let _ = path;
        Ok(None)
    }

    /// Render an object identity back to this adapter's canonical path
    /// form. Returns `None` when the identity does not belong to this
    /// adapter's authority.
    fn render_path(&self, identity: &crate::domain::ObjectIdentity) -> Option<String> {
        let _ = identity;
        None
    }
}

use crate::source::protocol::{FormatParser, SourceTransport};

pub struct ComposedSourceAdapter {
    config: SourceConfig,
    transport: Box<dyn SourceTransport>,
    parsers: Vec<Box<dyn FormatParser>>,
}

impl ComposedSourceAdapter {
    pub fn new(
        config: SourceConfig,
        transport: Box<dyn SourceTransport>,
        parsers: Vec<Box<dyn FormatParser>>,
    ) -> Self {
        Self {
            config,
            transport,
            parsers,
        }
    }
}

impl SourceAdapter for ComposedSourceAdapter {
    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        let raw_entities = self
            .transport
            .fetch_raw()
            .map_err(|e| SourceError::Other(e.to_string()))?;

        let mut resources = Vec::new();
        let mut relations = Vec::new();
        let mut link_occurrences = Vec::new();

        for entity in raw_entities {
            let parser = self
                .parsers
                .iter()
                .find(|p| p.supports(&entity.mime_type))
                .ok_or_else(|| SourceError::ParserNotFound {
                    source_id: self.config.id.clone(),
                    mime: entity.mime_type.clone(),
                    locator: entity.locator.clone(),
                })?;

            let parsed = parser
                .parse(&entity, &self.config.id)
                .map_err(|e| SourceError::Parse(e.to_string()))?;

            resources.extend(parsed.resources);
            relations.extend(parsed.relations);
            link_occurrences.extend(parsed.link_occurrences);
        }

        Ok(ScannedSource {
            source_id: self.config.id.clone(),
            resources,
            relations,
            link_occurrences,
            ignored: self.transport.ignored(),
        })
    }

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
        self.transport
            .mutate(&prep.target_ref, &prep.payload)
            .map_err(|e| SourceError::Other(e.to_string()))?;
        Ok(WriteResult {
            target_ref: prep.target_ref.clone(),
            committed: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_kind_other_round_trips_via_serde() {
        let k = SourceKind::Other("notion".to_string());
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"notion\"");
        let parsed: SourceKind = serde_json::from_str("\"notion\"").unwrap();
        assert_eq!(parsed, k);
    }

    #[test]
    fn source_kind_built_in_variants_round_trip_via_serde() {
        for k in [SourceKind::Native, SourceKind::Git, SourceKind::Obsidian, SourceKind::Anytype, SourceKind::AppleNotes, SourceKind::AppleCalendar, SourceKind::NotezRest] {
            let s = serde_json::to_string(&k).unwrap();
            let parsed: SourceKind = serde_json::from_str(&s).unwrap();
            assert_eq!(parsed, k, "round-trip failed for {k:?}");
        }
    }
}
