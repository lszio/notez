use crate::ApplicationError;
use domain::{
    LinkOccurrence, LinkTarget, ProjectionStore, ResolvedRelation, ResolutionStatus, ResourceKind,
    ResourceRef, Selector,
};

/// Resolves link occurrences to resources and updates the projection store.
pub fn resolve_and_store_links<S: ProjectionStore>(
    store: &mut S,
    source_id: &str,
    occurrences: Vec<LinkOccurrence>,
) -> Result<(), ApplicationError> {
    let mut resolved_relations = Vec::new();

    for occ in occurrences {
        let (status, target_ref, candidates) = match &occ.target {
            LinkTarget::Id { value, kind_hint: _ } => {
                // If it has a kind hint, it's a direct reference
                if let Some(r_ref) = occ.target.as_resource_ref() {
                    if store.get(&r_ref).map_err(|e| ApplicationError::Storage(e.to_string()))?.is_some() {
                        (ResolutionStatus::Resolved, Some(r_ref), vec![])
                    } else {
                        (ResolutionStatus::Unresolved, None, vec![])
                    }
                } else {
                    // Try all kinds
                    let mut matched = Vec::new();
                    for kind in &[
                        ResourceKind::Document,
                        ResourceKind::Heading,
                        ResourceKind::Block,
                        ResourceKind::Attachment,
                    ] {
                        let candidate = format!("{}:{value}", kind.as_str());
                        if let Ok(r_ref) = ResourceRef::parse(&candidate) {
                            if store.get(&r_ref).map_err(|e| ApplicationError::Storage(e.to_string()))?.is_some() {
                                matched.push(r_ref);
                            }
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
            LinkTarget::File { path, fragment: _ } => {
                // Find by locator exactly matching the path, or ending with the path
                // Simplified matching for now: look for exact locator match
                let selector = Selector::kind(ResourceKind::Document);
                let page = store.query(&selector).map_err(|e| ApplicationError::Storage(e.to_string()))?;
                let mut matched = Vec::new();
                for r in page.items {
                    if r.locator == *path || r.locator.ends_with(&format!("/{path}")) {
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
            LinkTarget::Title { title, fragment: _ } => {
                let selector = Selector::default().with_title_contains(title);
                let page = store.query(&selector).map_err(|e| ApplicationError::Storage(e.to_string()))?;
                // Strictly match the exact title first
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
            LinkTarget::Url { .. } => {
                (ResolutionStatus::External, None, vec![])
            }
            LinkTarget::Custom { .. } => {
                (ResolutionStatus::Unresolved, None, vec![])
            }
        };

        if let Some(t_ref) = target_ref {
            resolved_relations.push(ResolvedRelation {
                source_ref: occ.source_ref,
                target_ref: t_ref,
                target: occ.target,
                status,
                candidates,
            });
        }
    }

    store.replace_resolved_relations(source_id, resolved_relations)
        .map_err(|e| ApplicationError::Storage(e.to_string()))?;

    Ok(())
}
