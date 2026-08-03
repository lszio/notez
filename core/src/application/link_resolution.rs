use crate::ApplicationError;
use crate::domain::{
    LinkDiagnostic, LinkOccurrence, LinkTarget, ProjectionStore, ResolvedRelation, ResolutionStatus,
    ResourceKind, ResourceRef, Selector,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Per-status counts produced by a reindex pass.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkReindexReport {
    pub scanned: usize,
    pub resolved: usize,
    pub unresolved: usize,
    pub ambiguous: usize,
    pub external: usize,
    pub invalid: usize,
}

/// Single, deterministic link resolver. Implements the priority order from
/// the link-model plan §3.4 (exact ref → normalized path → alias →
/// basename/title) and records status / candidates for every occurrence.
pub struct LinkResolver;

impl LinkResolver {
    /// Resolve a single occurrence against the store. Returns
    /// `(status, target_ref, candidates)`.
    pub fn resolve<S: ProjectionStore>(
        store: &S,
        occ: &LinkOccurrence,
    ) -> (ResolutionStatus, Option<ResourceRef>, Vec<ResourceRef>) {
        match &occ.target {
            LinkTarget::Id { value, .. } => {
                if let Some(r_ref) = occ.target.as_resource_ref() {
                    match store.get(&r_ref) {
                        Ok(Some(_)) => (ResolutionStatus::Resolved, Some(r_ref), vec![]),
                        Ok(None) => (ResolutionStatus::Unresolved, None, vec![]),
                        Err(e) => {
                            eprintln!("link_resolver: store.get failed: {e}");
                            (ResolutionStatus::Unresolved, None, vec![])
                        }
                    }
                } else {
                    // Try all kinds; treat as ambiguous if multiple match.
                    let mut matched = Vec::new();
                    for kind in &[
                        ResourceKind::Document,
                        ResourceKind::Heading,
                        ResourceKind::Block,
                        ResourceKind::Attachment,
                    ] {
                        let candidate = format!("{}:{value}", kind.as_str());
                        if let Ok(r_ref) = ResourceRef::parse(&candidate)
                            && let Ok(Some(_)) = store.get(&r_ref)
                        {
                            matched.push(r_ref);
                        }
                    }
                    if matched.len() == 1 {
                        (ResolutionStatus::Resolved, Some(matched[0]), vec![])
                    } else if matched.len() > 1 {
                        (ResolutionStatus::Ambiguous, None, matched)
                    } else {
                        (ResolutionStatus::Unresolved, None, vec![])
                    }
                }
            }
            LinkTarget::File { path, .. } => {
                let selector = Selector::kind(ResourceKind::Document);
                let page = match store.query(&selector) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("link_resolver: store.query failed: {e}");
                        return (ResolutionStatus::Unresolved, None, vec![]);
                    }
                };
                let normalized = path.trim_start_matches("./");
                let mut matched = Vec::new();
                for r in page.items {
                    let loc = r.locator.trim_start_matches("./");
                    if r.locator == *path || r.locator.ends_with(&format!("/{path}")) {
                        matched.push(r.r#ref);
                    } else if loc == normalized {
                        matched.push(r.r#ref);
                    }
                }
                if matched.len() == 1 {
                    (ResolutionStatus::Resolved, Some(matched[0]), vec![])
                } else if matched.len() > 1 {
                    (ResolutionStatus::Ambiguous, None, matched)
                } else {
                    (ResolutionStatus::Unresolved, None, vec![])
                }
            }
            LinkTarget::Title { title, .. } => {
                let selector = Selector::new().with_title_contains(title);
                let page = match store.query(&selector) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("link_resolver: store.query failed: {e}");
                        return (ResolutionStatus::Unresolved, None, vec![]);
                    }
                };
                let mut exact_matches = Vec::new();
                for r in &page.items {
                    if r.title == *title {
                        exact_matches.push(r.r#ref);
                    }
                }
                let matches = if !exact_matches.is_empty() {
                    exact_matches
                } else {
                    page.items.into_iter().map(|r| r.r#ref).collect()
                };
                if matches.len() == 1 {
                    (ResolutionStatus::Resolved, Some(matches[0]), vec![])
                } else if matches.len() > 1 {
                    (ResolutionStatus::Ambiguous, None, matches)
                } else {
                    (ResolutionStatus::Unresolved, None, vec![])
                }
            }
            LinkTarget::Url { .. } => (ResolutionStatus::External, None, vec![]),
            LinkTarget::Custom { .. } => (ResolutionStatus::Unresolved, None, vec![]),
        }
    }

    /// Resolve all occurrences and persist the resulting diagnostics + resolved
    /// relations. Returns the list of (occurrence, status, candidates) tuples
    /// for the caller to record.
    pub fn resolve_all<S: ProjectionStore>(
        store: &mut S,
        source_id: &str,
        occurrences: Vec<LinkOccurrence>,
    ) -> Result<Vec<(LinkOccurrence, ResolutionStatus, Vec<ResourceRef>)>, ApplicationError> {
        let mut resolved_relations = Vec::new();
        let mut diagnostics = Vec::with_capacity(occurrences.len());

        for occ in occurrences {
            let (status, target_ref, candidates) = Self::resolve(store, &occ);
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let evidence = serde_json::json!({
                "source_id": source_id,
                "span_line": occ.span.line,
                "span_col_start": occ.span.col_start,
                "span_col_end": occ.span.col_end,
                "rule": "default_profile:markdown",
            });
            diagnostics.push((occ.clone(), status.clone(), candidates.clone()));
            if let Some(t_ref) = target_ref {
                resolved_relations.push(ResolvedRelation {
                    source_ref: occ.source_ref,
                    target_ref: t_ref,
                    target: occ.target.clone(),
                    status: status.clone(),
                    candidates: candidates.clone(),
                    relation_type: crate::domain::RelationType::References,
                    direction: crate::domain::RelationDirection::Forward,
                    evidence_json: evidence,
                    created_at: now_secs.to_string(),
                    creator: "scan".to_string(),
                });
            }
        }
        store
            .replace_resolved_relations(source_id, resolved_relations)
            .map_err(|e| ApplicationError::Storage {
                kind: crate::application::StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;
        store
            .write_link_diagnostics(source_id, &diagnostics)
            .map_err(|e| ApplicationError::Storage {
                kind: crate::application::StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;
        Ok(diagnostics)
    }
}

/// Backwards-compatible wrapper kept for older callers.
pub fn resolve_and_store_links<S: ProjectionStore>(
    store: &mut S,
    source_id: &str,
    occurrences: Vec<LinkOccurrence>,
) -> Result<(), ApplicationError> {
    let _ = LinkResolver::resolve_all(store, source_id, occurrences)?;
    Ok(())
}

/// Reindex every source under `space_root`, returning aggregate status counts.
pub fn reindex_links<S: ProjectionStore>(
    store: &mut S,
    _space_root: &Path,
    occurrences_by_source: Vec<(String, Vec<LinkOccurrence>)>,
) -> Result<LinkReindexReport, ApplicationError> {
    let mut report = LinkReindexReport::default();
    for (source_id, occurrences) in occurrences_by_source {
        let diags = LinkResolver::resolve_all(store, &source_id, occurrences)?;
        report.scanned += diags.len();
        for (_occ, status, _cands) in diags {
            match status {
                ResolutionStatus::Resolved => report.resolved += 1,
                ResolutionStatus::Unresolved => report.unresolved += 1,
                ResolutionStatus::Ambiguous => report.ambiguous += 1,
                ResolutionStatus::External => report.external += 1,
                ResolutionStatus::Invalid => report.invalid += 1,
            }
        }
    }
    Ok(report)
}

/// Helper used by the service to compute diagnostics for a single resource.
pub fn diagnostics_for<S: ProjectionStore>(
    store: &S,
    source_ref: &ResourceRef,
) -> Result<Option<Vec<LinkDiagnostic>>, ApplicationError> {
    store
        .list_link_diagnostics(source_ref)
        .map_err(|e| ApplicationError::Storage {
                kind: crate::application::StorageErrorKind::Sqlite,
                message: e.to_string(),
            })
}
