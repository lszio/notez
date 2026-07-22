use domain::{ResourceKind, ResourceRef, Selector};

#[test]
fn golden_suite_resource_algebra_invariants() {
    let doc_ref = ResourceRef::parse("document:01J00000000000000000000001").unwrap();
    let heading_ref = ResourceRef::parse("heading:01J00000000000000000000002").unwrap();
    let att_ref = ResourceRef::parse("attachment:01J00000000000000000000003").unwrap();

    assert_eq!(doc_ref.kind(), ResourceKind::Document);
    assert_eq!(heading_ref.kind(), ResourceKind::Heading);
    assert_eq!(att_ref.kind(), ResourceKind::Attachment);

    assert_eq!(doc_ref.to_string(), "document:01J00000000000000000000001");
    assert_eq!(heading_ref.to_string(), "heading:01J00000000000000000000002");
    assert_eq!(att_ref.to_string(), "attachment:01J00000000000000000000003");

    let selector = Selector::kind(ResourceKind::Document)
        .with_title_contains("Architecture")
        .with_exact_ref(doc_ref);

    assert_eq!(selector.kind, Some(ResourceKind::Document));
    assert_eq!(selector.title_contains.as_deref(), Some("Architecture"));
    assert_eq!(selector.exact_refs, vec![doc_ref]);
}

#[test]
fn golden_suite_org_and_markdown_roundtrip_fidelity() {
    let org_input = "#+title: Golden Org\n#+ID: 01J00000000000000000000004\n\n* NEXT Directives\n:PROPERTIES:\n:ID: 01J00000000000000000000005\n:END:\n";
    let temp_dir = tempfile::tempdir().unwrap();
    let org_file = temp_dir.path().join("golden.org");
    std::fs::write(&org_file, org_input).unwrap();

    let scanned_org = document::OrgScanner::scan(&org_file, "native").unwrap();
    assert_eq!(*scanned_org.raw, *org_input);

    let md_input = "---\ntitle: Golden MD\nid: 01J00000000000000000000006\n---\n\n# Golden Section <!-- id: 01J00000000000000000000007 -->\n";
    let md_file = temp_dir.path().join("golden.md");
    std::fs::write(&md_file, md_input).unwrap();

    let scanned_md = document::MarkdownScanner::scan(&md_file, "native").unwrap();
    assert_eq!(*scanned_md.raw, *md_input);
}
