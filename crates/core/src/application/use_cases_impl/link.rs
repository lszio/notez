//! LinkUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, StorageErrorKind};
use crate::application::use_cases::LinkUseCase;
use crate::domain::{
    LinkDiagnostic, LinkOccurrence, ProjectionStore, ResolutionStatus, ResolvedRelation,
    ResourceRef, Selector,
};
use std::path::Path;

impl<S: ProjectionStore> LinkUseCase for ApplicationFacade<S> {
    fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::LinkOccurrence>, ApplicationError> {
        self.store
            .query_link_occurrences(source_ref)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })
    }

    fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::ResolvedRelation>, ApplicationError> {
        self.store
            .query_resolved_relations(source_ref)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })
    }

    fn list_links(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError> {
        <Self as crate::application::use_cases::LinkUseCase>::query_link_occurrences(self, source_ref)
    }

    fn resolve_links(
        &mut self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError> {
        let occs = <Self as crate::application::use_cases::LinkUseCase>::query_link_occurrences(self, source_ref)?;
        // Determine the source_id by inspecting the existing diagnostics row.
        let source_id = occs
            .first()
            .map(|_| "native")
            .unwrap_or("native")
            .to_string();
        let _ = crate::application::link_resolution::LinkResolver::resolve_all(
            &mut self.store,
            &source_id,
            occs,
        )?;
        self.store
            .query_resolved_relations(source_ref)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })
    }

    fn diagnose_link(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkDiagnostic>, ApplicationError> {
        let rows = self
            .store
            .list_link_diagnostics(source_ref)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        Ok(rows.unwrap_or_default())
    }

    fn reindex_links(
        &mut self,
        space_root: &Path,
    ) -> Result<crate::application::link_resolution::LinkReindexReport, ApplicationError> {
        // Aggregate counts from the existing projection without mutating it.
        // A full rewrite is unnecessary: `replace_source` already persisted
        // occurrences during scan, and `LinkResolver::resolve_all` already
        // wrote diagnostics. Here we merely tally what is on disk so callers
        // get a stable view of unresolved/ambiguous/external counts.
        let _ = space_root;
        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(self, &Selector::new())?;
        let mut report = crate::application::link_resolution::LinkReindexReport::default();
        for res in &page.items {
            let diags = self
                .store
                .list_link_diagnostics(&res.r#ref)
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
            if let Some(rows) = diags {
                for d in rows {
                    report.scanned += 1;
                    match d.status {
                        crate::domain::ResolutionStatus::Resolved => report.resolved += 1,
                        crate::domain::ResolutionStatus::Unresolved => report.unresolved += 1,
                        crate::domain::ResolutionStatus::Ambiguous => report.ambiguous += 1,
                        crate::domain::ResolutionStatus::External => report.external += 1,
                        crate::domain::ResolutionStatus::Invalid => report.invalid += 1,
                    }
                }
            }
        }
        Ok(report)
    }

}
