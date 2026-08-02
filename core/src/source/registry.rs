//! `SourceAdapterFactory` and `SourceRegistry`.
//!
//! Each `SourceKind` value is associated with a [`SourceAdapterFactory`]
//! that knows how to construct a [`SourceAdapter`] from a
//! [`SourceConfig`]. Built-in factories cover the six native kinds
//! (`Native`, `Git`, `Obsidian`, `Anytype`, `AppleNotes`,
//! `AppleCalendar`). Third-party crates can register a factory for a
//! custom `SourceKind::Other(String)` without modifying `core`.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::source::adapter::{
    ComposedSourceAdapter, ScannedSource, SourceAdapter, SourceCapabilities, SourceConfig,
    SourceError, SourceKind,
};
use crate::source::protocol::{FormatParser, RawEntity, SourceTransport, TransportError};

/// Builds a [`SourceAdapter`] for a single `SourceKind`.
pub trait SourceAdapterFactory: Send + Sync {
    /// The `SourceKind` this factory serves.
    fn kind(&self) -> SourceKind;

    /// Construct the adapter for `config`. Returning an error here is
    /// the documented way to surface unsupported configuration at
    /// composition time.
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError>;
}

/// In-process registry mapping `SourceKind` → `SourceAdapterFactory`.
#[derive(Default)]
pub struct SourceRegistry {
    factories: HashMap<SourceKind, Box<dyn SourceAdapterFactory>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a registry pre-populated with the six built-in
    /// factories. This is the default state used by
    /// `ApplicationFacade::new` and `ApplicationFacade::with_space`.
    pub fn with_builtins() -> Self {
        let mut r = Self::new();
        r.register(Box::new(NativeFactory));
        r.register(Box::new(GitFactory));
        r.register(Box::new(ObsidianFactory));
        r.register(Box::new(AnytypeFactory));
        r.register(Box::new(AppleNotesFactory));
        r.register(Box::new(AppleCalendarFactory));
        r
    }

    /// Register `factory` for its `SourceKind`. If a factory is already
    /// registered for that kind, the new one replaces it (last writer
    /// wins).
    pub fn register(&mut self, factory: Box<dyn SourceAdapterFactory>) {
        let kind = factory.kind();
        self.factories.insert(kind, factory);
    }

    pub fn get(&self, kind: &SourceKind) -> Option<&dyn SourceAdapterFactory> {
        self.factories.get(kind).map(|f| f.as_ref())
    }

    /// Build an adapter for `config`. Returns
    /// `SourceError::Other(...)` when no factory is registered for
    /// the kind.
    pub fn build(
        &self,
        config: SourceConfig,
    ) -> Result<Box<dyn SourceAdapter>, SourceError> {
        let factory = self.factories.get(&config.kind).ok_or_else(|| {
            SourceError::Other(format!(
                "no source factory registered for kind `{}`",
                config.kind
            ))
        })?;
        factory.build(config)
    }

    /// All registered kinds. Returned in arbitrary order; the caller
    /// must sort if order matters.
    pub fn kinds(&self) -> Vec<SourceKind> {
        self.factories.keys().cloned().collect()
    }
}

// ---- Built-in factories ----

pub struct NativeFactory;
impl SourceAdapterFactory for NativeFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Native
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        let transport: Box<dyn SourceTransport> = Box::new(NativeDirTransport::new(
            config.path.clone(),
            config.include_paths.clone(),
            config.exclude_paths.clone(),
        ));
        // Built-in parsers for Org and Markdown are not re-registered
        // here; composition roots add them via the registry. Native
        // transport returns the raw bytes; the parser registry lives at
        // the ApplicationFacade level.
        let parsers: Vec<Box<dyn FormatParser>> = vec![Box::new(EmptyParser), Box::new(EmptyParser)];
        Ok(Box::new(ComposedSourceAdapter::new(config, transport, parsers)))
    }
}

