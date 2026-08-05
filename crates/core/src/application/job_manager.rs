use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub job_type: String,
    pub target_ref: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArtifactStaleReport {
    pub status: String,
    pub stale_artifacts: Vec<String>,
}
