use crate::link::{LinkOccurrence, ResolvedRelation, ResolutionStatus};
use crate::resource::{Resource, ResourceKind, ResourceRef, ResourceRelation, SegmentRecord};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selector {
    pub kind: Option<ResourceKind>,
    pub exact_refs: Vec<ResourceRef>,
    pub title_contains: Option<String>,
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
