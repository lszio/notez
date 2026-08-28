//! `core` — the notez core crate.
//!
//! All non-protocol-specific logic lives here:
//!
//! | module        | responsibility                                                 |
//! |---------------|----------------------------------------------------------------|
//! | `domain`      | ULID identifiers, link model, projection trait, rule engine     |
//! | `storage`     | `SqliteProjection` impl + content-addressed blob store         |
//! | `document`    | Generic document scanner traits + workflow/security helpers     |
//! | `source`      | Source adapters (native, git, obsidian, anytype, apple_*)      |
//! | `config`      | Space discovery + TOML config loader                           |
//! | `application` | `Engine<S>` — single use-case seam                           |
//! | `sync`        | Multi-actor sync engine + transports + three-way merge         |
//! | `artifact`    | Attachment extraction + recipes + skill export                 |
//!
//! `preview` (SSR/view-model builders) lives in its own crate,
//! `crates/preview` (`notez-preview`), so heavyweight format parsers
//! stay out of the engine's dependency graph.
//!
//! Format parsers (`org`, `markdown`) live in dedicated adapters under
//! `crates/adapters/{orgmode,markdown}` and are wired in via the
//! `orgmode` / `markdown` cargo features.
// `core` is intentionally a thin name over the standard `core` crate.

pub mod application;
pub mod artifact;
pub mod capability;
pub mod config;
pub mod document;
pub mod domain;
mod error_serde;
pub mod source;
pub mod storage;
pub mod sync;

// Re-export the common domain and engine surface.
pub use application::{ApplicationError, Engine, ResolveResult, ScanReport};
pub use domain::{
    Community, CommunityCandidate, CommunitySelector, DocumentKey, DocumentSnapshot, InspectResult,
    LinkDiagnostic, LinkOccurrence, LinkTarget, ObjectIdentity, ObjectIdentityError, ObjectSnapshot,
    Projection, ProjectionStore, QueryPage, RelationDirection, RelationType, ResolutionStatus,
    ResolvedRelation, Resource, ResourceKind, ResourceRef, ResourceRefError, ResourceRelation,
    Revision, Rule, RuleEngine, RuleKind, RuleTrace, Selector, SourceId, TaskState, TextSpan,
    derived_id, derived_object_id,
};
pub use storage::{BlobMeta, BlobStore, SegmentRecord, SqliteProjection, StorageError};
