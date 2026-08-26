use crate::document::OrgScanner;
use crate::domain::{LinkTarget, ResourceKind, ResourceRef};
use std::path::Path;

#[test]
fn scan_basic_org_fixture() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic.org");
    let scanned = { let __bytes = std::fs::read(&fixture_path).unwrap(); OrgScanner::parse_bytes(&__bytes, "native", &fixture_path.to_string_lossy()).unwrap() };

    let raw_expected = std::fs::read_to_string(&fixture_path).unwrap();
    assert_eq!(*scanned.raw, raw_expected);

    assert_eq!(scanned.resources.len(), 2);

    let doc = &scanned.resources[0];
    assert_eq!(
        doc.r#ref,
        ResourceRef::parse("document:01J00000000000000000000000").unwrap()
    );
    assert_eq!(doc.kind, ResourceKind::Document);
    assert_eq!(doc.title, "Notez Architecture");
    assert_eq!(doc.source_id, "native");

    let heading = &scanned.resources[1];
    assert_eq!(
        heading.r#ref,
        ResourceRef::parse("heading:01J00000000000000000000001").unwrap()
    );
    assert_eq!(heading.kind, ResourceKind::Heading);
    assert_eq!(heading.title, "Design sync");
    assert_eq!(
        heading.properties.get("TODO").map(|s| s.as_str()),
        Some("NEXT")
    );
    assert_eq!(
        heading.properties.get("TYPE").map(|s| s.as_str()),
        Some("project")
    );

    // Legacy links: bare id without kind_hint → not in legacy links
    assert_eq!(scanned.links.len(), 0);

    // link_occurrences captures all links
    assert_eq!(scanned.link_occurrences.len(), 1);
    let occ = &scanned.link_occurrences[0];
    assert_eq!(occ.source_ref, heading.r#ref);
    assert!(matches!(&occ.target, LinkTarget::Id { value, kind_hint: None } if value == "01J00000000000000000000002"));
    assert_eq!(occ.display_text.as_deref(), Some("Merge rules"));
}
#[test]
fn scan_malformed_id_reports_location() {
    use crate::document::DocumentError;
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("malformed.org");
    std::fs::write(&file_path, "#+title: Bad Doc\n#+ID: not-a-ulid\n").unwrap();

    let err = { let __bytes = std::fs::read(&file_path).unwrap(); OrgScanner::parse_bytes(&__bytes, "native", &file_path.to_string_lossy()).unwrap() };
    match err {
        DocumentError::MalformedId { line, column, .. } => {
            assert_eq!(line, 2);
            assert_eq!(column, 7);
        }
        other => panic!("expected MalformedId error, got {other:?}"),
    }
}

#[test]
fn scan_nested_headings_preserve_parent_ref() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("nested.org");
    let content = r#"#+title: Parent Child Doc
* Parent Heading
:PROPERTIES:
:ID: 01J00000000000000000000010
:END:
** Child Heading
:PROPERTIES:
:ID: 01J00000000000000000000011
:END:
"#;
    std::fs::write(&file_path, content).unwrap();

    let scanned = { let __bytes = std::fs::read(&file_path).unwrap(); OrgScanner::parse_bytes(&__bytes, "native", &file_path.to_string_lossy()).unwrap() };
    assert_eq!(scanned.resources.len(), 3); // 1 doc + 2 headings

    let parent = &scanned.resources[1];
    assert_eq!(parent.title, "Parent Heading");
    assert_eq!(
        parent.properties.get("LEVEL").map(|s| s.as_str()),
        Some("1")
    );
    assert!(!parent.properties.contains_key("PARENT_REF"));

    let child = &scanned.resources[2];
    assert_eq!(child.properties.get("LEVEL").map(|s| s.as_str()), Some("2"));
    assert_eq!(
        child.properties.get("PARENT_REF").map(|s| s.as_str()),
        Some("heading:01J00000000000000000000010")
    );
}
