use serde::{Deserialize, Serialize};
use sync::ConflictRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ActiveConflicts {
    pub conflicts: Vec<ConflictRecord>,
}
