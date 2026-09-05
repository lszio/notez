use crate::application::context::SourceContext;
use crate::domain::{ProjectionReader, ProjectionWrite};
use crate::config::SourceInstanceConfig;
use crate::domain::{
    LinkDiagnostic, LinkOccurrence, ProjectionStore, QueryPage, ResolutionStatus, ResolvedRelation,
    Resource, ResourceKind, ResourceRef, Selector,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// The stable error taxonomy for the application layer. Every variant
/// carries typed fields; the `Display` output and the `serde` shape are
/// public contracts (see docs/superpowers/specs/2026-08-02-application-error-design.org).

///
/// Wire format: internally tagged JSON, `{"kind": "<snake_case variant>", ...}`.
/// The impls are hand-written because `UnsupportedCapability` carries a
/// `&'static str` (no `Deserialize` impl) and `Io` carries
/// `std::io::ErrorKind` (no serde impls at all); see `crate::error_serde`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationError {
    /// A single resource reference could not be resolved to a row.
    NotFound {
        kind: ResourceKind,
        r_ref: ResourceRef,
    },
    /// A projection/storage operation failed.
    Storage {
        kind: StorageErrorKind,
        message: String,
    },
    /// A document parser failed.
    Document { source: DocumentErrorKind },
    /// A filesystem operation failed.
    Io {
        path: Option<PathBuf>,
        source: std::io::ErrorKind,
    },
    /// A restricted Janet execution failed in a stable category.
    Janet { kind: String, message: String },
    /// A requested capability is not implemented by this build.
    UnsupportedCapability { capability: &'static str },
    /// An expected revision did not match the persisted value.
    RevisionConflict { expected: String, actual: String },
    /// A write was attempted against a read-only source.
    ReadOnlySource { source_id: String },
    /// A source with the given id is not registered in the space.
    SourceNotFound { source_id: String },
    /// The same ResourceAddress already binds to a different ResourceRef.
    /// See spec `docs/superpowers/specs/2026-08-09-0.5x-a1-a3-usecase-impl-split-and-write-checks-design.org` §2.4.
    AddressUniqueness {
        addr: crate::domain::ResourceAddress,
        existing: ResourceRef,
        candidate: ResourceRef,
    },
    /// A protocol request could not be interpreted: malformed ref,
    /// unknown kind, missing required field. Every surface surfaces
    /// this identically instead of re-implementing validation.
    InvalidRequest { message: String },
}

/// Classification of storage-layer failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageErrorKind {
    /// Failure from the SQLite projection.
    Sqlite,
    /// A required blob is missing.
    BlobMissing,
    /// No source is registered in the space.
    NoSourceRegistered,
    /// Any other invalid-state condition.
    InvalidState,
}

/// Which document parser produced the error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum DocumentErrorKind {
    Org(crate::document::OrgDocumentError),
    Markdown(crate::document::MarkdownDocumentError),
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicationError::NotFound { kind, r_ref } => {
                write!(f, "resource not found: {r_ref} (kind={})", kind.as_str())
            }
            ApplicationError::Storage { kind, message } => {
                write!(f, "storage error ({kind}): {message}")
            }
            ApplicationError::Document { source } => match source {
                DocumentErrorKind::Org(e) => write!(f, "document error (org): {e}"),
                DocumentErrorKind::Markdown(e) => write!(f, "document error (markdown): {e}"),
            },
            ApplicationError::Io { path, source } => match path {
                Some(p) => write!(f, "io error: {source} at {}", p.display()),
                None => write!(f, "io error: {source}"),
            },
            ApplicationError::Janet { kind, message } => write!(f, "janet {kind}: {message}"),
            ApplicationError::UnsupportedCapability { capability } => {
                write!(f, "unsupported capability: {capability}")
            }
            ApplicationError::RevisionConflict { expected, actual } => {
                write!(f, "revision conflict: expected {expected}, actual {actual}")
            }
            ApplicationError::ReadOnlySource { source_id } => {
                write!(f, "read-only source: {source_id}")
            }
            ApplicationError::SourceNotFound { source_id } => {
                write!(f, "source not found: {source_id}")
            }
            ApplicationError::AddressUniqueness {
                addr,
                existing,
                candidate,
            } => {
                // ResourceAddress has no Display impl yet; render the
                // bound refs (which are Copy) instead.
                let _ = addr;
                write!(
                    f,
                    "address uniqueness: already bound to {existing}; cannot rebind to {candidate}"
                )
            }
            ApplicationError::InvalidRequest { message } => {
                write!(f, "invalid request: {message}")
            }
        }
    }
}

