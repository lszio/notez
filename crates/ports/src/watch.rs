//! Watcher observation ingestion boundary.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchEvent {
    pub locator: String,
    pub kind: WatchEventKind,
    pub observed_revision: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventKind {
    Created,
    Modified,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchBatch {
    pub batch_id: String,
    pub source_id: String,
    pub events: Vec<WatchEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchIngestionError {
    Duplicate,
    Rejected { message: String },
    Retryable { message: String },
}

pub trait WatchIngestion: Send + Sync {
    fn ingest(&self, batch: WatchBatch) -> Result<(), WatchIngestionError>;
}
