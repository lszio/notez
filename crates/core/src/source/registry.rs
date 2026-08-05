//! `SourceAdapterFactory` and `SourceRegistry`.
//!
//! Each `SourceKind` value is associated with a [`SourceAdapterFactory`]
//! that knows how to construct a [`SourceAdapter`] from a
//! [`SourceConfig`]. Built-in factories cover the six native kinds
//! (`Native`, `Git`, `Obsidian`, `Anytype`, `AppleNotes`,
//! `AppleCalendar`). Third-party crates can register a factory for a
//! custom `SourceKind::Other(String)` without modifying `core`.

use std::collections::HashMap;

use crate::source::adapter::{SourceAdapter, SourceConfig, SourceError, SourceKind};

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
//
// Each factory delegates to the historical concrete `*SourceAdapter`
// type, which is the canonical implementation that already wires the
// appropriate transport and parser set.

pub struct NativeFactory;
impl SourceAdapterFactory for NativeFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Native
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        Ok(Box::new(crate::source::NativeSourceAdapter::new(config)))
    }
}

pub struct GitFactory;
impl SourceAdapterFactory for GitFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Git
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        Ok(Box::new(crate::source::GitSourceAdapter::new(config)))
    }
}

pub struct ObsidianFactory;
impl SourceAdapterFactory for ObsidianFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Obsidian
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        Ok(Box::new(crate::source::ObsidianSourceAdapter::new(config)))
    }
}

pub struct AnytypeFactory;
impl SourceAdapterFactory for AnytypeFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::Anytype
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        Ok(Box::new(crate::source::AnytypeSourceAdapter::new(config)))
    }
}

pub struct AppleNotesFactory;
impl SourceAdapterFactory for AppleNotesFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::AppleNotes
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        Ok(Box::new(crate::source::AppleNotesSourceAdapter::new(config)))
    }
}

pub struct AppleCalendarFactory;
impl SourceAdapterFactory for AppleCalendarFactory {
    fn kind(&self) -> SourceKind {
        SourceKind::AppleCalendar
    }
    fn build(&self, config: SourceConfig) -> Result<Box<dyn SourceAdapter>, SourceError> {
        Ok(Box::new(crate::source::AppleCalendarSourceAdapter::new(config)))
    }
}