impl std::error::Error for ApplicationError {}

impl serde::Serialize for ApplicationError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        match self {
            ApplicationError::NotFound { kind, r_ref } => {
                map.serialize_entry("kind", "not_found")?;
                map.serialize_entry("resource_kind", kind)?;
                map.serialize_entry("r_ref", r_ref)?;
            }
            ApplicationError::Storage { kind, message } => {
                map.serialize_entry("kind", "storage")?;
                map.serialize_entry("storage_kind", kind)?;
                map.serialize_entry("message", message)?;
            }
            ApplicationError::Document { source } => {
                map.serialize_entry("kind", "document")?;
                map.serialize_entry("source", source)?;
            }
            ApplicationError::Io { path, source } => {
                map.serialize_entry("kind", "io")?;
                map.serialize_entry("path", path)?;
                map.serialize_entry("source", crate::error_serde::name(source))?;
            }
            ApplicationError::Janet { kind, message } => {
                map.serialize_entry("kind", "janet")?;
                map.serialize_entry("janet_kind", kind)?;
                map.serialize_entry("message", message)?;
            }
            ApplicationError::UnsupportedCapability { capability } => {
                map.serialize_entry("kind", "unsupported_capability")?;
                map.serialize_entry("capability", capability)?;
            }
            ApplicationError::RevisionConflict { expected, actual } => {
                map.serialize_entry("kind", "revision_conflict")?;
                map.serialize_entry("expected", expected)?;
                map.serialize_entry("actual", actual)?;
            }
            ApplicationError::ReadOnlySource { source_id } => {
                map.serialize_entry("kind", "read_only_source")?;
                map.serialize_entry("source_id", source_id)?;
            }
            ApplicationError::SourceNotFound { source_id } => {
                map.serialize_entry("kind", "source_not_found")?;
                map.serialize_entry("source_id", source_id)?;
            }
            ApplicationError::AddressUniqueness {
                addr,
                existing,
                candidate,
            } => {
                map.serialize_entry("kind", "address_uniqueness")?;
                map.serialize_entry("addr", addr)?;
                map.serialize_entry("existing", existing)?;
                map.serialize_entry("candidate", candidate)?;
            }
            ApplicationError::InvalidRequest { message } => {
                map.serialize_entry("kind", "invalid_request")?;
                map.serialize_entry("message", message)?;
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for ApplicationError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        let kind = value
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| D::Error::custom("ApplicationError: missing `kind` tag"))?;
        match kind {
            "not_found" => {
                let kind = value
                    .get("resource_kind")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `resource_kind`"))?;
                let r_ref = value
                    .get("r_ref")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `r_ref`"))?;
                Ok(ApplicationError::NotFound {
                    kind: serde_json::from_value(kind.clone()).map_err(D::Error::custom)?,
                    r_ref: serde_json::from_value(r_ref.clone()).map_err(D::Error::custom)?,
                })
            }
            "storage" => {
                let kind = value
                    .get("storage_kind")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `storage_kind`"))?;
                let message = value
                    .get("message")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `message`"))?;
                Ok(ApplicationError::Storage {
                    kind: serde_json::from_value(kind.clone()).map_err(D::Error::custom)?,
                    message: serde_json::from_value(message.clone()).map_err(D::Error::custom)?,
                })
            }
            "document" => {
                let source = value
                    .get("source")
                    .cloned()
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `source`"))?;
                Ok(ApplicationError::Document {
                    source: serde_json::from_value(source).map_err(D::Error::custom)?,
                })
            }
            "io" => {
                let path = serde_json::from_value(value.get("path").cloned().unwrap_or(serde_json::Value::Null)).map_err(D::Error::custom)?;
                let source = value.get("source").and_then(|v| v.as_str()).ok_or_else(|| D::Error::custom("ApplicationError: missing `source`"))?;
                Ok(ApplicationError::Io { path, source: crate::error_serde::from_name(source) })
            }
            "janet" => {
                let kind = value.get("janet_kind").and_then(|v| v.as_str()).ok_or_else(|| D::Error::custom("ApplicationError: missing `janet_kind`"))?;
                let message = value.get("message").and_then(|v| v.as_str()).ok_or_else(|| D::Error::custom("ApplicationError: missing `message`"))?;
                Ok(ApplicationError::Janet { kind: kind.to_string(), message: message.to_string() })
            }
            "unsupported_capability" => {
                let capability = value.get("capability").and_then(|v| v.as_str()).ok_or_else(|| D::Error::custom("ApplicationError: missing `capability`"))?;
                const KNOWN: &[&str] = &["conflict list is not yet implemented; use `notez sync` commands", "space doctor is not yet implemented", "job manager is not yet implemented; jobs are tracked via `notez task`", "artifact freshness check is not yet implemented", "relay sync is not yet implemented; sync via folder transport", "execute_janet"];
                let capability = KNOWN.iter().find(|k| **k == capability).copied().ok_or_else(|| D::Error::custom("ApplicationError: unknown unsupported capability string"))?;
                Ok(ApplicationError::UnsupportedCapability { capability })
            }
            "revision_conflict" => {
                let expected = value.get("expected").ok_or_else(|| D::Error::custom("ApplicationError: missing `expected`"))?;
                let actual = value.get("actual").ok_or_else(|| D::Error::custom("ApplicationError: missing `actual`"))?;
                Ok(ApplicationError::RevisionConflict { expected: serde_json::from_value(expected.clone()).map_err(D::Error::custom)?, actual: serde_json::from_value(actual.clone()).map_err(D::Error::custom)? })
            }
            "read_only_source" => {
                let source_id = value
                    .get("source_id")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `source_id`"))?;
                Ok(ApplicationError::ReadOnlySource {
                    source_id: serde_json::from_value(source_id.clone())
                        .map_err(D::Error::custom)?,
                })
            }
            "source_not_found" => {
                let source_id = value
                    .get("source_id")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `source_id`"))?;
                Ok(ApplicationError::SourceNotFound {
                    source_id: serde_json::from_value(source_id.clone())
                        .map_err(D::Error::custom)?,
                })
            }
            "address_uniqueness" => {
                let addr = value
                    .get("addr")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `addr`"))?;
                let existing = value
                    .get("existing")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `existing`"))?;
                let candidate = value
                    .get("candidate")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `candidate`"))?;
                Ok(ApplicationError::AddressUniqueness {
                    addr: serde_json::from_value(addr.clone()).map_err(D::Error::custom)?,
                    existing: serde_json::from_value(existing.clone()).map_err(D::Error::custom)?,
                    candidate: serde_json::from_value(candidate.clone())
                        .map_err(D::Error::custom)?,
                })
            }
            "invalid_request" => {
                let message = value
                    .get("message")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `message`"))?;
                Ok(ApplicationError::InvalidRequest {
                    message: serde_json::from_value(message.clone()).map_err(D::Error::custom)?,
                })
            }
            other => Err(D::Error::custom(format!(
                "ApplicationError: unknown kind tag `{other}`"
            ))),
        }
    }
}

