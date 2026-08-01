//! `SpaceContext` — the resolved runtime binding for a notez space.
//!
//! `ApplicationService` MUST take an explicit `SpaceContext` at
//! construction; it must never infer the space from the process working
//! directory or from a `.` path. This is the single source of truth for
//! the active space's filesystem root and resolved runtime configuration.

use std::path::PathBuf;

use crate::config::RuntimeConfig;

/// Resolved binding between an `ApplicationService` and the space it
/// operates on. The same `SpaceContext` value is shared with the CLI
/// startup, the MCP server, and the web front-end (when present) so
/// that no transport can drift into a different filesystem root.
#[derive(Debug, Clone)]
pub struct SpaceContext {
    /// Stable identifier for the space. Used as the audit actor for
    /// operations that have no other explicit actor and as the
    /// authoritative key when looking up cached projections.
    pub space_id: String,
    /// Absolute path to the space's root directory on disk.
    pub root: PathBuf,
    /// Resolved runtime configuration (global + space + env + CLI).
    pub runtime: RuntimeConfig,
}

impl SpaceContext {
    /// Construct a `SpaceContext`. The caller is responsible for
    /// resolving `runtime` via the configuration pipeline; this type
    /// does not perform discovery.
    pub fn new(space_id: impl Into<String>, root: PathBuf, runtime: RuntimeConfig) -> Self {
        Self {
            space_id: space_id.into(),
            root,
            runtime,
        }
    }
}
