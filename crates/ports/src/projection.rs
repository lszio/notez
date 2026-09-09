//! Projection read/write boundary.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionSnapshot {
    pub source_id: String,
    pub revision: String,
    pub objects: Vec<ProjectionObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionObject {
    pub address: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionPatch {
    pub address: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    Storage { message: String },
    Invalid { message: String },
}

pub trait ProjectionReader: Send + Sync {
    fn snapshot(&self, source_id: &str) -> Result<ProjectionSnapshot, ProjectionError>;
}

pub trait ProjectionWriter: Send + Sync {
    fn apply(
        &mut self,
        snapshot: &ProjectionSnapshot,
        patches: &[ProjectionPatch],
    ) -> Result<ProjectionSnapshot, ProjectionError>;
}
