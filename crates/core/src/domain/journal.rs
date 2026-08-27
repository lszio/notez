//! Event journal port.
//!
//! A journal is an append-only log of [`Change`](super::change::Change)
//! events, plus a monotonic cursor. Two adapters ship today:
//!
//! * `SqliteEventJournal` — primary, durable, queryable;
//! * `NullJournal` — used in tests and as the default when no
//!   journal adapter is wired up. It **accepts and silently discards**
//!   every append (returning `Ok(0)`), so writes never fail on its
//!   account; durable, queryable Change history requires attaching
//!   `SqliteEventJournal` via `ApplicationFacade::attach_journal`.
//!
//! All methods are required: silent dropping would defeat the audit
//! guarantees.
//!
//! SQLite connections are not `Sync`, so the port requires only
//! [`Send`]. Adapters that share state across threads must provide
//! their own interior synchronization.
use super::change::Change;
use std::path::PathBuf;

pub trait EventJournal: Send {
    /// Append `change` and return the sequence number assigned to
    /// it. Implementations MUST be atomic (no partial writes) and
    /// MUST return an error when the underlying store is unavailable.
    fn append(&self, change: &Change) -> Result<u64, JournalError>;

    /// List every change at or after `cursor` (inclusive), oldest
    /// first. `cursor == 0` returns the entire journal.
    fn since(&self, cursor: u64) -> Result<Vec<JournalEntry>, JournalError>;

    /// Count of events appended so far.
    fn len(&self) -> Result<u64, JournalError>;

    /// Mirror the journal to the given file path. `None` means the
    /// adapter has no file mirror (the sqlite adapter always does).
    fn mirror_path(&self) -> Option<PathBuf> {
        None
    }
}

/// One row from `since`: change + assigned sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub sequence: u64,
    pub change: Change,
}

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("journal storage unavailable: {0}")]
    Storage(String),
    #[error("journal schema error: {0}")]
    Schema(String),
    #[error("journal io error: {0}")]
    Io(#[from] std::io::Error),
}

/// A journal that accepts and discards writes — the default when no
/// journal adapter is wired up, and a stand-in for unit tests that do
/// not care about audit. Returns `Ok(0)` for every append so the
/// surrounding write path is never blocked by journal unavailability.
#[derive(Debug, Default)]
pub struct NullJournal;

impl EventJournal for NullJournal {
    fn append(&self, _change: &Change) -> Result<u64, JournalError> {
        Ok(0)
    }

    fn since(&self, _cursor: u64) -> Result<Vec<JournalEntry>, JournalError> {
        Ok(Vec::new())
    }

    fn len(&self) -> Result<u64, JournalError> {
        Ok(0)
    }
}
