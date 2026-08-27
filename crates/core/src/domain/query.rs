//! Projection ports.
//!
//! [`ProjectionStore`] is the full read+write contract a projection
//! backend must honour. **Every method is required** — there are no
//! default bodies, so a store that cannot persist something fails at
//! compile time, never silently drops data at runtime (the historical
//! no-op defaults hid data loss).
//!
use crate::domain::conflict::ConflictRecord;
use crate::domain::link::{LinkOccurrence, ResolutionStatus, ResolvedRelation};
use crate::domain::resource::{
    Resource, ResourceKind, ResourceRef, ResourceRelation, SegmentRecord,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Selector {
    pub kind: Option<ResourceKind>,
    pub exact_refs: Vec<ResourceRef>,
    pub title_contains: Option<String>,
    /// Restrict results to a particular source adapter (e.g. `native`,
    /// `apple_notes`). When `None`, every source is searched.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Row cap pushed down into the store query (`LIMIT` in SQL).
    /// `None` means unbounded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl Selector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kind(kind: ResourceKind) -> Self {
        Self {
            kind: Some(kind),
            ..Self::default()
        }
    }

    pub fn with_title_contains(mut self, title: impl Into<String>) -> Self {
        self.title_contains = Some(title.into());
        self
    }

    pub fn with_exact_ref(mut self, r: ResourceRef) -> Self {
        self.exact_refs.push(r);
        self
    }

    /// Restrict the selector to a single source adapter.
    pub fn with_source(mut self, source_id: impl Into<String>) -> Self {
        self.source_id = Some(source_id.into());
        self
    }

    /// Cap the number of rows the store returns.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    pub fields: Vec<String>,
}

impl Projection {
    pub fn summary() -> Self {
        Self {
            fields: vec!["ref".into(), "title".into(), "revision".into()],
        }
    }
}

/// Read-only projection queries.

pub trait ProjectionReader {
    type Error: std::error::Error + Send + Sync + 'static;

    fn get(&self, r#ref: &ResourceRef) -> Result<Option<Resource>, Self::Error>;
    fn query(&self, selector: &Selector) -> Result<QueryPage, Self::Error>;
    fn query_segments(&self, attachment_ref: &str) -> Result<Vec<SegmentRecord>, Self::Error>;
    fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, Self::Error>;
    fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, Self::Error>;
    /// Returns `None` when the source has no diagnostics rows at all,
    /// distinguishing "never written" from "written as empty".
    fn list_link_diagnostics(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Option<Vec<LinkDiagnostic>>, Self::Error>;
    /// 跨 Space 同一对象查询：返回所有 `object_id` 匹配的资源。
    fn find_by_object(
        &self,
        object_id: &crate::domain::ObjectIdentity,
    ) -> Result<Vec<Resource>, Self::Error>;
    /// List persisted sync conflict records, newest first.
    fn list_conflicts(&self) -> Result<Vec<ConflictRecord>, Self::Error>;
}

/// Projection writes. Required methods only: an implementation that
/// cannot honour one of these must say so by returning `Err`, never
/// by inheriting a silent no-op.
pub trait ProjectionWrite {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Atomically swap one source's slice of the projection.
    fn replace_source(
        &mut self,
        source_id: &str,
        resources: Vec<Resource>,
        relations: Vec<ResourceRelation>,
        link_occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error>;

    /// Remove conflict records by logical path (adjudication). Paths
    /// that are not present are ignored.
    fn remove_conflicts(&mut self, logical_paths: &[String]) -> Result<(), Self::Error>;

    /// Insert or update a single resource keyed by its `ResourceRef`.
    fn upsert_resource(&mut self, resource: &Resource) -> Result<(), Self::Error>;

    /// Delete a single resource by its `ResourceRef`. Idempotent.
    fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), Self::Error>;

    fn clear(&mut self) -> Result<(), Self::Error>;

    fn insert_segments(&mut self, segments: &[SegmentRecord]) -> Result<(), Self::Error>;

    fn replace_link_occurrences(
        &mut self,
        source_id: &str,
        occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error>;

    fn replace_resolved_relations(
        &mut self,
        source_id: &str,
        relations: Vec<ResolvedRelation>,
    ) -> Result<(), Self::Error>;

    /// Persist resolution status/candidates alongside each occurrence.
    fn write_link_diagnostics(
        &mut self,
        source_id: &str,
        diagnostics: &[(LinkOccurrence, ResolutionStatus, Vec<ResourceRef>)],
    ) -> Result<(), Self::Error>;

    /// Upsert conflict records keyed by `logical_path`; latest wins.
    fn replace_conflicts(
        &mut self,
        records: &[crate::domain::conflict::ConflictRecord],
    ) -> Result<(), Self::Error>;
}

/// Full projection contract: reads + required writes.
pub trait ProjectionStore: ProjectionReader + ProjectionWrite {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryPage {
    pub items: Vec<Resource>,
    pub next_cursor: Option<String>,
}

impl QueryPage {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            next_cursor: None,
        }
    }
}

/// Diagnostic record for a single link occurrence: the original occurrence plus
/// the resolution status and candidate list produced by the resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkDiagnostic {
    pub occurrence: LinkOccurrence,
    pub status: ResolutionStatus,
    pub candidates: Vec<ResourceRef>,
}
