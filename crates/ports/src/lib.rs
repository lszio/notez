//! Port traits used by the Notez engine.

#![forbid(unsafe_code)]

pub use notez_domain as domain;

mod audit;
mod blob;
mod clock;
mod journal;
mod projection;
mod source;
mod watch;

pub use audit::{AuditError, AuditLog, AuditOutcome, AuditRecord};
pub use blob::{BlobError, BlobMeta, BlobStore};
pub use clock::{Clock, SystemClock};
pub use journal::{ChangeJournal, JournalEntry, JournalError};
pub use projection::{
    ProjectionError, ProjectionObject, ProjectionPatch, ProjectionReader, ProjectionSnapshot,
    ProjectionWriter,
};
pub use source::{
    DocumentSnapshot, SourceError, SourcePatch, SourceReader, SourceSnapshot, SourceWriteOutcome,
    SourceWriter,
};
pub use watch::{WatchBatch, WatchEvent, WatchEventKind, WatchIngestion, WatchIngestionError};
