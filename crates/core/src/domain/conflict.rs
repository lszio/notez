//! Sync conflict records (spec §7: conflicts are persisted, inspectable
//! state — never silent overwrites).

use serde::{Deserialize, Serialize};

/// A merge conflict detected while pulling a remote change into the local
/// space. The conflicting working file carries inline markers; this record
/// is the queryable evidence kept alongside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRecord {
    /// Space-relative logical path of the conflicted file.
    pub logical_path: String,
    pub mine_hash: String,
    pub theirs_hash: String,
    /// The full marker-annotated text written to the working file.
    pub conflict_text: String,
    /// Lifecycle state. `"pending"` until an actor resolves the conflict
    /// (resolution flow lands with the 0.6 Change model).
    pub status: String,
    /// Unix seconds when the conflict was detected.
    pub detected_at: u64,
}

impl ConflictRecord {
    /// Build a fresh record in the `pending` state.
    pub fn pending(
        logical_path: impl Into<String>,
        mine_hash: impl Into<String>,
        theirs_hash: impl Into<String>,
        conflict_text: impl Into<String>,
    ) -> Self {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            logical_path: logical_path.into(),
            mine_hash: mine_hash.into(),
            theirs_hash: theirs_hash.into(),
            conflict_text: conflict_text.into(),
            status: "pending".to_string(),
            detected_at: now_secs,
        }
    }
}
