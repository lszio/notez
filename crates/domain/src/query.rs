use crate::link::{LinkOccurrence, ResolvedRelation, ResolutionStatus};
use crate::resource::{Resource, ResourceKind, ResourceRef, ResourceRelation, SegmentRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selector {
    pub kind: Option<ResourceKind>,
    pub exact_refs: Vec<ResourceRef>,
    pub title_contains: Option<String>,
    /// Restrict results to a particular source adapter (e.g. `native`,
    /// `apple_notes`). When `None`, every source is searched.
    #[serde(default)]
    pub source_id: Option<String>,
}

impl Selector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kind(kind: ResourceKind) -> Self {
        Self {
            kind: Some(kind),
            ..Default::default()
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    pub fields: Vec<String>,
}

impl Projection {
    pub fn summary() -> Self {
        Self {
            fields: vec![
                "ref".to_string(),
                "title".to_string(),
                "revision".to_string(),
            ],
        }
    }
}
pub trait ProjectionStore {
    type Error: std::error::Error + Send + Sync + 'static;

    fn replace_source(
        &mut self,
        source_id: &str,
        resources: Vec<Resource>,
        relations: Vec<ResourceRelation>,
        link_occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error>;

    fn get(&self, r#ref: &ResourceRef) -> Result<Option<Resource>, Self::Error>;

    fn query(&self, selector: &Selector) -> Result<QueryPage, Self::Error>;

    /// Insert or update a single resource keyed by its `ResourceRef`. Default
    /// no-op so test stubs don't break. Real projections (e.g. SQLite) MUST
    /// override this to make per-resource writes possible without a full
    /// source rescan.
    fn upsert_resource(&mut self, _resource: &Resource) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Delete a single resource by its `ResourceRef`. Idempotent: deleting an
    /// absent resource is not an error.
    fn delete_resource(&mut self, _r_ref: &ResourceRef) -> Result<(), Self::Error> {
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error>;
    fn insert_segments(&mut self, _segments: &[SegmentRecord]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn query_segments(&self, _attachment_ref: &str) -> Result<Vec<SegmentRecord>, Self::Error> {
        Ok(Vec::new())
    }

    fn replace_link_occurrences(
        &mut self,
        _source_id: &str,
        _occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn query_link_occurrences(
        &self,
        _source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, Self::Error> {
        Ok(Vec::new())
    }

    fn replace_resolved_relations(
        &mut self,
        _source_id: &str,
        _relations: Vec<ResolvedRelation>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn query_resolved_relations(
        &self,
        _source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, Self::Error> {
        Ok(Vec::new())
    }

    /// Persist the resolution status and candidate list alongside each
    /// occurrence. Implementations may store this on the same `link_occurrences`
    /// row, a sidecar table, or in a diagnostic log. The input is keyed by the
    /// raw occurrence text; implementations match occurrences in insertion
    /// order.
    fn write_link_diagnostics(
        &mut self,
        _source_id: &str,
        _diagnostics: &[(LinkOccurrence, ResolutionStatus, Vec<ResourceRef>)],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Fetch diagnostics for a single source. Returns `None` when the source
    /// has no rows (so callers can distinguish "no link columns" from
    /// "zero diagnostics written").
    fn list_link_diagnostics(
        &self,
        _source_ref: &ResourceRef,
    ) -> Result<Option<Vec<LinkDiagnostic>>, Self::Error> {
        Ok(None)
    }
 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryPage {
    pub items: Vec<Resource>,
    pub next_cursor: Option<String>,
}

/// Diagnostic record for a single link occurrence: the original occurrence plus
/// the resolution status and candidate list produced by the resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkDiagnostic {
    pub occurrence: LinkOccurrence,
    pub status: ResolutionStatus,
    pub candidates: Vec<ResourceRef>,
}
