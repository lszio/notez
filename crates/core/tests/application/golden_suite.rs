use crate::application::ApplicationService;
use crate::domain::{LinkTarget, ResolutionStatus, ResourceKind, ResourceRef, Selector};
use std::collections::HashSet;
use crate::storage::SqliteProjection;

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

fn ulid() -> String {
    "01J00000000000000000000AA1".to_string()
}

fn build_golden_space() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let doc_id = "01J000000000000000000000D1".to_string();
    std::fs::write(
        root.join("hub.org"),
        format!(
            ":PROPERTIES:\n:ID: {doc_id}\n:END:\n#+title: Twin Hub\n\nbody\n\n[[id:01J000000000000000000000C1]]\n[[Twin]]\n[[https://example.com]]\n[[NoSuchTarget]]\n",
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("twin_a.md"),
        "---\nid: 01J000000000000000000000C1\n---\n# Twin\n",
    )
    .unwrap();
    std::fs::write(
        root.join("twin_b.md"),
        "---\nid: 01J000000000000000000000C2\n---\n# Twin\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join(".notez")).unwrap();
    tmp
}

fn open_service(root: &std::path::Path) -> ApplicationService<SqliteProjection> {
    let store = SqliteProjection::open(&root.join(".notez/index.sqlite")).unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(root).unwrap();
    service
}

fn capture(
    service: &ApplicationService<SqliteProjection>,
    source: &ResourceRef,
) -> Vec<(String, ResolutionStatus, Vec<String>)> {
    service
        .diagnose_link(source)
        .unwrap()
        .into_iter()
        .map(|d| {
            (
                d.occurrence.raw,
                d.status,
                d.candidates.iter().map(|c| c.to_string()).collect(),
            )
        })
        .collect()
}

#[test]
fn golden_link_resolution_is_deterministic() {
    let tmp = build_golden_space();
    let mut service = open_service(tmp.path());
    let heading_ref = ResourceRef::parse(&format!("heading:{}", ulid())).unwrap();

    let report1 = service.reindex_links(tmp.path()).unwrap();
    let snap1 = capture(&service, &heading_ref);

    service = open_service(tmp.path());
    let report2 = service.reindex_links(tmp.path()).unwrap();
    let snap2 = capture(&service, &heading_ref);

    assert_eq!(report1.scanned, report2.scanned, "scanned count must be stable");
    assert_eq!(
        report1.ambiguous, report2.ambiguous,
    );
    assert_eq!(snap1, snap2, "diagnostics must be byte-stable across scans");
}

#[test]
fn golden_link_statuses_cover_every_resolution_branch() {
    let tmp = build_golden_space();
    let mut service = open_service(tmp.path());
    let doc_ref = ResourceRef::parse("document:01J000000000000000000000D1").unwrap();
    service.resolve_links(&doc_ref).unwrap();
    let diags = service.diagnose_link(&doc_ref).unwrap();
    let statuses: HashSet<ResolutionStatus> = diags.iter().map(|d| d.status.clone()).collect();
    assert!(statuses.contains(&ResolutionStatus::Resolved));
    assert!(statuses.contains(&ResolutionStatus::Ambiguous));
    assert!(statuses.contains(&ResolutionStatus::External));
    assert!(statuses.contains(&ResolutionStatus::Unresolved));

    let ambig = diags
        .iter()
        .find(|d| d.status == ResolutionStatus::Ambiguous)
        .unwrap();
    assert!(ambig.candidates.len() >= 2);
    assert!(ambig.candidates.iter().all(|c: &ResourceRef| matches!(
        c.kind(),
        ResourceKind::Document | ResourceKind::Heading
    )));

    let resolved_id = diags
        .iter()
        .find(|d| matches!(&d.occurrence.target, LinkTarget::Id { .. }))
        .unwrap();
    assert_eq!(resolved_id.status, ResolutionStatus::Resolved);
}
