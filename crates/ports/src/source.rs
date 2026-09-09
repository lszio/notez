//! Source facts and write-back boundary.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub source_id: String,
    pub revision: String,
    pub documents: Vec<DocumentSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSnapshot {
    pub locator: String,
    pub revision: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePatch {
    pub locator: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceWriteOutcome {
    Applied { new_revision: String },
    Stale { current_revision: String },
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    Unavailable { message: String },
    Invalid { message: String },
    Io { message: String },
}

/// Read source facts without exposing filesystem or adapter handles.
pub trait SourceReader: Send + Sync {
    fn read(&self, source_id: &str) -> Result<SourceSnapshot, SourceError>;
}

/// Apply a source-relative patch against the supplied snapshot.
pub trait SourceWriter: Send + Sync {
    fn apply(
        &self,
        snapshot: &SourceSnapshot,
        patches: &[SourcePatch],
    ) -> Result<SourceWriteOutcome, SourceError>;
}
