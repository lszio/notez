use crate::application::ApplicationService;
use crate::domain::{LinkDiagnostic, LinkTarget, ResolutionStatus, ResourceAddress, ResourceKind, ResourceRef};
use std::fs;
use crate::storage::SqliteProjection;
fn ulid(c: char) -> String {
    let mut s = String::from("01J0000000000000000000000");
    s.push(c);
    s
}

fn build_space() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::write(
        root.join("first.org"),
        format!(
            "* Heading\n:PROPERTIES:\n:ID: {0}\n:END:\n\nbody\n\n[[Second Note]] and [[https://example.com]] after.\n",
            ulid('A')
        ),
    )
    .unwrap();
    fs::write(
        root.join("second.md"),
        format!("---\nid: {}\n---\n# Second Note\n", ulid('B')),
    )
    .unwrap();
    fs::write(
        root.join("second_alias.md"),
        format!("---\nid: {}\n---\n# Second Note\n", ulid('C')),
    )
    .unwrap();
    fs::create_dir_all(root.join(".notez")).unwrap();
    tmp
}

fn open_service(root: &std::path::Path) -> ApplicationService<SqliteProjection> {
    let db = root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(root).unwrap();
    service
}

#[test]
fn list_links_returns_occurrences() {
    let tmp = build_space();
    let service = open_service(tmp.path());

    let heading_ref = ResourceRef::parse(&format!("heading:{}", ulid('A'))).unwrap();
    let links = service.list_links(&heading_ref).unwrap();
    assert!(!links.is_empty());
}

#[test]
fn resolve_links_persists_unique_only() {
    let tmp = build_space();
    let mut service = open_service(tmp.path());
    let heading_ref = ResourceRef::parse(&format!("heading:{}", ulid('A'))).unwrap();
    let resolved = service.resolve_links(&heading_ref).unwrap();
    // URL must NOT be in resolved; only the bare title (if unique).
    let title_rel = resolved
        .iter()
        .find(|r| matches!(&r.target, LinkTarget::Title { .. }));
    assert!(
        title_rel.is_none()
            || title_rel.unwrap().status == ResolutionStatus::Ambiguous,
        "expected ambiguous or no title relation, got {:?}",
        resolved
    );
    let url_rel = resolved
        .iter()
        .find(|r| matches!(&r.target, LinkTarget::Url { .. }));
    assert!(url_rel.is_none(), "URL must not be in resolved relations");
}

#[test]
fn diagnose_link_returns_one_record_per_occurrence() {
    let tmp = build_space();
    let mut service = open_service(tmp.path());
    let heading_ref = ResourceRef::parse(&format!("heading:{}", ulid('A'))).unwrap();
    service.resolve_links(&heading_ref).unwrap();
    let diags: Vec<LinkDiagnostic> = service.diagnose_link(&heading_ref).unwrap();
    assert_eq!(diags.len(), 2, "expected 2 occurrences, got {diags:?}");
    let url = diags.iter().find(|d| matches!(&d.occurrence.target, LinkTarget::Url { .. })).unwrap();
    assert_eq!(url.status, ResolutionStatus::External);
    let title = diags.iter().find(|d| matches!(&d.occurrence.target, LinkTarget::Title { .. })).unwrap();
    assert_eq!(title.status, ResolutionStatus::Ambiguous);
    assert!(title.candidates.len() >= 2);
}

#[test]
fn reindex_links_reports_status_counts() {
    let tmp = build_space();
    let mut service = open_service(tmp.path());
    let report = service.reindex_links(tmp.path()).unwrap();
    assert!(report.scanned >= 2, "scanned={}", report.scanned);
    assert!(report.ambiguous + report.unresolved + report.external + report.resolved >= 2);
}

#[test]
fn resolve_address_accepts_locator_and_ref() {
    let tmp = build_space();
    let service = open_service(tmp.path());
    let locator = ResourceAddress::from(LinkTarget::file("second.md", None));
    let resolved = service.resolve_address(&locator).unwrap();
    assert!(matches!(resolved, application::ResolveResult::Found(_)));
    let r_ref = ResourceAddress::from(ResourceRef::parse(&format!("document:{}", ulid('B'))).unwrap());
    let resolved_ref = service.resolve_address(&r_ref).unwrap();
    assert!(matches!(resolved_ref, application::ResolveResult::Found(_)));
}
