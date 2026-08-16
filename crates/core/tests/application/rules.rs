use crate::application::ApplicationService;
use crate::domain::ResourceRef;
use std::fs;
use crate::storage::SqliteProjection;

#[test]
fn inspect_rules_and_evaluation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();
    let file = space_root.join("project.org");
    let content = r#"#+title: Alpha Project
#+ID: 01J00000000000000000000500

* NEXT Build feature
:PROPERTIES:
:ID: 01J00000000000000000000501
:TYPE: project
:END:
"#;
    fs::write(&file, content).unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let r_ref = ResourceRef::parse("heading:01J00000000000000000000501").unwrap();
    let inspect_res = service
        .inspect_rules(&r_ref)
        .unwrap()
        .expect("should inspect");

    assert_eq!(inspect_res.classified_type.as_deref(), Some("project"));
    assert_eq!(
        inspect_res
            .derived_properties
            .get("para")
            .map(|s| s.as_str()),
        Some("projects")
    );
    assert!(!inspect_res.traces.is_empty());
}
