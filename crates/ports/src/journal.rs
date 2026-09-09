//! Durable change journal boundary.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub sequence: u64,
    pub change_id: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    Unavailable { message: String },
    Serialization { message: String },
}

pub trait ChangeJournal: Send + Sync {
    fn append(&self, change: &[u8]) -> Result<JournalEntry, JournalError>;
    fn since(&self, sequence: u64) -> Result<Vec<JournalEntry>, JournalError>;
}
