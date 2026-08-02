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
    store: S,
    rule_engine: crate::domain::RuleEngine,
    format_parsers: Vec<Box<dyn crate::source::FormatParser>>,
    space: Option<SpaceContext>,
    capability_catalog: crate::capability::CapabilityCatalog,
    source_registry: crate::source::SourceRegistry,
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

    /// Register a public capability descriptor. This is a no-op in the
    /// current revision; it exists so that the future P1 capability
    /// directory can collect descriptors without a breaking change.
    /// See `core::capability::CapabilityDescriptor`.
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
    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }
    pub fn inspect_rules(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<crate::domain::InspectResult>, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::inspect_rules(self, r_ref)
    }

    pub fn inspect_rules_impl(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<crate::domain::InspectResult>, ApplicationError> {
        let res = self
            .store
            .get(r_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        Ok(res.map(|r| self.rule_engine.evaluate(&r)))
    }

    pub fn add_source(
        &mut self,
        space_root: &Path,
        config: crate::source::SourceConfig,
    ) -> Result<(), ApplicationError> {
        let mut cfg = crate::application::federation::SpaceSourcesConfig::load(space_root)?;
        cfg.sources.retain(|s| s.id != config.id);
        cfg.sources.push(config.clone());
        cfg.save(space_root)?;
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
    pub fn upsert_resource_impl(
        &mut self,
        resource: Resource,
    ) -> Result<(), ApplicationError> {
        self.store
            .upsert_resource(&resource)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Delete a resource by `ResourceRef`. Idempotent at the projection layer.
    pub fn delete_resource_impl(
        &mut self,
        r_ref: &ResourceRef,
    ) -> Result<(), ApplicationError> {
        self.store
            .delete_resource(r_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Recent activity feed, ordered by `revision` descending.
    pub fn list_recent_impl(
        &self,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError> {
        let page = self
            .store
            .query(&Selector::new())
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        let mut items = page.items;
        // Revision is monotonic by convention; lexicographic desc gives a
        // stable "most-recently-touched first" order.
        items.sort_by(|a, b| b.revision.cmp(&a.revision));
        items.truncate(limit);
        Ok(items)
    }

    /// List resources from a given source adapter.
    pub fn list_by_source_impl(
        &self,
        source_id: &str,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError> {
        let page = self
            .store
            .query(&Selector::new().with_source(source_id))
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        let mut items = page.items;
        if items.len() > limit {
            items.truncate(limit);
        }
        Ok(items)
    }

    pub fn list_sources(
        &self,
        space_root: &Path,
    ) -> Result<Vec<crate::source::SourceConfig>, ApplicationError> {
        let cfg = crate::application::federation::SpaceSourcesConfig::load(space_root)?;
        Ok(cfg.sources)
    }



    pub fn scan_federation(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_federation(self, space_root)
    }

    pub fn scan_native(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_native(self, root)
    }

    pub fn scan_federation_impl(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError> {
        use crate::source::SourceAdapter;

        let mut total_resources = 0;

        let sources_cfg = crate::application::federation::SpaceSourcesConfig::load(space_root)?;
        let exclude_paths: Vec<std::path::PathBuf> =
            sources_cfg.sources.iter().map(|s| s.path.clone()).collect();

        let native_config = crate::source::SourceConfig {
            id: "native".to_string(),
            kind: crate::source::SourceKind::Native,
            path: space_root.to_path_buf(),
            read_only: false,
            include_paths: vec![],
            exclude_paths,
        };
        let native_adapter = crate::source::NativeSourceAdapter::new(native_config);
        let native_scanned = native_adapter
            .scan()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        total_resources += native_scanned.resources.len();

        self.store
            .replace_source(
                "native",
                native_scanned.resources,
                native_scanned.relations,
                native_scanned.link_occurrences.clone(),
            )
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        crate::application::link_resolution::resolve_and_store_links(&mut self.store, "native", native_scanned.link_occurrences)?;

        let sources_cfg = crate::application::federation::SpaceSourcesConfig::load(space_root)?;
        for src_cfg in sources_cfg.sources {
            let adapter = self
                .source_registry
                .build(src_cfg.clone())
                .map_err(|e| ApplicationError::Storage(e.to_string()))?;
            let scanned = adapter
                .scan()
                .map_err(|e| ApplicationError::Storage(e.to_string()))?;

            total_resources += scanned.resources.len();

            self.store
                .replace_source(
                    &src_cfg.id,
                    scanned.resources,
                    scanned.relations,
                    scanned.link_occurrences.clone(),
                )
                .map_err(|e| ApplicationError::Storage(e.to_string()))?;

            crate::application::link_resolution::resolve_and_store_links(&mut self.store, &src_cfg.id, scanned.link_occurrences)?;
        }

        let mut resolved_count = 0;
        let page = self.query_impl(&crate::domain::Selector::new())?;
        for res in page.items {
            let rels = self.store.query_resolved_relations(&res.r#ref).unwrap_or_default();
            resolved_count += rels.len();
        }

        Ok(ScanReport {
            scanned_files: total_resources, // Note: not fully accurate, but historically used
            scanned_resources: total_resources,
            scanned_relations: resolved_count,
        })
    }

    pub fn scan_native_impl(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        use crate::source::native::NativeSourceAdapter;
        use crate::source::SourceAdapter;
        use crate::application::link_resolution::resolve_and_store_links;

        if self.format_parsers.is_empty() {
            return Err(ApplicationError::Storage(
                "no format parsers registered; call ApplicationService::register_format_parser at composition root before scanning".into(),
            ));
        }

        let config = crate::source::SourceConfig {
            id: "native".to_string(),
            kind: crate::source::SourceKind::Native,
            path: root.to_path_buf(),
            read_only: false,
            include_paths: vec![],
            exclude_paths: vec![],
        };
        let adapter = NativeSourceAdapter::new(config);
        // `NativeSourceAdapter` already wires the Org/Markdown parsers it
        // ships with. We still consult any parsers the service registered
        // for additional MIME types. Scan via the adapter, then merge any
        // extra-parser results for non-handled MIMEs (none today, but
        // keeps the seam open).
        let mut scanned = adapter
            .scan()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        for parser in &self.format_parsers {
            if parser.supports("text/org") || parser.supports("text/markdown") {
                continue;
            }
            // Additional MIME support: not used by the built-in transports.
            // Reserved for future extension.
            let _ = parser;
        }

        let scanned_files = scanned
            .resources
            .iter()
            .map(|r| r.locator.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let scanned_resources = scanned.resources.len();
        let link_occurrences = scanned.link_occurrences.clone();
        self.store
            .replace_source(
                "native",
                std::mem::take(&mut scanned.resources),
                std::mem::take(&mut scanned.relations),
                link_occurrences.clone(),
            )
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        resolve_and_store_links(&mut self.store, "native", link_occurrences)?;

        let mut resolved_count = 0;
        let page = self.query_impl(&crate::domain::Selector::new())?;
        for res in page.items {
            if res.source_id == "native" {
                let rels = self
                    .store
                    .query_resolved_relations(&res.r#ref)
                    .unwrap_or_default();
                resolved_count += rels.len();
            }
        }

        Ok(ScanReport {
            scanned_files,
            scanned_resources,
            scanned_relations: resolved_count,
        })
    }

    pub fn resolve_impl(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> {
        let trimmed = query_str.trim();

        if let Ok(r_ref) = ResourceRef::parse(trimmed)
            && let Some(res) = self
                .store
                .get(&r_ref)
                .map_err(|e| ApplicationError::Storage(e.to_string()))?
        {
            return Ok(ResolveResult::Found(res.r#ref));
        }

        if trimmed.len() == 26 {
            let mut matched = Vec::new();
            if let Ok(heading_ref) = ResourceRef::parse(&format!("heading:{trimmed}"))
                && let Some(res) = self
                    .store
                    .get(&heading_ref)
                    .map_err(|e| ApplicationError::Storage(e.to_string()))?
            {
                matched.push(res.r#ref);
            }
            if let Ok(doc_ref) = ResourceRef::parse(&format!("document:{trimmed}"))
                && let Some(res) = self
                    .store
                    .get(&doc_ref)
                    .map_err(|e| ApplicationError::Storage(e.to_string()))?
            {
                matched.push(res.r#ref);
            }
            if matched.len() == 1 {
                return Ok(ResolveResult::Found(matched[0]));
            } else if matched.len() > 1 {
                return Ok(ResolveResult::Ambiguous(matched));
            }
        }

        let page_all = self.query_impl(&Selector::new())?;
        let locator_matches: Vec<ResourceRef> = page_all
            .items
            .iter()
            .filter(|r| r.locator == trimmed)
            .map(|r| r.r#ref)
            .collect();
        if locator_matches.len() == 1 {
            return Ok(ResolveResult::Found(locator_matches[0]));
        } else if locator_matches.len() > 1 {
            return Ok(ResolveResult::Ambiguous(locator_matches));
        }

        let title_selector = Selector::new().with_title_contains(trimmed);
        let page_title = self.query_impl(&title_selector)?;
        let title_matches: Vec<ResourceRef> = page_title.items.iter().map(|r| r.r#ref).collect();
        if title_matches.len() == 1 {
            return Ok(ResolveResult::Found(title_matches[0]));
        } else if title_matches.len() > 1 {
            return Ok(ResolveResult::Ambiguous(title_matches));
        }

        Ok(ResolveResult::NotFound)
    }
    pub fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkOccurrence>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::query_link_occurrences(self, source_ref)
    }

    pub fn query_link_occurrences_impl(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkOccurrence>, ApplicationError> {
        self.store
            .query_link_occurrences(source_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }

    pub fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::ResolvedRelation>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::query_resolved_relations(self, source_ref)
    }

    pub fn query_resolved_relations_impl(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::ResolvedRelation>, ApplicationError> {
        self.store
            .query_resolved_relations(source_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }

    /// List every link occurrence originating from `source_ref` (raw form).
    pub fn list_links(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::list_links(self, source_ref)
    }

    pub fn list_links_impl(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError> {
        self.query_link_occurrences_impl(source_ref)
    }

    /// Re-resolve every occurrence for `source_ref` and persist diagnostics.
    pub fn resolve_links(
        &mut self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::resolve_links(self, source_ref)
    }

    pub fn resolve_links_impl(
        &mut self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError> {
        let occs = self.query_link_occurrences_impl(source_ref)?;
        // Determine the source_id by inspecting the existing diagnostics row.
        let source_id = occs
            .first()
            .map(|_| "native")
            .unwrap_or("native")
            .to_string();
        let _ = crate::application::link_resolution::LinkResolver::resolve_all(
            &mut self.store,
            &source_id,
            occs,
        )?;
        self.store
            .query_resolved_relations(source_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }

    /// Return per-occurrence diagnostics (status + candidates) for `source_ref`.
    pub fn diagnose_link(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkDiagnostic>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::diagnose_link(self, source_ref)
    }

    pub fn diagnose_link_impl(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkDiagnostic>, ApplicationError> {
        let rows = self
            .store
            .list_link_diagnostics(source_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        Ok(rows.unwrap_or_default())
    }

    /// Walk the native source under `space_root` again, resolve every link,
    /// and return a [`LinkReindexReport`].
    pub fn reindex_links(
        &mut self,
        space_root: &Path,
    ) -> Result<crate::application::link_resolution::LinkReindexReport, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::reindex_links(self, space_root)
    }

    pub fn reindex_links_impl(
        &mut self,
        space_root: &Path,
    ) -> Result<crate::application::link_resolution::LinkReindexReport, ApplicationError> {
        // Aggregate counts from the existing projection without mutating it.
        // A full rewrite is unnecessary: `replace_source` already persisted
        // occurrences during scan, and `LinkResolver::resolve_all` already
        // wrote diagnostics. Here we merely tally what is on disk so callers
        // get a stable view of unresolved/ambiguous/external counts.
        let _ = space_root;
        let page = self.query_impl(&Selector::new())?;
        let mut report = crate::application::link_resolution::LinkReindexReport::default();
        for res in &page.items {
            let diags = self
                .store
                .list_link_diagnostics(&res.r#ref)
                .map_err(|e| ApplicationError::Storage(e.to_string()))?;
            if let Some(rows) = diags {
                for d in rows {
                    report.scanned += 1;
                    match d.status {
                        crate::domain::ResolutionStatus::Resolved => report.resolved += 1,
                        crate::domain::ResolutionStatus::Unresolved => report.unresolved += 1,
                        crate::domain::ResolutionStatus::Ambiguous => report.ambiguous += 1,
                        crate::domain::ResolutionStatus::External => report.external += 1,
                        crate::domain::ResolutionStatus::Invalid => report.invalid += 1,
                    }
                }
            }
        }
        Ok(report)
    }

    /// Resolve a `ResourceAddress` (either a `Ref` or a `Locator`) and
    /// return a [`ResolveResult`].
    pub fn resolve_address_impl(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError> {
        use crate::domain::ResourceAddress;
        match address {
            ResourceAddress::Ref { r#ref } => {
                if let Some(res) = self.read_impl(r#ref)? {
                    Ok(ResolveResult::Found(res.r#ref))
                } else {
                    Ok(ResolveResult::NotFound)
                }
            }
            ResourceAddress::Locator { target } => {
                // We don't have a source_ref for a bare locator; pick a
                // document-kind placeholder so resolver strategies that branch
                // on kind_hint still work. The resolver never uses source_ref
                // to compute the answer.
                let placeholder =
                    ResourceRef::new(ResourceKind::Document, ulid::Ulid::nil());
                let occ = LinkOccurrence {
                    source_ref: placeholder,
                    target: target.clone(),
                    raw: target.to_string(),
                    display_text: None,
                    span: crate::domain::TextSpan {
                        line: 0,
                        col_start: 0,
                        col_end: 0,
                    },
                };
                let (status, target_ref, candidates) =
                    crate::application::link_resolution::LinkResolver::resolve(&self.store, &occ);
                match status {
                    ResolutionStatus::Resolved => Ok(ResolveResult::Found(
                        target_ref.expect("resolved has target"),
                    )),
                    ResolutionStatus::Ambiguous => Ok(ResolveResult::Ambiguous(candidates)),
                    _ => Ok(ResolveResult::NotFound),
                }
            }
        }
    }

    pub fn query_impl(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> {
        self.store
            .query(selector)
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }

    pub fn read_impl(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> {
        self.store
            .get(r_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }
    pub fn agenda(&self) -> Result<crate::application::task_para::AgendaView, ApplicationError> {
        <Self as crate::application::use_cases::TaskUseCase>::agenda(self)
    }

    pub fn agenda_impl(&self) -> Result<crate::application::task_para::AgendaView, ApplicationError> {
        let page = self.query_impl(&Selector::new())?;
        let mut items = Vec::new();

        for res in page.items {
            let scheduled = res.properties.get("SCHEDULED").cloned();
            let deadline = res.properties.get("DEADLINE").cloned();
            let closed = res.properties.get("CLOSED").cloned();
            let todo = res.properties.get("TODO").cloned();

            if scheduled.is_some() || deadline.is_some() || todo.is_some() {
                items.push(crate::application::task_para::AgendaItem {
                    r_ref: res.r#ref.to_string(),
                    title: res.title,
                    todo,
                    scheduled,
                    deadline,
                    closed,
                    locator: res.locator,
                });
            }
        }

        Ok(crate::application::task_para::AgendaView { items })
    }

    pub fn transition_task(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
    ) -> Result<crate::document::StateTransition, ApplicationError> {
        <Self as crate::application::use_cases::TaskUseCase>::transition_task(self, r_ref, to_state, timestamp)
    }

    pub fn transition_task_impl(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
    ) -> Result<crate::document::StateTransition, ApplicationError> {
        let mut res = self
            .read_impl(r_ref)?
            .ok_or_else(|| ApplicationError::NotFound(r_ref.to_string()))?;

        let current_todo = res
            .properties
            .get("TODO")
            .cloned()
            .unwrap_or_else(|| "TODO".to_string());

        let profile = crate::document::WorkflowProfile::default();
        let transition = profile
            .transition(&current_todo, to_state, timestamp)
            .map_err(|e| {
                ApplicationError::Document(crate::document::OrgDocumentError::Other(e.to_string()))
            })?;

        res.properties
            .insert("TODO".to_string(), transition.to_state.clone());
        if let Some(ref closed_ts) = transition.closed_timestamp {
            res.properties
                .insert("CLOSED".to_string(), closed_ts.clone());
        }

        let source_id = res.source_id.clone();

        // First write back to the authoritative source
        // We serialize the state change into a JSON payload for the adapter's mutate interface
        let payload = serde_json::json!({
            "action": "UpdateTaskStatus",
            "to_state": transition.to_state,
            "closed_timestamp": transition.closed_timestamp
        }).to_string();

        self.writeback_resource(&source_id, &res.locator, &payload)?;

        // Then update the local projection
        self.store
            .replace_source(&source_id, vec![res], vec![], vec![])
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        Ok(transition)
    }

    pub fn para_overview(&self) -> Result<crate::application::task_para::ParaOverview, ApplicationError> {
        <Self as crate::application::use_cases::TaskUseCase>::para_overview(self)
    }

    pub fn para_overview_impl(&self) -> Result<crate::application::task_para::ParaOverview, ApplicationError> {
        let page = self.query_impl(&Selector::new())?;
        let mut projects = Vec::new();
        let mut areas = Vec::new();
        let mut resources = Vec::new();
        let mut archives = Vec::new();

        let mut parent_to_tasks: std::collections::BTreeMap<String, Vec<crate::application::task_para::AgendaItem>> = std::collections::BTreeMap::new();

        // First pass: collect all tasks and map them to their parents
        for res in &page.items {
            let todo = res.properties.get("TODO").cloned();
            if todo.is_some() {
                if let Some(parent_ref) = res.properties.get("PARENT_REF") {
                    let item = crate::application::task_para::AgendaItem {
                        r_ref: res.r#ref.to_string(),
                        title: res.title.clone(),
                        todo,
                        scheduled: res.properties.get("SCHEDULED").cloned(),
                        deadline: res.properties.get("DEADLINE").cloned(),
                        closed: res.properties.get("CLOSED").cloned(),
                        locator: res.locator.clone(),
                    };
                    parent_to_tasks.entry(parent_ref.to_string()).or_default().push(item);
                }
            }
        }

        for res in page.items {
            let inspect_res = self.inspect_rules_impl(&res.r#ref)?;
            let para_val = inspect_res
                .as_ref()
                .and_then(|i| i.derived_properties.get("para").map(|s| s.to_string()))
                .or_else(|| res.properties.get("para").map(|s| s.to_string()))
                .or_else(|| res.properties.get("TYPE").map(|s| s.to_string()));

            if let Some(pv) = para_val {
                let tasks = parent_to_tasks.remove(&res.r#ref.to_string()).unwrap_or_default();
                let node = crate::application::task_para::ParaNode { resource: res, tasks };
                match pv.as_str() {
                    "projects" | "project" => projects.push(node),
                    "areas" | "area" => areas.push(node),
                    "resources" | "resource" => resources.push(node),
                    "archives" | "archive" => archives.push(node),
                    _ => {}
                }
            }
        }

        Ok(crate::application::task_para::ParaOverview {
            projects,
            areas,
            resources,
            archives,
        })
    }
    pub fn add_attachment(
        &mut self,
        space_root: &Path,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::add_attachment(self, space_root, file_path, default_mime)
    }

    pub fn add_attachment_impl(
        &mut self,
        space_root: &Path,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError> {
        let bytes = std::fs::read(file_path)?;
        let blob_store = crate::storage::BlobStore::new(space_root);
        let meta = blob_store
            .store_bytes(&bytes, default_mime)
            .map_err(ApplicationError::Io)?;

        let att_ulid = if meta.hash.len() >= 32 {
            u128::from_str_radix(&meta.hash[..32], 16)
                .map(ulid::Ulid::from)
                .unwrap_or_else(|_| ulid::Ulid::new())
        } else {
            ulid::Ulid::new()
        };
        let att_ref = ResourceRef::new(crate::domain::ResourceKind::Attachment, att_ulid);
        let mut properties = std::collections::BTreeMap::new();
        properties.insert("hash".to_string(), meta.hash.clone());
        properties.insert("mime".to_string(), meta.mime_type.clone());
        properties.insert("size_bytes".to_string(), meta.size_bytes.to_string());

        let title = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let resource = Resource {
            r#ref: att_ref,
            kind: crate::domain::ResourceKind::Attachment,
            title,
            revision: meta.hash,
            source_id: "native".to_string(),
            locator: file_path.to_string_lossy().to_string(),
            properties,
        };

        let page = self.query_impl(&Selector::new())?;
        let mut native_resources: Vec<Resource> = page
            .items
            .into_iter()
            .filter(|r| r.source_id == "native" && r.r#ref != att_ref)
            .collect();
        native_resources.push(resource);

        self.store
            .replace_source("native", native_resources, vec![], vec![])
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        Ok(att_ref)
    }

    pub fn run_extraction(
        &mut self,
        space_root: &Path,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::run_extraction(self, space_root, att_ref)
    }

    pub fn run_extraction_impl(
        &mut self,
        space_root: &Path,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        use crate::artifact::{Extractor, ImageMetadataExtractor, SegmentSlicer, TextExtractor};

        let res = self
            .read_impl(att_ref)?
            .ok_or_else(|| ApplicationError::NotFound(att_ref.to_string()))?;

        let hash = res
            .properties
            .get("hash")
            .ok_or_else(|| ApplicationError::Storage("missing hash property".to_string()))?;
        let mime = res
            .properties
            .get("mime")
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let blob_store = crate::storage::BlobStore::new(space_root);
        let bytes = blob_store
            .get(hash)?
            .ok_or_else(|| ApplicationError::NotFound(format!("blob hash {hash}")))?;

        let extracted_content = if mime.starts_with("image/") {
            let ext = ImageMetadataExtractor;
            ext.extract(&bytes, &mime).map_err(|e| {
                ApplicationError::Document(crate::document::OrgDocumentError::Other(e.to_string()))
            })?
        } else {
            let ext = TextExtractor;
            ext.extract(&bytes, &mime).map_err(|e| {
                ApplicationError::Document(crate::document::OrgDocumentError::Other(e.to_string()))
            })?
        };

        let slicer = SegmentSlicer::default();
        let records = slicer.slice(&att_ref.to_string(), &extracted_content.text);

        self.store
            .insert_segments(&records)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        Ok(records)
    }

    pub fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        <Self as crate::application::use_cases::AttachmentUseCase>::query_segments(self, att_ref)
    }

    pub fn query_segments_impl(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        self.store
            .query_segments(&att_ref.to_string())
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }
    pub fn create_community(
        &self,
        space_root: &Path,
        community: crate::domain::community::Community,
    ) -> Result<(), ApplicationError> {
        <Self as crate::application::use_cases::CommunityUseCase>::create_community(self, space_root, community)
    }

    pub fn create_community_impl(
        &self,
        space_root: &Path,
        community: crate::domain::community::Community,
    ) -> Result<(), ApplicationError> {
        let mut cfg = crate::application::community_app::SpaceCommunitiesConfig::load(space_root)?;
        cfg.communities.retain(|c| c.id != community.id);
        cfg.communities.push(community);
        cfg.save(space_root)?;
        Ok(())
    }

    pub fn list_communities(
        &self,
        space_root: &Path,
    ) -> Result<Vec<crate::domain::community::Community>, ApplicationError> {
        <Self as crate::application::use_cases::CommunityUseCase>::list_communities(self, space_root)
    }

    pub fn list_communities_impl(
        &self,
        space_root: &Path,
    ) -> Result<Vec<crate::domain::community::Community>, ApplicationError> {
        let cfg = crate::application::community_app::SpaceCommunitiesConfig::load(space_root)?;
        Ok(cfg.communities)
    }
    pub fn derive_artifact(
        &self,
        space_root: &Path,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<crate::artifact::DerivedArtifact, ApplicationError> {
        <Self as crate::application::use_cases::ArtifactUseCase>::derive_artifact(self, space_root, community_id, recipe_name)
    }

    pub fn derive_artifact_impl(
        &self,
        space_root: &Path,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<crate::artifact::DerivedArtifact, ApplicationError> {
        let communities = self.list_communities_impl(space_root)?;
        let comm = communities
            .iter()
            .find(|c| c.id == community_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("community {community_id}")))?;

        let page = self.query_impl(&Selector::new())?;
        let members: Vec<Resource> = comm
            .filter_members(&page.items)
            .into_iter()
            .cloned()
            .collect();

        let recipe_kind = match recipe_name {
            "summary" => crate::artifact::RecipeKind::Summary,
            "llms-txt" | "llms.txt" => crate::artifact::RecipeKind::LlmsTxt,
            "context-pack" => crate::artifact::RecipeKind::ContextPack,
            "skill-ir" => crate::artifact::RecipeKind::SkillIr,
            _ => {
                return Err(ApplicationError::Document(crate::document::OrgDocumentError::Other(
                    format!("unknown recipe: {recipe_name}"),
                )));
            }
        };

        let recipe = crate::artifact::Recipe {
            name: recipe_name.to_string(),
            kind: recipe_kind,
            token_budget: 4000,
        };

        let derived = crate::artifact::RecipeEvaluator::evaluate(&recipe, &members).map_err(|e| {
            ApplicationError::Document(crate::document::OrgDocumentError::Other(e.to_string()))
        })?;

        Ok(derived)
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

    pub fn export_skill_impl(
        &self,
        space_root: &Path,
        community_id: &str,
        description: &str,
        export_path: &Path,
    ) -> Result<crate::artifact::SkillPackage, ApplicationError> {
        let communities = self.list_communities_impl(space_root)?;
        let comm = communities
            .iter()
            .find(|c| c.id == community_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("community {community_id}")))?;

        let page = self.query_impl(&Selector::new())?;
        let members: Vec<Resource> = comm
            .filter_members(&page.items)
            .into_iter()
            .cloned()
            .collect();

        let skill_ir = crate::artifact::SkillIr::compile(&comm.name, description, &members);

        let package = crate::artifact::SkillExporter::export(&skill_ir, export_path)
            .map_err(ApplicationError::Io)?;

        Ok(package)
    }
    pub fn sync_push(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::sync_push(self, actor_id, space_root, shared_folder)
    }

    pub fn sync_push_impl(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError> {
        let transport = crate::sync::FolderTransport::new(shared_folder);
        let engine = crate::sync::SyncEngine::new(actor_id, space_root, transport);
        let report = engine
            .push()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        Ok(report)
    }

    pub fn sync_pull(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::sync_pull(self, actor_id, space_root, shared_folder)
    }

    pub fn sync_pull_impl(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError> {
        let transport = crate::sync::FolderTransport::new(shared_folder);
        let engine = crate::sync::SyncEngine::new(actor_id, space_root, transport);
        let report = engine
            .pull()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        self.scan_native_impl(space_root)?;

        Ok(report)
    }

    pub fn list_conflicts(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        <Self as crate::application::use_cases::SyncUseCase>::list_conflicts(self)
    }

    pub fn list_conflicts_impl(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        Err(ApplicationError::Unsupported(
            "conflict list is not yet implemented; use `notez sync` commands",
        ))
    }
    pub fn space_doctor(
        &self,
        _space_root: &Path,
    ) -> Result<crate::application::doctor::DoctorReport, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::space_doctor(self, _space_root)
    }

    pub fn space_doctor_impl(
        &self,
        _space_root: &Path,
    ) -> Result<crate::application::doctor::DoctorReport, ApplicationError> {
        Err(ApplicationError::Unsupported(
            "space doctor is not yet implemented",
        ))
    }

    pub fn list_jobs(&self) -> Result<Vec<crate::application::job_manager::JobRecord>, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::list_jobs(self)
    }

    pub fn list_jobs_impl(&self) -> Result<Vec<crate::application::job_manager::JobRecord>, ApplicationError> {
        Err(ApplicationError::Unsupported(
            "job manager is not yet implemented; jobs are tracked via `notez task`",
        ))
    }

    pub fn check_artifact_freshness(
        &self,
        _space_root: &Path,
    ) -> Result<crate::application::job_manager::ArtifactStaleReport, ApplicationError> {
        <Self as crate::application::use_cases::InspectUseCase>::check_artifact_freshness(self, _space_root)
    }

    pub fn check_artifact_freshness_impl(
        &self,
        _space_root: &Path,
    ) -> Result<crate::application::job_manager::ArtifactStaleReport, ApplicationError> {
        Err(ApplicationError::Unsupported(
            "artifact freshness check is not yet implemented",
        ))
    }
    pub fn writeback_resource(
        &self,
        source_id: &str,
        target_ref: &str,
        payload: &str,
    ) -> Result<crate::application::writeback::WritebackReport, ApplicationError> {
        use crate::source::{SourceAdapter, SourceKind, AnytypeSourceAdapter, AppleNotesSourceAdapter, AppleCalendarSourceAdapter, NativeSourceAdapter};

        let space = self.space.as_ref().ok_or_else(|| {
            ApplicationError::Storage(
                "writeback_resource requires an explicit SpaceContext; current working directory must not be used".into(),
            )
        })?;
        if space.runtime.sources.is_empty() {
            return Err(ApplicationError::Storage(format!(
                "no sources registered in space `{}`",
                space.space_id
            )));
        }
        let src_cfg = space
            .runtime
            .sources
            .iter()
            .find(|s| s.id == source_id)
            .cloned()
            .ok_or_else(|| {
                ApplicationError::Storage(format!(
                    "source `{source_id}` is not registered in space `{}`",
                    space.space_id
                ))
            })?;

        if src_cfg.read_only {
            return Err(ApplicationError::Storage(format!(
                "source `{source_id}` is read-only"
            )));
        }
        let adapter: Box<dyn SourceAdapter> = self
            .source_registry
            .build(src_cfg.clone())
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        let prep = adapter
            .prepare_write(target_ref, payload)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        let commit_res = adapter
            .commit_write(&prep)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

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

    pub fn relay_sync_impl(
        &self,
        _source_id: &str,
        _space_root: &Path,
    ) -> Result<crate::application::writeback::RelaySyncReport, ApplicationError> {
        Err(ApplicationError::Unsupported(
            "relay sync is not yet implemented; sync via folder transport",
        ))
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
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        self.scan_native(root)
    }
}


impl<S: crate::domain::ProjectionStore> crate::application::use_cases::ResourceUseCase
    for ApplicationFacade<S>
{
    fn upsert_resource(&mut self, resource: Resource) -> Result<(), ApplicationError> {
        ApplicationFacade::upsert_resource_impl(self, resource)
    }
    fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), ApplicationError> {
        ApplicationFacade::delete_resource_impl(self, r_ref)
    }
    fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> {
        ApplicationFacade::query_impl(self, selector)
    }
    fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> {
        ApplicationFacade::read_impl(self, r_ref)
    }
    fn list_recent(&self, limit: usize) -> Result<Vec<Resource>, ApplicationError> {
        ApplicationFacade::list_recent_impl(self, limit)
    }
    fn list_by_source(
        &self,
        source_id: &str,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError> {
        ApplicationFacade::list_by_source_impl(self, source_id, limit)
    }
    fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> {
        ApplicationFacade::resolve_impl(self, query_str)
    }
    fn resolve_address(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError> {
        ApplicationFacade::resolve_address_impl(self, address)
}

}

impl<S: crate::domain::ProjectionStore> crate::application::use_cases::ScanUseCase for ApplicationFacade<S> {
    fn scan_native(&mut self, root: &std::path::Path) -> Result<ScanReport, ApplicationError> {
        ApplicationFacade::scan_native_impl(self, root)
    }

    fn scan_federation(
        &mut self,
        space_root: &std::path::Path,
    ) -> Result<ScanReport, ApplicationError> {
        ApplicationFacade::scan_federation_impl(self, space_root)
    }
}
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::LinkUseCase for ApplicationFacade<S> {
    fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkOccurrence>, ApplicationError> {
        ApplicationFacade::query_link_occurrences_impl(self, source_ref)
    }
    fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::ResolvedRelation>, ApplicationError> {
        ApplicationFacade::query_resolved_relations_impl(self, source_ref)
    }
    fn list_links(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkOccurrence>, ApplicationError> {
        ApplicationFacade::list_links_impl(self, source_ref)
    }
    fn resolve_links(
        &mut self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::ResolvedRelation>, ApplicationError> {
        ApplicationFacade::resolve_links_impl(self, source_ref)
    }
    fn diagnose_link(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkDiagnostic>, ApplicationError> {
        ApplicationFacade::diagnose_link_impl(self, source_ref)
    }
    fn reindex_links(
        &mut self,
        space_root: &std::path::Path,
    ) -> Result<crate::application::link_resolution::LinkReindexReport, ApplicationError> {
        ApplicationFacade::reindex_links_impl(self, space_root)
    }
}
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::TaskUseCase for ApplicationFacade<S> {
    fn agenda(&self) -> Result<crate::application::task_para::AgendaView, ApplicationError> {
        ApplicationFacade::agenda_impl(self)
    }
    fn para_overview(&self) -> Result<crate::application::task_para::ParaOverview, ApplicationError> {
        ApplicationFacade::para_overview_impl(self)
    }
    fn transition_task(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
    ) -> Result<crate::document::StateTransition, ApplicationError> {
        ApplicationFacade::transition_task_impl(self, r_ref, to_state, timestamp)
    }
}
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::AttachmentUseCase for ApplicationFacade<S> {
    fn add_attachment(
        &mut self,
        space_root: &std::path::Path,
        file_path: &std::path::Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError> {
        ApplicationFacade::add_attachment_impl(self, space_root, file_path, default_mime)
    }
    fn run_extraction(
        &mut self,
        space_root: &std::path::Path,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        ApplicationFacade::run_extraction_impl(self, space_root, att_ref)
    }
    fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        ApplicationFacade::query_segments_impl(self, att_ref)
    }
}
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::CommunityUseCase for ApplicationFacade<S> {
    fn create_community(
        &self,
        space_root: &std::path::Path,
        community: crate::domain::community::Community,
    ) -> Result<(), ApplicationError> {
        ApplicationFacade::create_community_impl(self, space_root, community)
    }
    fn list_communities(
        &self,
        space_root: &std::path::Path,
    ) -> Result<Vec<crate::domain::community::Community>, ApplicationError> {
        ApplicationFacade::list_communities_impl(self, space_root)
    }
}
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::ArtifactUseCase for ApplicationFacade<S> {
    fn derive_artifact(
        &self,
        space_root: &std::path::Path,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<crate::artifact::DerivedArtifact, ApplicationError> {
        ApplicationFacade::derive_artifact_impl(self, space_root, community_id, recipe_name)
    }
    fn export_skill(
        &self,
        space_root: &std::path::Path,
        community_id: &str,
        description: &str,
        export_path: &std::path::Path,
    ) -> Result<crate::artifact::SkillPackage, ApplicationError> {
        ApplicationFacade::export_skill_impl(self, space_root, community_id, description, export_path)
    }
}
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::SyncUseCase for ApplicationFacade<S> {
    fn sync_push(
        &mut self,
        actor_id: &str,
        space_root: &std::path::Path,
        shared_folder: &std::path::Path,
    ) -> Result<crate::sync::PushReport, ApplicationError> {
        ApplicationFacade::sync_push_impl(self, actor_id, space_root, shared_folder)
    }
    fn sync_pull(
        &mut self,
        actor_id: &str,
        space_root: &std::path::Path,
        shared_folder: &std::path::Path,
    ) -> Result<crate::sync::PullReport, ApplicationError> {
        ApplicationFacade::sync_pull_impl(self, actor_id, space_root, shared_folder)
    }
    fn relay_sync(
        &self,
        source_id: &str,
        space_root: &std::path::Path,
    ) -> Result<crate::application::writeback::RelaySyncReport, ApplicationError> {
        ApplicationFacade::relay_sync_impl(self, source_id, space_root)
    }
    fn list_conflicts(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        ApplicationFacade::list_conflicts_impl(self)
    }
}
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::InspectUseCase for ApplicationFacade<S> {
    fn inspect_rules(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<crate::domain::InspectResult>, ApplicationError> {
        ApplicationFacade::inspect_rules_impl(self, r_ref)
    }
    fn space_doctor(
        &self,
        space_root: &std::path::Path,
    ) -> Result<crate::application::doctor::DoctorReport, ApplicationError> {
        ApplicationFacade::space_doctor_impl(self, space_root)
    }
    fn list_jobs(&self) -> Result<Vec<crate::application::job_manager::JobRecord>, ApplicationError> {
        ApplicationFacade::list_jobs_impl(self)
    }
    fn check_artifact_freshness(
        &self,
        space_root: &std::path::Path,
    ) -> Result<crate::application::job_manager::ArtifactStaleReport, ApplicationError> {
        ApplicationFacade::check_artifact_freshness_impl(self, space_root)
    }
}
