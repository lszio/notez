//! ResourceUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, ResolveResult, StorageErrorKind};
use crate::application::write_check;
use crate::application::use_cases::ResourceUseCase;
use crate::domain::{
    LinkOccurrence, ProjectionStore, QueryPage, ResolutionStatus, Resource, ResourceKind,
    ResourceRef, Selector,
};

impl<S: ProjectionStore> ResourceUseCase for ApplicationFacade<S> {
    fn upsert_resource(
        &mut self,
        resource: Resource,
    ) -> Result<(), ApplicationError> {
        write_check::check_capability(self, "resource")?;
        write_check::check_address_uniqueness(
            self,
            &crate::domain::ResourceAddress::Ref { r#ref: resource.r#ref },
            &resource.r#ref,
        )?;

        self.store
            .upsert_resource(&resource)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        Ok(())
    }

    fn delete_resource(
        &mut self,
        r_ref: &ResourceRef,
    ) -> Result<(), ApplicationError> {
        write_check::check_capability(self, "resource")?;

        self.store
            .delete_resource(r_ref)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        Ok(())
    }

    fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> {
        self.store
            .query(selector)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })
    }

    fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> {
        self.store
            .get(r_ref)
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })
    }

    fn list_recent(
        &self,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError> {
        let page = self
            .store
            .query(&Selector::new())
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        let mut items = page.items;
        // Revision is monotonic by convention; lexicographic desc gives a
        // stable "most-recently-touched first" order.
        items.sort_by(|a, b| b.revision.cmp(&a.revision));
        items.truncate(limit);
        Ok(items)
    }

    fn list_by_source(
        &self,
        source_id: &str,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError> {
        let page = self
            .store
            .query(&Selector::new().with_source(source_id))
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        let mut items = page.items;
        if items.len() > limit {
            items.truncate(limit);
        }
        Ok(items)
    }

    fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> {
        let trimmed = query_str.trim();

        if let Ok(r_ref) = ResourceRef::parse(trimmed)
            && let Some(res) = self
                .store
                .get(&r_ref)
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?
        {
            return Ok(ResolveResult::Found(res.r#ref));
        }

        if trimmed.len() == 26 {
            let mut matched = Vec::new();
            if let Ok(heading_ref) = ResourceRef::parse(&format!("heading:{trimmed}"))
                && let Some(res) = self
                    .store
                    .get(&heading_ref)
                    .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?
            {
                matched.push(res.r#ref);
            }
            if let Ok(doc_ref) = ResourceRef::parse(&format!("document:{trimmed}"))
                && let Some(res) = self
                    .store
                    .get(&doc_ref)
                    .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?
            {
                matched.push(res.r#ref);
            }
            if matched.len() == 1 {
                return Ok(ResolveResult::Found(matched[0]));
            } else if matched.len() > 1 {
                return Ok(ResolveResult::Ambiguous(matched));
            }
        }

        let page_all = <Self as crate::application::use_cases::ResourceUseCase>::query(self, &Selector::new())?;
        let locator_matches: Vec<ResourceRef> = page_all
            .items
            .iter()
            .filter(|r| r.locator == trimmed)
            .map(|r| r.r#ref)
            .collect();
        if locator_matches.len() == 1 {
            return Ok(ResolveResult::Found(locator_matches[0]));
        } else if locator_matches.len() > 1 {
            return Ok(ResolveResult::Ambiguous(locator_matches));
        }

        let title_selector = Selector::new().with_title_contains(trimmed);
        let page_title = <Self as crate::application::use_cases::ResourceUseCase>::query(self, &title_selector)?;
        let title_matches: Vec<ResourceRef> = page_title.items.iter().map(|r| r.r#ref).collect();
        if title_matches.len() == 1 {
            return Ok(ResolveResult::Found(title_matches[0]));
        } else if title_matches.len() > 1 {
            return Ok(ResolveResult::Ambiguous(title_matches));
        }

        Ok(ResolveResult::NotFound)
    }

    fn resolve_address(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError> {
        use crate::domain::ResourceAddress;
        match address {
            ResourceAddress::Ref { r#ref } => {
                if let Some(res) = <Self as crate::application::use_cases::ResourceUseCase>::read(self, r#ref)? {
                    Ok(ResolveResult::Found(res.r#ref))
                } else {
                    Ok(ResolveResult::NotFound)
                }
            }
            ResourceAddress::Locator { target } => {
                // We don't have a source_ref for a bare locator; pick a
                // document-kind placeholder so resolver strategies that branch
                // on kind_hint still work. The resolver never uses source_ref
                // to compute the answer.
                let placeholder =
                    ResourceRef::new(ResourceKind::Document, ulid::Ulid::nil());
                let occ = LinkOccurrence {
                    source_ref: placeholder,
                    target: target.clone(),
                    raw: target.to_string(),
                    display_text: None,
                    span: crate::domain::TextSpan {
                        line: 0,
                        col_start: 0,
                        col_end: 0,
                    },
                };
                let (status, target_ref, candidates) =
                    crate::application::link_resolution::LinkResolver::resolve(&self.store, &occ);
                match status {
                    ResolutionStatus::Resolved => Ok(ResolveResult::Found(
                        target_ref.expect("resolved has target"),
                    )),
                    ResolutionStatus::Ambiguous => Ok(ResolveResult::Ambiguous(candidates)),
                    _ => Ok(ResolveResult::NotFound),
                }
            }
        }
    }

}
