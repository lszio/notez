use domain::{LinkOccurrence, Resource, ResourceRelation};
use std::sync::Arc;

pub mod markdown;
pub mod security;
pub use security::{MAX_ATTACHMENT_SIZE_BYTES, SecurityError, SecurityGuard};
pub mod org;
pub use markdown::MarkdownScanner;
pub mod workflow;
pub use workflow::{StateTransition, TodoState, TodoStateKind, WorkflowError, WorkflowProfile};

pub use org::{DocumentError, OrgScanner};

/// Result of scanning a single document file.
#[derive(Debug, Clone)]
pub struct ScannedDocument {
    pub raw: Arc<str>,
    pub resources: Vec<Resource>,
    /// Legacy resolved ID links. Kept for backward compatibility during migration.
    pub links: Vec<ResourceRelation>,
    /// All link occurrences found in the document, preserving raw form.
    pub link_occurrences: Vec<LinkOccurrence>,
}
