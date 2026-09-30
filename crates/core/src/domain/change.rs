//! Change events — the spine of the event-sourced write pipeline.
//!
//! A [`Change`] is the single durable record of an attempted write. It
//! captures the actor, operation, targets, explicit revision precondition,
//! and payload so journals preserve the concurrency contract.

use crate::domain::resource::ResourceRef;
use notez_protocol::request::RevisionPrecondition;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Who triggered the change. The model is intentionally coarse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub principal: String,
    #[serde(default)]
    pub space: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// Revision contract recorded for a durable change.
///
/// Resource mutations carry the protocol's strict precondition. Projection
/// maintenance (scan, relation replacement, and other bulk observations)
/// is explicitly marked rather than pretending to create a resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangePrecondition {
    Revision(RevisionPrecondition),
    UnconditionalObservation,
}

impl From<RevisionPrecondition> for ChangePrecondition {
    fn from(precondition: RevisionPrecondition) -> Self { Self::Revision(precondition) }
}

impl Actor {
    pub fn new(principal: impl Into<String>) -> Self {
        Self { principal: principal.into(), space: None, source: None }
    }
}

/// Operation kinds. Each maps to exactly one Change payload shape.
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
    TransitionTask { from_state: String, to_state: String, timestamp: String, closed_timestamp: Option<String>, logbook_entry: String },
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
    pub precondition: ChangePrecondition,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl Change {
    pub fn now_id() -> Ulid { Ulid::new() }
}
