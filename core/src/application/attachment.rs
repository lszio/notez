use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub attachment_ref: String,
    pub segments_count: usize,
    pub char_count: usize,
}
