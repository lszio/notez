//! `SourceContext` — the resolved runtime binding for a Notez source.
//!
//! `Engine` takes an explicit `SourceContext` and never infers the source
//! from the process working directory.

use crate::config::SourceConfig;
use std::path::PathBuf;
#[derive(Debug, Clone)]
pub struct SourceContext {
    /// Stable identifier for the source. Used as the audit actor for
    /// operations that have no other explicit actor and as the authoritative
    /// key when looking up cached projections.
    pub source_id: String,
    /// Absolute path to the source's root directory on disk.
    pub root: PathBuf,
    /// Resolved per-source configuration (schema v2).
    pub config: SourceConfig,
}

impl SourceContext {
    /// Construct a `SourceContext`. The caller is responsible for resolving
    /// `config` via the configuration pipeline; this type does not perform
    /// discovery.
    pub fn new(source_id: impl Into<String>, root: PathBuf, config: SourceConfig) -> Self {
        Self {
            source_id: source_id.into(),
            root,
            config,
        }
    }
}