impl std::fmt::Display for StorageErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageErrorKind::Sqlite => f.write_str("sqlite"),
            StorageErrorKind::BlobMissing => f.write_str("blob_missing"),
            StorageErrorKind::NoSourceRegistered => f.write_str("no_source_registered"),
            StorageErrorKind::InvalidState => f.write_str("invalid_state"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ResolveResult {
    Found(ResourceRef),
    NotFound,
    Ambiguous(Vec<ResourceRef>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScanReport {
    pub scanned_files: usize,
    pub scanned_resources: usize,
    pub scanned_relations: usize,
    /// Total files rejected by the source scan policy.
    pub ignored: u32,
    pub ignored_hidden: u32,
    pub ignored_excluded: u32,
    pub ignored_not_included: u32,
    pub ignored_too_large: u32,
    pub ignored_symlink: u32,
}

/// Result of a successful engine-level document update.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DocumentUpdateReport {
    /// Ref of the updated document row.
    pub r_ref: String,
    /// Source-relative locator of the written file.
    pub locator: String,
    /// Revision of the freshly rescanned row (content hash).
    pub revision: String,
}
pub struct Engine<S: ProjectionStore> {
    pub(crate) store: S,
    pub(crate) rule_engine: crate::domain::RuleEngine,
    pub(crate) format_parsers: Vec<Box<dyn crate::source::FormatParser>>,
    pub(crate) source: Option<SourceContext>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) janet_executor: Option<Box<dyn JanetExecutor>>,
    /// Card execution service: registry of language executors plus a
    /// memo cache. Default-constructed with the built-in Janet
    /// executor wired up so surfaces never need to bootstrap one.
    pub(crate) card_service: super::card_executor::CardExecutionService,
    pub(crate) capability_catalog: crate::capability::CapabilityCatalog,
    pub(crate) source_registry: crate::source::SourceRegistry,
    pub(crate) journal: Box<dyn crate::domain::journal::EventJournal>,
    pub(crate) audit: Box<dyn crate::domain::audit::AuditLog>,
    pub(crate) clock: Box<dyn crate::application::ports::Clock>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default)]
