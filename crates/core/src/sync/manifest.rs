use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub source_id: String,
    pub actor_id: String,
    pub parent_snapshots: Vec<String>,
    pub logical_path: String,
    pub content_hash: String,
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
}
