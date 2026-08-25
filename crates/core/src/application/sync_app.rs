use crate::sync::ConflictRecord;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ActiveConflicts {
    pub conflicts: Vec<ConflictRecord>,
}
