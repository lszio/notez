use crate::application::context::SpaceContext;
use crate::domain::{
    LinkDiagnostic, LinkOccurrence, ProjectionStore, QueryPage, ResolvedRelation,
    ResolutionStatus, Resource, ResourceKind, ResourceRef, Selector,
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
            ApplicationError::AddressUniqueness { addr, existing, candidate } => {
                // ResourceAddress has no Display impl yet; render the
                // bound refs (which are Copy) instead.
                let _ = addr;
                write!(
                    f,
                    "address uniqueness: already bound to {existing}; cannot rebind to {candidate}"
                )
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
            ApplicationError::AddressUniqueness { addr, existing, candidate } => {
                map.serialize_entry("kind", "address_uniqueness")?;
                map.serialize_entry("addr", addr)?;
                map.serialize_entry("existing", existing)?;
                map.serialize_entry("candidate", candidate)?;
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
                let path = serde_json::from_value(
                    value
                        .get("path")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )
                .map_err(D::Error::custom)?;
                let source = value
                    .get("source")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `source`"))?;
                Ok(ApplicationError::Io {
                    path,
                    source: crate::error_serde::from_name(source),
                })
            }
            "unsupported_capability" => {
                let capability = value
                    .get("capability")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `capability`"))?;
                // `capability` is `&'static str`; only the strings this
                // build actually produces can be round-tripped. Unknown
                // strings are rejected rather than leaked.
                const KNOWN: &[&str] = &[
                    "conflict list is not yet implemented; use `notez sync` commands",
                    "space doctor is not yet implemented",
                    "job manager is not yet implemented; jobs are tracked via `notez task`",
                    "artifact freshness check is not yet implemented",
                    "relay sync is not yet implemented; sync via folder transport",
                ];
                let capability = KNOWN
                    .iter()
                    .find(|k| **k == capability)
                    .copied()
                    .ok_or_else(|| {
                        D::Error::custom("ApplicationError: unknown unsupported capability string")
                    })?;
                Ok(ApplicationError::UnsupportedCapability { capability })
            }
            "revision_conflict" => {
                let expected = value
                    .get("expected")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `expected`"))?;
                let actual = value
                    .get("actual")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `actual`"))?;
                Ok(ApplicationError::RevisionConflict {
                    expected: serde_json::from_value(expected.clone()).map_err(D::Error::custom)?,
                    actual: serde_json::from_value(actual.clone()).map_err(D::Error::custom)?,
                })
            }
            "read_only_source" => {
                let source_id = value
                    .get("source_id")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `source_id`"))?;
                Ok(ApplicationError::ReadOnlySource {
                    source_id: serde_json::from_value(source_id.clone()).map_err(D::Error::custom)?,
                })
            }
            "source_not_found" => {
                let source_id = value
                    .get("source_id")
                    .ok_or_else(|| D::Error::custom("ApplicationError: missing `source_id`"))?;
                Ok(ApplicationError::SourceNotFound {
                    source_id: serde_json::from_value(source_id.clone()).map_err(D::Error::custom)?,
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
                    candidate: serde_json::from_value(candidate.clone()).map_err(D::Error::custom)?,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveResult {
    Found(ResourceRef),
    NotFound,
    Ambiguous(Vec<ResourceRef>),
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub scanned_files: usize,
    pub scanned_resources: usize,
    pub scanned_relations: usize,
}

pub struct ApplicationFacade<S: ProjectionStore> {
    pub(crate) store: S,
    pub(crate) rule_engine: crate::domain::RuleEngine,
    pub(crate) format_parsers: Vec<Box<dyn crate::source::FormatParser>>,
    pub(crate) space: Option<SpaceContext>,
    pub(crate) capability_catalog: crate::capability::CapabilityCatalog,
    pub(crate) source_registry: crate::source::SourceRegistry,
}

/// Backwards-compatible alias for [`ApplicationFacade`]. New code should
/// refer to `ApplicationFacade` directly; the alias is preserved so
/// downstream consumers can keep their imports stable across the rename.
pub type ApplicationService<S = crate::storage::SqliteProjection> = ApplicationFacade<S>;

impl<S: ProjectionStore> ApplicationFacade<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            space: None,
            capability_catalog: crate::capability::CapabilityCatalog::with_builtins(),
            source_registry: crate::source::SourceRegistry::with_builtins(),
        }
    }

    /// Construct an `ApplicationFacade` bound to an explicit
    /// [`SpaceContext`]. The space is the single source of truth for the
    /// service's filesystem root and resolved configuration; the service
    /// will refuse mutation paths that depend on the process working
    /// directory.
    pub fn with_space(store: S, space: SpaceContext) -> Self {
        Self {
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            space: Some(space),
            capability_catalog: crate::capability::CapabilityCatalog::with_builtins(),
            source_registry: crate::source::SourceRegistry::with_builtins(),
        }
    }

    /// Construct an `ApplicationFacade` with a caller-supplied source
    /// factory registry. The default constructors register the six
    /// built-in factories; this constructor is for tests and for
    /// third-party compositions that want to start from an empty
    /// registry or one with custom factories pre-registered.
    pub fn with_registry(
        store: S,
        registry: crate::source::SourceRegistry,
    ) -> Self {
        Self {
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            space: None,
            capability_catalog: crate::capability::CapabilityCatalog::with_builtins(),
            source_registry: registry,
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
    pub fn space(&self) -> Option<&SpaceContext> {
        self.space.as_ref()
    }

    /// Register a `FormatParser` for use by future scans. Callers (typically
    /// the composition root) are responsible for registering every parser
    /// the runtime needs. Without registrations, scans against a source
    /// whose transport yields non-Org/Orphan-Entity MIME bytes will fail
    /// with `ParserNotFound`.
    pub fn register_format_parser(
        &mut self,
        parser: Box<dyn crate::source::FormatParser>,
    ) {
        self.format_parsers.push(parser);
    }

    /// Register a public capability descriptor. The descriptor is
    /// inserted into (or replaces the entry in) the active
    /// [`CapabilityCatalog`]; subsequent calls to
    /// [`ApplicationFacade::capability_catalog`] and the
    /// `capabilities_json` helper observe the change. This is the
    /// canonical write path used by third-party composition roots to
    /// surface custom `UseCase` traits via the same CLI/MCP listing.
    pub fn register_capability(
        &mut self,
        descriptor: &crate::capability::CapabilityDescriptor,
    ) {
        self.capability_catalog.register(descriptor.clone());
    }

    /// Borrow the active capability catalog. Used by CLI help text,
    /// MCP tool listings, and documentation generators to surface a
    /// single source of truth for what the facade can do.
    pub fn capability_catalog(&self) -> &crate::capability::CapabilityCatalog {
        &self.capability_catalog
    }

    /// Render the active capability catalog as a JSON array using the
    /// shared [`crate::capability::catalog_to_json_array`] helper. This
    /// is the canonical "list capabilities" payload used by both the
    /// CLI `list-capabilities` subcommand and the MCP
    /// `list_capabilities` tool, so the two surfaces stay byte-equal.
    pub fn capabilities_json(&self) -> serde_json::Value {
        crate::capability::catalog_to_json_array(&self.capability_catalog)
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }
    pub fn store(&self) -> &S {
        &self.store
    }
    pub fn inspect_rules(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<crate::domain::InspectResult>, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::inspect_rules(self, r_ref)
    }


    pub fn add_source(
        &mut self,
        space_root: &Path,
        config: crate::source::SourceConfig,
    ) -> Result<(), ApplicationError> {
        let mut cfg = crate::application::federation::SpaceSourcesConfig::load(space_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        cfg.sources.retain(|s| s.id != config.id);
        cfg.sources.push(config.clone());
        cfg.save(space_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        // Mirror the change into the in-memory SpaceContext so subsequent
        // operations (e.g. `writeback_resource`, `transition_task`) see
        // the new source without needing to reload the space.
        if let Some(space) = self.space.as_mut() {
            space.runtime.sources.retain(|s| s.id != config.id);
            space.runtime.sources.push(config);
        }
        Ok(())
    }

    /// Persist a new or updated resource. The projection is updated
    /// atomically; the resource's source adapter (when present and writable)
    /// is invoked so the authoritative backing store stays in sync.

    /// Delete a resource by `ResourceRef`. Idempotent at the projection layer.

    /// Recent activity feed, ordered by `revision` descending.

    /// List resources from a given source adapter.

    pub fn list_sources(
        &self,
        space_root: &Path,
    ) -> Result<Vec<crate::source::SourceConfig>, ApplicationError> {
        let cfg = crate::application::federation::SpaceSourcesConfig::load(space_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        Ok(cfg.sources)
    }



    pub fn scan_federation(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_federation(self, space_root)
    }

    pub fn scan_native(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_native(self, root)
    }


    pub fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkOccurrence>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::query_link_occurrences(self, source_ref)
    }


    pub fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::ResolvedRelation>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::query_resolved_relations(self, source_ref)
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


    /// Walk the native source under `space_root` again, resolve every link,
    /// and return a [`LinkReindexReport`].
    pub fn reindex_links(
        &mut self,
        space_root: &Path,
    ) -> Result<crate::application::link_resolution::LinkReindexReport, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::reindex_links(self, space_root)
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
        <Self as crate::application::use_cases::TaskUseCase>::transition_task(self, r_ref, to_state, timestamp)
    }


    pub fn para_overview(&self) -> Result<crate::application::task_para::ParaOverview, ApplicationError> {
        <Self as crate::application::use_cases::TaskUseCase>::para_overview(self)
    }

    pub fn add_attachment(
        &mut self,
        space_root: &Path,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::add_attachment(self, space_root, file_path, default_mime)
    }


    pub fn run_extraction(
        &mut self,
        space_root: &Path,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::run_extraction(self, space_root, att_ref)
    }


    pub fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::query_segments(self, att_ref)
    }

    pub fn create_community(
        &self,
        space_root: &Path,
        community: crate::domain::community::Community,
    ) -> Result<(), ApplicationError> {
        <Self as crate::application::use_cases::CommunityUseCase>::create_community(self, space_root, community)
    }


    pub fn list_communities(
        &self,
        space_root: &Path,
    ) -> Result<Vec<crate::domain::community::Community>, ApplicationError> {
        <Self as crate::application::use_cases::CommunityUseCase>::list_communities(self, space_root)
    }

    pub fn derive_artifact(
        &self,
        space_root: &Path,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<crate::artifact::DerivedArtifact, ApplicationError> {
        <Self as crate::application::use_cases::ArtifactUseCase>::derive_artifact(self, space_root, community_id, recipe_name)
    }


    pub fn export_skill(
        &self,
        space_root: &Path,
        community_id: &str,
        description: &str,
        export_path: &Path,
    ) -> Result<crate::artifact::SkillPackage, ApplicationError> {
        <Self as crate::application::use_cases::ArtifactUseCase>::export_skill(self, space_root, community_id, description, export_path)
    }

    pub fn sync_push(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::sync_push(self, actor_id, space_root, shared_folder)
    }


    pub fn sync_pull(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::sync_pull(self, actor_id, space_root, shared_folder)
    }


    pub fn list_conflicts(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::list_conflicts(self)
    }

    pub fn space_doctor(
        &self,
        _space_root: &Path,
    ) -> Result<crate::application::doctor::DoctorReport, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::space_doctor(self, _space_root)
    }


    pub fn list_jobs(&self) -> Result<Vec<crate::application::job_manager::JobRecord>, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::list_jobs(self)
    }


    pub fn check_artifact_freshness(
        &self,
        _space_root: &Path,
    ) -> Result<crate::application::job_manager::ArtifactStaleReport, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::check_artifact_freshness(self, _space_root)
    }

    pub fn writeback_resource(
        &self,
        source_id: &str,
        target_ref: &str,
        payload: &str,
    ) -> Result<crate::application::writeback::WritebackReport, ApplicationError> {
        use crate::source::{SourceAdapter, SourceKind, AnytypeSourceAdapter, AppleNotesSourceAdapter, AppleCalendarSourceAdapter, NativeSourceAdapter};

        let space = self.space.as_ref().ok_or_else(|| {
            ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: "writeback_resource requires an explicit SpaceContext; current working directory must not be used".to_string(),
            }
        })?;
        if space.runtime.sources.is_empty() {
            return Err(ApplicationError::Storage {
                kind: StorageErrorKind::NoSourceRegistered,
                message: format!(
                    "no sources registered in space `{}`",
                    space.space_id
                ),
            });
        }
        let src_cfg = space
            .runtime
            .sources
            .iter()
            .find(|s| s.id == source_id)
            .cloned()
            .ok_or_else(|| ApplicationError::SourceNotFound {
                source_id: source_id.to_string(),
            })?;

        if src_cfg.read_only {
            return Err(ApplicationError::ReadOnlySource {
                source_id: source_id.to_string(),
            });
        }
        let adapter: Box<dyn SourceAdapter> = self
            .source_registry
            .build(src_cfg.clone())
            .map_err(|e| {
                ApplicationError::Storage {
                    kind: StorageErrorKind::InvalidState,
                    message: e.to_string(),
                }
            })?;

        let prep = adapter
            .prepare_write(target_ref, payload)
            .map_err(|e| {
                ApplicationError::Storage {
                    kind: StorageErrorKind::InvalidState,
                    message: e.to_string(),
                }
            })?;
        let commit_res = adapter
            .commit_write(&prep)
            .map_err(|e| {
                ApplicationError::Storage {
                    kind: StorageErrorKind::InvalidState,
                    message: e.to_string(),
                }
            })?;

        Ok(crate::application::writeback::WritebackReport {
            target_ref: commit_res.target_ref,
            committed: commit_res.committed,
        })
    }

    pub fn relay_sync(
        &self,
        _source_id: &str,
        _space_root: &Path,
    ) -> Result<crate::application::writeback::RelaySyncReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::relay_sync(self, _source_id, _space_root)
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
    pub fn list_by_source(&self, source_id: &str, limit: usize) -> Result<Vec<Resource>, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::list_by_source(self, source_id, limit)
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

    pub fn rebuild(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        self.store
            .clear()
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        self.scan_native(root)
    }
}



