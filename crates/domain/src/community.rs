use crate::query::Selector;
use crate::resource::{Resource, ResourceRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Community {
    pub id: String,
    pub name: String,
    pub selector: Selector,
    pub pinned_members: Vec<ResourceRef>,
    pub excluded_members: Vec<ResourceRef>,
}

impl Community {
    pub fn filter_members<'a>(&self, candidates: &'a [Resource]) -> Vec<&'a Resource> {
        let mut results = Vec::new();

        for res in candidates {
            if self.excluded_members.contains(&res.r#ref) {
                continue;
            }

            if self.pinned_members.contains(&res.r#ref) {
                results.push(res);
                continue;
            }

            if let Some(ref kind) = self.selector.kind {
                if res.kind != *kind {
                    continue;
                }
            }

            if let Some(ref sub) = self.selector.title_contains {
                if !res.title.to_lowercase().contains(&sub.to_lowercase()) {
                    continue;
                }
            }

            if !self.selector.exact_refs.is_empty()
                && !self.selector.exact_refs.contains(&res.r#ref)
            {
                continue;
            }

            results.push(res);
        }

        results
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommunityCandidate {
    pub name: String,
    pub member_refs: Vec<ResourceRef>,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommunitySelector {
    pub tag: Option<String>,
    pub path_prefix: Option<String>,
}
