//! Audit log port.
//!
//! Every successful write through the event spine appends an
//! [`AuditRecord`] to the audit log. The record captures the
//! principal, the change sequence, the affected resource, and the
//! resulting outcome — enough to answer "who did what when" without
//! needing to replay the journal itself.

use super::change::Change;
use crate::domain::resource::ResourceRef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub change_id: ulid::Ulid,
    pub principal: String,
    pub action: String,
    pub target: ResourceRef,
    pub outcome: AuditOutcome,
    pub recorded_at_unix_millis: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Rejected { reason: String },
}

pub trait AuditLog: Send {
    fn append(&self, record: AuditRecord) -> Result<(), AuditError>;
    fn for_target(&self, target: &ResourceRef) -> Result<Vec<AuditRecord>, AuditError>;
}

impl AuditRecord {
    pub fn success(change: &Change, target: ResourceRef) -> Self {
        Self {
            change_id: change.id,
            principal: change.actor.principal.clone(),
            action: format!("{:?}", change.op),
            target,
            outcome: AuditOutcome::Success,
            recorded_at_unix_millis: change.at_unix_millis,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("audit storage unavailable: {0}")]
    Storage(String),
}

#[derive(Debug, Default)]
pub struct NullAuditLog;

impl AuditLog for NullAuditLog {
    fn append(&self, _record: AuditRecord) -> Result<(), AuditError> {
        Ok(())
    }
    fn for_target(&self, _target: &ResourceRef) -> Result<Vec<AuditRecord>, AuditError> {
        Ok(Vec::new())
    }
}