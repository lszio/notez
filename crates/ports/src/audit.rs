//! Audit outcome boundary.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub principal: String,
    pub operation: String,
    pub outcome: AuditOutcome,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditOutcome {
    Applied,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    Unavailable { message: String },
}

pub trait AuditLog: Send + Sync {
    fn record(&self, record: &AuditRecord) -> Result<(), AuditError>;
}
