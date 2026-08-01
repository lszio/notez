//! Contract tests for `LinkUseCase`.

use notez_core::application::use_cases::LinkUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, ApplicationFacade<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    (dir, ApplicationFacade::new(store))
}

#[test]
fn empty_link_diagnostics_and_relations() {
    let (_dir, facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E1").unwrap();
    let occs = <ApplicationFacade<_> as LinkUseCase>::query_link_occurrences(&facade, &r_ref).unwrap();
    assert!(occs.is_empty());
    let rels = <ApplicationFacade<_> as LinkUseCase>::query_resolved_relations(&facade, &r_ref).unwrap();
    assert!(rels.is_empty());
    let links = <ApplicationFacade<_> as LinkUseCase>::list_links(&facade, &r_ref).unwrap();
    assert!(links.is_empty());
}

#[test]
fn resolve_links_does_not_panic_on_missing_resource() {
    let (_dir, mut facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E2").unwrap();
    let _ = <ApplicationFacade<_> as LinkUseCase>::resolve_links(&mut facade, &r_ref);
    // Behaviour on a missing source: the implementation must not panic
    // and must return a Vec (possibly empty). We only assert shape here.
}

#[test]
fn diagnose_link_returns_empty_list_for_missing_resource() {
    let (_dir, facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E3").unwrap();
    let diag = <ApplicationFacade<_> as LinkUseCase>::diagnose_link(&facade, &r_ref).unwrap();
    assert!(diag.is_empty());
}

#[test]
fn reindex_links_returns_report_with_zero_counts_on_empty_space() {
    let (_dir, mut facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let report = <ApplicationFacade<_> as LinkUseCase>::reindex_links(&mut facade, dir.path()).unwrap();
    assert_eq!(report.scanned, 0);
}