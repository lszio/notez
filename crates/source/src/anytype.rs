use crate::adapter::{
    PreparedWrite, ScannedSource, SourceAdapter, SourceCapabilities, SourceConfig, SourceError,
    WriteResult,
};
use domain::{Resource, ResourceKind, ResourceRef};
use std::collections::BTreeMap;

pub struct AnytypeSourceAdapter {
    config: SourceConfig,
}

impl AnytypeSourceAdapter {
    pub fn new(config: SourceConfig) -> Self {
        Self { config }
    }
}

impl SourceAdapter for AnytypeSourceAdapter {
    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            can_read: true,
            can_write: true,
            can_import: true,
            can_watch: false,
        }
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        let att_ref = ResourceRef::parse("document:01J00000000000000000000077").unwrap();
        let mut props = BTreeMap::new();
        props.insert("source_type".to_string(), "anytype_stub".to_string());

        let res = Resource {
            r#ref: att_ref,
            kind: ResourceKind::Document,
            title: "Anytype External Object Note".to_string(),
            revision: "anytype_rev1".to_string(),
            source_id: self.config.id.clone(),
            locator: "/anytype/object/01J00000000000000000000077".to_string(),
            properties: props,
        };

        Ok(ScannedSource {
            source_id: self.config.id.clone(),
            resources: vec![res],
            relations: vec![],
            link_occurrences: vec![],
        })
    }

    fn prepare_write(&self, target_ref: &str, payload: &str) -> Result<PreparedWrite, SourceError> {
        Ok(PreparedWrite {
            target_ref: target_ref.to_string(),
            payload: payload.to_string(),
            ready: true,
        })
    }

    fn commit_write(&self, prep: &PreparedWrite) -> Result<WriteResult, SourceError> {
        Ok(WriteResult {
            target_ref: prep.target_ref.clone(),
            committed: true,
        })
    }
}