pub struct JanetQuerySnapshot {
    pub sources: serde_json::Value,
    pub search: serde_json::Value,
    pub objects: serde_json::Value,
    pub relations: serde_json::Value,
    pub reads: std::collections::BTreeMap<String, serde_json::Value>,
    pub render_list: serde_json::Value,
}

#[cfg(not(target_arch = "wasm32"))]
pub trait JanetExecutor: Send {
    fn execute(
        &mut self,
        request: &notez_protocol::request::ExecuteJanetRequest,
        snapshot: &JanetQuerySnapshot,
    ) -> Result<serde_json::Value, (String, String)>;
}


impl<S> Engine<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    /// Build a facade with the default in-memory state.
    pub fn new(store: S) -> Self {
        Self {
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            source: None,
            #[cfg(not(target_arch = "wasm32"))]
            janet_executor: None,
            card_service: super::card_executor::CardExecutionService::new(super::card_executor::CardCache::default())
                .with_executor(std::sync::Arc::new(super::janet::JanetCardExecutor) as std::sync::Arc<dyn super::card_executor::CardExecutor>),
            capability_catalog: crate::capability::CapabilityCatalog::with_builtins(),
            source_registry: crate::source::SourceRegistry::with_builtins(),
            journal: Box::new(crate::domain::journal::NullJournal::default()),
            audit: Box::new(crate::domain::audit::NullAuditLog::default()),
            clock: Box::new(crate::application::ports::SystemClock),
        }
    }
    /// Construct an `Engine` bound to an explicit `SourceContext`.
    pub fn with_source(store: S, source: SourceContext) -> Self {
        Self {
            source: Some(source),
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            janet_executor: None,
            card_service: super::card_executor::CardExecutionService::new(super::card_executor::CardCache::default())
                .with_executor(std::sync::Arc::new(super::janet::JanetCardExecutor) as std::sync::Arc<dyn super::card_executor::CardExecutor>),
            capability_catalog: crate::capability::CapabilityCatalog::with_builtins(),
            source_registry: crate::source::SourceRegistry::with_builtins(),
            journal: Box::new(crate::domain::journal::NullJournal::default()),
            audit: Box::new(crate::domain::audit::NullAuditLog::default()),
            clock: Box::new(crate::application::ports::SystemClock),
        }
    }

    /// Borrow the card execution service for callers that need to
    /// project documents into typed [`crate::application::card_executor::CardProjection`]s.
    pub fn card_service(&self) -> &super::card_executor::CardExecutionService {
        &self.card_service
    }

    /// Attach an additional [`super::card_executor::CardExecutor`]
    /// for a non-Janet language (e.g. SQL).
    pub fn attach_card_executor(
        &mut self,
        executor: std::sync::Arc<dyn super::card_executor::CardExecutor>,
    ) {
        self.card_service
            .executors
            .lock()
            .expect("card executor registry lock")
            .insert(executor.language().to_string(), executor);
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn attach_janet_executor(&mut self, executor: impl JanetExecutor + 'static) {
        self.janet_executor = Some(Box::new(executor));
    }
    /// Construct an `Engine` with a caller-supplied source factory registry.
    /// This constructor is used by tests and third-party compositions.
    /// The default constructors register built-in factories.
    /// third-party compositions that want to start from an empty
    /// registry or one with custom factories pre-registered.
    pub fn with_registry(store: S, registry: crate::source::SourceRegistry) -> Self {
        Self {
            source: None,
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            janet_executor: None,
            card_service: super::card_executor::CardExecutionService::new(super::card_executor::CardCache::default())
                .with_executor(std::sync::Arc::new(super::janet::JanetCardExecutor) as std::sync::Arc<dyn super::card_executor::CardExecutor>),
            capability_catalog: crate::capability::CapabilityCatalog::with_builtins(),
            source_registry: registry,
            journal: Box::new(crate::domain::journal::NullJournal::default()),
            audit: Box::new(crate::domain::audit::NullAuditLog::default()),
            clock: Box::new(crate::application::ports::SystemClock),
        }
    }
    /// Register an additional `SourceAdapterFactory`. Used by tests
    /// and by third-party composition roots that need to handle
    /// `SourceKind::Other(...)` variants.
    pub fn register_source_factory(
        &mut self,
        factory: Box<dyn crate::source::SourceAdapterFactory>,
    ) {
        self.source_registry.register(factory);
    }

    /// Return the active space context, if one was provided at
    /// construction. Callers that need filesystem paths should use this
    /// getter rather than the process working directory.
    pub fn source(&self) -> Option<&SourceContext> {
        self.source.as_ref()
    }

    /// Return a clone of the bound space root, or fail with the canonical
    /// "no explicit SourceContext" error. Use-case implementations call
    /// this instead of taking a `source_root` parameter so every transport
    /// derives paths from the single composition-root binding.
    pub(crate) fn require_space_root(&self) -> Result<std::path::PathBuf, ApplicationError> {
        self.source.as_ref().map(|s| s.root.clone()).ok_or_else(|| ApplicationError::Storage {
            kind: StorageErrorKind::InvalidState,
            message: "operation requires an explicit SourceContext; open the runtime via composition::open_space".to_string(),
        })
    }

    /// Register a `FormatParser` for use by future scans. Callers (typically
    /// the composition root) are responsible for registering every parser
    /// the runtime needs. Without registrations, scans against a source
    /// whose transport yields non-Org/Orphan-Entity MIME bytes will fail
    /// with `ParserNotFound`.
    pub fn register_format_parser(&mut self, parser: Box<dyn crate::source::FormatParser>) {
        self.format_parsers.push(parser);
    }

    /// Register a public capability descriptor. The descriptor is
    /// inserted into (or replaces the entry in) the active
    /// [`CapabilityCatalog`] and the `capabilities_json` helper observe the
    /// registered set. This is the canonical composition-root path.
    pub fn register_capability(&mut self, descriptor: &crate::capability::CapabilityDescriptor) {
        self.capability_catalog.register(descriptor.clone());
    }

    /// Borrow the active capability catalog. Used by CLI help text,
    /// MCP tool listings, and documentation generators to surface a
    /// single source of truth for what the facade can do.
    pub fn capability_catalog(&self) -> &crate::capability::CapabilityCatalog {
        &self.capability_catalog
    }

    /// Actor principal for audit/journal records. Returns the
    /// process-level principal placeholder until per-call identity
    /// propagation lands in the protocol dispatch layer.
    pub fn actor_principal(&self) -> String {
        "default".to_string()
    }

    pub fn now_unix_millis(&self) -> i64 {
        use crate::application::ports::Clock;
        self.clock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default()
    }

    /// Replace the event journal adapter. Used by the composition root
    /// to install the durable sqlite-backed journal.
    pub fn attach_journal(
        &mut self,
        journal: impl crate::domain::journal::EventJournal + 'static,
    ) {
        self.journal = Box::new(journal);
    }

    /// Replace the audit log adapter.
    pub fn attach_audit(
        &mut self,
        audit: impl crate::domain::audit::AuditLog + 'static,
    ) {
        self.audit = Box::new(audit);
    }

    /// Borrow the journal adapter (downcast to trait object).
    fn journal_ref(&self) -> &dyn crate::domain::journal::EventJournal {
        &*self.journal
    }

    /// Borrow the audit adapter.
    fn audit_ref(&self) -> &dyn crate::domain::audit::AuditLog {
        &*self.audit
    }

    /// shared [`crate::capability::catalog_to_json_array`] helper. This
    /// is the canonical "list capabilities" payload used by both the
    /// CLI `list-capabilities` subcommand and the MCP
    /// `list_capabilities` tool, so the two surfaces stay byte-equal.
    pub fn capabilities_json(&self) -> serde_json::Value {
        crate::capability::catalog_to_json_array(&self.capability_catalog)
    }

    // NOTE: no public `store()` / `store_mut()` accessors. Transports and
    // external callers must go through the use-case methods; handing out
    // the raw projection would let them bypass write checks and journaling.
    pub fn inspect_rules(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<crate::domain::InspectResult>, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::inspect_rules(self, r_ref)
    }

    pub fn add_source(
        &mut self,
        config: crate::source::SourceConfig,
    ) -> Result<(), ApplicationError> {
        let source_root = self.require_space_root()?;
        let mut cfg = crate::application::federation::SourceInstancesCache::load(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        cfg.sources.retain(|s| s.id != config.id);
        cfg.sources.push(config.clone().into());
        cfg.save(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        // Mirror the change into the in-memory SourceContext so subsequent
        // operations (e.g. `writeback_resource`, `transition_task`) see
        // the new source without needing to reload the space.
        if let Some(src) = self.source.as_mut() {
            src.config.sources.retain(|s| s.id != config.id);
            src.config.sources.push(config.into());
        }
        Ok(())
    }

    /// Persist a new or updated resource. The projection is updated
    /// atomically; the resource's source adapter (when present and writable)
    /// is invoked so the authoritative backing store stays in sync.

    /// Delete a resource by `ResourceRef`. Idempotent at the projection layer.

    /// Recent activity feed, ordered by `revision` descending.

    /// List resources from a given source adapter.

    pub fn list_sources(&self) -> Result<Vec<crate::source::SourceConfig>, ApplicationError> {
        let source_root = self.require_space_root()?;
        let cfg = crate::application::federation::SourceInstancesCache::load(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        Ok(cfg
            .sources
            .into_iter()
            .map(|s| s.into_source_config())
            .collect())
    }

    pub fn scan_federation(&mut self) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_federation(self)
    }

    pub fn scan_native(&mut self) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_native(self)
    }

    pub fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkOccurrence>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::query_link_occurrences(
            self, source_ref,
        )
    }

    pub fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::ResolvedRelation>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::query_resolved_relations(
            self, source_ref,
        )
    }

    /// List every link occurrence originating from `source_ref` (raw form).
    pub fn list_links(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::list_links(self, source_ref)
    }

    /// Re-resolve every occurrence for `source_ref` and persist diagnostics.
    pub fn resolve_links(
        &mut self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::resolve_links(self, source_ref)
    }

    /// Return per-occurrence diagnostics (status + candidates) for `source_ref`.
    pub fn diagnose_link(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkDiagnostic>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::diagnose_link(self, source_ref)
    }

    /// Walk the bound native source again, resolve every link,
    /// and return a [`LinkReindexReport`].
    pub fn reindex_links(
        &mut self,
    ) -> Result<crate::application::link_resolution::LinkReindexReport, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::reindex_links(self)
    }

    /// Resolve a `ResourceAddress` (either a `Ref` or a `Locator`) and
    /// return a [`ResolveResult`].

    pub fn agenda(&self) -> Result<crate::application::task_para::AgendaView, ApplicationError> {
        <Self as crate::application::use_cases::TaskUseCase>::agenda(self)
    }

    pub fn transition_task(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
    ) -> Result<crate::document::StateTransition, ApplicationError> {
        <Self as crate::application::use_cases::TaskUseCase>::transition_task(
            self, r_ref, to_state, timestamp,
        )
    }

    pub fn para_overview(
        &self,
    ) -> Result<crate::application::task_para::ParaOverview, ApplicationError> {
        <Self as crate::application::use_cases::TaskUseCase>::para_overview(self)
    }

    pub fn add_attachment(
        &mut self,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::add_attachment(
            self,
            file_path,
            default_mime,
        )
    }

    pub fn run_extraction(
        &mut self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::run_extraction(self, att_ref)
    }

    pub fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::query_segments(self, att_ref)
    }

    pub fn create_community(
        &self,
        community: crate::domain::community::Community,
    ) -> Result<(), ApplicationError> {
        <Self as crate::application::use_cases::CommunityUseCase>::create_community(self, community)
    }

    pub fn list_communities(
        &self,
    ) -> Result<Vec<crate::domain::community::Community>, ApplicationError> {
        <Self as crate::application::use_cases::CommunityUseCase>::list_communities(self)
    }

    pub fn derive_artifact(
        &self,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<crate::artifact::DerivedArtifact, ApplicationError> {
        <Self as crate::application::use_cases::ArtifactUseCase>::derive_artifact(
            self,
            community_id,
            recipe_name,
        )
    }

    pub fn export_skill(
        &self,
        community_id: &str,
        description: &str,
        export_path: &Path,
    ) -> Result<crate::artifact::SkillPackage, ApplicationError> {
        <Self as crate::application::use_cases::ArtifactUseCase>::export_skill(
            self,
            community_id,
            description,
            export_path,
        )
    }

    pub fn sync_push(
        &mut self,
        actor_id: &str,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::sync_push(
            self,
            actor_id,
            shared_folder,
        )
    }
    pub fn sync_pull(
        &mut self,
        actor_id: &str,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::sync_pull(
            self,
            actor_id,
            shared_folder,
        )
    }

    pub fn list_conflicts(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::list_conflicts(self)
    }

    pub fn source_doctor(
        &self,
    ) -> Result<crate::application::doctor::DoctorReport, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::source_doctor(self)
    }

    pub fn list_jobs(
        &self,
    ) -> Result<Vec<crate::application::job_manager::JobRecord>, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::list_jobs(self)
    }

    pub fn check_artifact_freshness(
        &self,
    ) -> Result<crate::application::job_manager::ArtifactStaleReport, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::check_artifact_freshness(self)
    }

    pub fn writeback_resource(
        &self,
        source_id: &str,
        target_ref: &str,
        payload: &str,
    ) -> Result<crate::application::writeback::WritebackReport, ApplicationError> {
        let source = self.source.as_ref().ok_or_else(|| {
            ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: "writeback_resource requires an explicit SourceContext; current working directory must not be used".to_string(),
            }
        })?;
        if source.config.sources.is_empty() {
            return Err(ApplicationError::Storage {
                kind: StorageErrorKind::NoSourceRegistered,
                message: format!("no sources registered in source `{}`", source.source_id),
            });
        }
        let src_cfg = source
            .config
            .sources
            .iter()
            .find(|s| s.id == source_id)
            .cloned()
            .ok_or_else(|| ApplicationError::SourceNotFound {
                source_id: source_id.to_string(),
            })?;
        let adapter: Box<dyn crate::source::SourceAdapter> = self
            .source_registry
            .build(src_cfg.into_source_config())
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        // Honesty gate: never report a committed write against a source
        // whose adapter does not actually support writes.
        if !adapter.capabilities().can_write {
            return Err(ApplicationError::ReadOnlySource {
                source_id: source_id.to_string(),
            });
        }
        let prep =
            adapter
                .prepare_write(target_ref, payload)
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::InvalidState,
                    message: e.to_string(),
                })?;
        let commit_res = adapter
            .commit_write(&prep)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;

        Ok(crate::application::writeback::WritebackReport {
            target_ref: commit_res.target_ref,
            committed: commit_res.committed,
        })
    }

    /// Unified write pipe for whole-document content updates.
    ///
    /// Resolves the target row by `source_id` + `locator` (document
    /// kind only), enforces the expected-revision guard (content hash
    /// of the raw bytes, empty/None = no precondition), journals a
    /// [`ChangeOp::Writeback`](crate::domain::change::ChangeOp) Change,
    /// performs an atomic tmp+rename filesystem write, and refreshes
    /// the projection with a full native rescan (which journals its
    /// own Scan Changes).
    pub fn update_document(
        &mut self,
        source_id: &str,
        locator: &str,
        content: &str,
        expected_revision: Option<&str>,
    ) -> Result<DocumentUpdateReport, ApplicationError> {
        use crate::application::use_cases::{ResourceUseCase, ScanUseCase};

        let space_root = self.require_space_root()?;
        let page = <Self as ResourceUseCase>::query(
            self,
            &Selector::new().with_source(source_id),
        )?;
        let full = space_root.join(locator);
        let res = page
            .items
            .into_iter()
            .find(|r| r.kind == ResourceKind::Document && r.locator == locator)
            .ok_or_else(|| ApplicationError::Io {
                path: Some(full.clone()),
                source: std::io::ErrorKind::NotFound,
            })?;
        let canonical_space =
            std::fs::canonicalize(&space_root).unwrap_or_else(|_| space_root.clone());
        let canonical_file = std::fs::canonicalize(&full).unwrap_or_else(|_| full.clone());
        if !canonical_file.starts_with(&canonical_space) || !full.is_file() {
            return Err(ApplicationError::Io {
                path: Some(full.clone()),
                source: std::io::ErrorKind::NotFound,
            });
        }

        // Revision guard against the current on-disk bytes.
        let current_bytes = std::fs::read(&full).map_err(|e| ApplicationError::Io {
            path: Some(full.clone()),
            source: e.kind(),
        })?;
        let current_revision = sha256_hex(&current_bytes);
        if let Some(expected) = expected_revision.filter(|e| !e.is_empty())
            && expected != current_revision
        {
            return Err(ApplicationError::RevisionConflict {
                expected: expected.to_string(),
                actual: current_revision,
            });
        }

        let journaling = crate::application::projector::Journaling::from_parts(
            self.journal.as_ref(),
            self.audit.as_ref(),
            self.actor_principal(),
            self.now_unix_millis(),
        );
        journaling
            .record_writeback(
                source_id,
                res.r#ref,
                expected_revision.map(str::to_string),
                serde_json::json!({
                    "locator": locator,
                    "new_content_bytes": content.len(),
                }),
            )
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;

        // Atomic write: same-directory temp file + rename. Keeps the old
        // content intact if the write fails midway.
        let new_bytes = content.as_bytes();
        let tmp = full.with_extension("notez-tmp");
        std::fs::write(&tmp, new_bytes).map_err(|e| ApplicationError::Io {
            path: Some(tmp.clone()),
            source: e.kind(),
        })?;
        std::fs::rename(&tmp, &full).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            ApplicationError::Io {
                path: Some(full.clone()),
                source: e.kind(),
            }
        })?;

        // Refresh the projection from the new content (journals Scan).
        <Self as ScanUseCase>::scan_native(self)?;

        // Re-read the updated row for its fresh revision.
        let updated = <Self as ResourceUseCase>::read(self, &res.r#ref)?
            .ok_or_else(|| ApplicationError::NotFound {
                kind: res.kind,
                r_ref: res.r#ref,
            })?;
        Ok(DocumentUpdateReport {
            r_ref: updated.r#ref.to_string(),
            locator: updated.locator.clone(),
            revision: updated.revision.clone(),
        })
    }
    pub fn relay_sync(
        &self,
    ) -> Result<crate::application::writeback::RelaySyncReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::relay_sync(self)
    }

    pub fn upsert_resource(&mut self, resource: Resource) -> Result<(), ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::upsert_resource(self, resource)
    }
    pub fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::delete_resource(self, r_ref)
    }
    pub fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::query(self, selector)
    }
    pub fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::read(self, r_ref)
    }
    pub fn list_recent(&self, limit: usize) -> Result<Vec<Resource>, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::list_recent(self, limit)
    }
    pub fn list_by_source(
        &self,
        source_id: &str,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::list_by_source(
            self, source_id, limit,
        )
    }
    pub fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::resolve(self, query_str)
    }
    pub fn resolve_address(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::resolve_address(self, address)
    }

    pub fn rebuild(&mut self) -> Result<ScanReport, ApplicationError> {
        self.store.clear().map_err(|e| ApplicationError::Storage {
            kind: StorageErrorKind::Sqlite,
            message: e.to_string(),
        })?;
        self.scan_native()
    }
}
impl<S> Engine<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    pub fn journal_activity(&self, limit: usize) -> Result<Vec<crate::domain::journal::ActivityRecord>, ApplicationError> {
        self.journal.activity(limit).map_err(|e| ApplicationError::Storage {
            kind: StorageErrorKind::Sqlite,
            message: e.to_string(),
        })
    }
}

/// SHA-256 hex digest of raw bytes — the revision scheme for raw
/// document content, matching the web `update_document` contract.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

