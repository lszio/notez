use domain::{Resource, ResourceKind, ResourceRef};
use source::{ScannedSource, SourceAdapter, SourceConfig, SourceKind};
use std::path::Path;

struct MockAdapter {
    config: SourceConfig,
}

impl SourceAdapter for MockAdapter {
    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn scan(&self) -> Result<ScannedSource, source::SourceError> {
        let r_ref = ResourceRef::parse("document:01J00000000000000000000999").unwrap();
        let res = Resource {
            r#ref: r_ref,
            kind: ResourceKind::Document,
            title: "Mock Doc".to_string(),
            revision: "rev1".to_string(),
            source_id: self.config.id.clone(),
            locator: "/mock/doc.org".to_string(),
            properties: Default::default(),
        };
        Ok(ScannedSource {
            source_id: self.config.id.clone(),
            resources: vec![res],
            relations: vec![],
            link_occurrences: vec![],
        })
    }
}

#[test]
fn source_adapter_interface() {
    let config = SourceConfig {
        id: "mock_source".to_string(),
        kind: SourceKind::Native,
        path: Path::new("/mock/path").to_path_buf(),
        read_only: true,
        include_paths: vec![],
        exclude_paths: vec![],
    };

    let adapter = MockAdapter { config };
    assert_eq!(adapter.config().id, "mock_source");
    assert_eq!(adapter.config().kind, SourceKind::Native);

    let scanned = adapter.scan().unwrap();
    assert_eq!(scanned.source_id, "mock_source");
    assert_eq!(scanned.resources.len(), 1);
    assert_eq!(scanned.resources[0].title, "Mock Doc");
}