pub struct GitFactory;
impl SourceAdapterFactory for GitFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Git
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        // Git adapter historically shares the native transport in the
        // aggregated crate; for now reuse the native transport. A
        // dedicated git transport is out of scope for this task.
        let transport: Box<dyn SourceTransport> = Box::new(NativeDirTransport::new(
            config.path.clone(),
            config.include_paths.clone(),
            config.exclude_paths.clone(),
        ));
        Ok(Box::new(ComposedSourceAdapter::new(config, transport, vec![])))
    }
}

pub struct ObsidianFactory;
impl SourceAdapterFactory for ObsidianFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Obsidian
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        let transport: Box<dyn SourceTransport> = Box::new(NativeDirTransport::new(
            config.path.clone(),
            config.include_paths.clone(),
            config.exclude_paths.clone(),
        ));
        Ok(Box::new(ComposedSourceAdapter::new(config, transport, vec![])))
    }
}

pub struct AnytypeFactory;
impl SourceAdapterFactory for AnytypeFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Anytype
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        // Anytype source has no real transport in the aggregated crate;
        // the historical adapter delegates to a stub. We use a
        // zero-yield transport here; the existing tests assert that
        // writeback for an unknown source fails (UnsupportedCapability
        // path) and that scan still completes.
        let transport: Box<dyn SourceTransport> = Box::new(EmptyTransport);
        Ok(Box::new(ComposedSourceAdapter::new(config, transport, vec![])))
    }
}

pub struct AppleNotesFactory;
impl SourceAdapterFactory for AppleNotesFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::AppleNotes
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        let transport: Box<dyn SourceTransport> = Box::new(EmptyTransport);
        Ok(Box::new(ComposedSourceAdapter::new(config, transport, vec![])))
    }
}

pub struct AppleCalendarFactory;
impl SourceAdapterFactory for AppleCalendarFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::AppleCalendar
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        let transport: Box<dyn SourceTransport> = Box::new(EmptyTransport);
        Ok(Box::new(ComposedSourceAdapter::new(config, transport, vec![])))
    }
}

// ---- Transports and parsers used by the factories above ----

struct NativeDirTransport {
    root: PathBuf,
    include_paths: Vec<PathBuf>,
    exclude_paths: Vec<PathBuf>,
}

impl NativeDirTransport {
    fn new(root: PathBuf, include_paths: Vec<PathBuf>, exclude_paths: Vec<PathBuf>) -> Self {
        Self {
            root,
            include_paths,
            exclude_paths,
        }
    }
}

impl SourceTransport for NativeDirTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        use walkdir::WalkDir;
        let mut out = Vec::new();
        let roots = if self.include_paths.is_empty() {
            vec![self.root.clone()]
        } else {
            self.include_paths.clone()
        };
        for root in &roots {
            for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
                let path = entry.path();
                if self.exclude_paths.iter().any(|ex| path.starts_with(ex)) {
                    continue;
                }
                if !path.is_file() {
                    continue;
                }
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
                let mime = match ext {
                    "org" => "text/org",
                    "md" => "text/markdown",
                    _ => continue,
                };
                let payload = std::fs::read(path).map_err(TransportError::Io)?;
                out.push(RawEntity {
                    locator: path.to_string_lossy().to_string(),
                    mime_type: mime.to_string(),
                    payload,
                });
            }
        }
        Ok(out)
    }
}

struct EmptyTransport;
impl SourceTransport for EmptyTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        Ok(vec![])
    }
}

struct EmptyParser;
impl FormatParser for EmptyParser {
    fn supports(&self, _mime: &str) -> bool {
        false
    }
    fn parse(
        &self,
        _entity: &RawEntity,
        _source_id: &str,
    ) -> Result<crate::source::protocol::ParsedEntity, crate::source::protocol::ParserError> {
        Ok(crate::source::protocol::ParsedEntity {
            resources: vec![],
            relations: vec![],
            link_occurrences: vec![],
        })
    }
}
