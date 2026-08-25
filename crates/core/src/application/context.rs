//! `SourceContext` — the resolved runtime binding for a notez Source.
//!
//! `ApplicationService` MUST take an explicit `SourceContext` at construction;
//! it must never infer the source from the process working directory or from a
//! `.` path. This is the single source of truth for the active source's
//! filesystem root and resolved runtime configuration.

use std::path::PathBuf;

use crate::config::SourceConfig;

/// Resolved binding between an `ApplicationService` and the source it operates
/// on. The same `SourceContext` value is shared with the CLI startup, the MCP
/// server, and the web front-end (when present) so that no transport can drift
/// into a different filesystem root.
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
