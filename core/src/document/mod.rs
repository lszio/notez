//! `document` — document scanning surface.
//!
//! The format-specific scanners (`OrgScanner` for org-mode, `MarkdownScanner`
//! for Markdown) live in this module. Each adapter crate under
//! `crates/adapters/{orgmode,markdown,obsidian,...}` re-exports a scanner to
//! stay self-contained, but the canonical implementations remain here so the
//! core crate has no cyclic dependencies.

use crate::domain::{LinkOccurrence, Resource, ResourceRelation};
use std::sync::Arc;

pub mod org;
pub use org::{DocumentError as OrgDocumentError, OrgScanner};

pub mod markdown;
pub use markdown::{DocumentError as MarkdownDocumentError, MarkdownScanner};

pub mod security;
pub use security::{MAX_ATTACHMENT_SIZE_BYTES, SecurityError, SecurityGuard};

pub mod workflow;
pub use workflow::{StateTransition, TodoState, TodoStateKind, WorkflowError, WorkflowProfile};

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
