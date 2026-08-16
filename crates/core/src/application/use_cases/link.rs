//! Link use case: query, resolve, and diagnose link occurrences.

use crate::application::link_resolution::LinkReindexReport;
use crate::application::ApplicationError;
use crate::domain::{LinkDiagnostic, LinkOccurrence, ResolvedRelation, ResourceRef};

pub trait LinkUseCase {
    fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError>;
    fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError>;
    fn list_links(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError>;
    fn resolve_links(
        &mut self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError>;
    fn diagnose_link(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkDiagnostic>, ApplicationError>;
    fn reindex_links(
        &mut self,
        space_root: &std::path::Path,
    ) -> Result<LinkReindexReport, ApplicationError>;
}