//! Pure domain vocabulary for Notez.
//!
//! Infrastructure, transport and UI dependencies are deliberately excluded.

#![forbid(unsafe_code)]

mod content;
mod graph;
mod identity;

pub use content::{Document, ObjectProjection, Source, SourceKind, SourceRole, Space, SpaceSourceMembership};
pub use graph::{GraphQuery, GraphScope, Relation, RelationKind, RelationScope};
pub use identity::{
    BlockLocator, ContentFingerprint, DocumentId, ObjectAddress, ObjectId, PositionedRef, Revision,
    SourceId, SourcePath, SourcePathError, SpaceId,
};
