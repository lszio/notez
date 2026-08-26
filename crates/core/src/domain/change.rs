//! Change events — the spine of the event-sourced write pipeline.
//!
//! A [`Change`] is the single durable record of an attempted write.
//! It captures the actor, the affected resources, the operation kind,
//! an optional expected revision, and the payload. Every write goes
//! through the same pipeline:
//!
//! 1. the caller constructs a [`Change`];
//! 2. the journal appends it (fail-fast on journal failure);
//! 3. the projector applies the corresponding mutation to the
//!    projection;
//! 4. an audit record is appended.
//!
//! `expected_revision` is mandatory: writes that omit it are rejected
//! with `ApplicationError::RevisionConflict` rather than silently
//! overwriting concurrent updates.

use crate::domain::resource::ResourceRef;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Who triggered the change. The model is intentionally coarse —
/// richer identity is an extension field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub principal: String,
    #[serde(default)]
    pub space: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

impl Actor {
    pub fn new(principal: impl Into<String>) -> Self {
        Self {
            principal: principal.into(),
            space: None,
            source: None,
        }
    }
}

/// Operation kinds. Each maps to exactly one [`Change`] payload
/// shape below.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ChangeOp {
    UpsertResource,
    DeleteResource,
    InsertSegments,
    ReplaceLinkOccurrences,
    ReplaceResolvedRelations,
    WriteLinkDiagnostics,
    ReplaceConflicts,
    TransitionTask {
        from_state: String,
        to_state: String,
        timestamp: String,
        closed_timestamp: Option<String>,
        logbook_entry: String,
    },
    Writeback,
    Scan,
    Rebuilt,
}

/// One durable write attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub id: Ulid,
    pub actor: Actor,
    pub at_unix_millis: i64,
    pub source_id: String,
    pub op: ChangeOp,
    pub targets: Vec<ResourceRef>,
    /// Expected revision of the head resource, if the operation
    /// affects one. `None` only for operations that explicitly do
    /// not target a specific head (e.g. bulk replace).
    #[serde(default)]
    pub expected_revision: Option<String>,
    /// Operation-specific payload. Always serialized so journals can
    /// reconstruct projection state from the log alone.
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl Change {
    pub fn now_id() -> Ulid {
        Ulid::new()
    }
}