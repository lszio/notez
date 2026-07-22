use crate::resource::{Resource, ResourceKind, ResourceRef, ResourceRelation};
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
    ) -> Result<(), Self::Error>;

    fn get(&self, r#ref: &ResourceRef) -> Result<Option<Resource>, Self::Error>;

    fn query(&self, selector: &Selector) -> Result<QueryPage, Self::Error>;

    fn clear(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryPage {
    pub items: Vec<Resource>,
    pub next_cursor: Option<String>,
}
