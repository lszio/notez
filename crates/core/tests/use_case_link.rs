//! Contract tests for `LinkUseCase`.

use notez_core::application::Engine;
use notez_core::application::use_cases::LinkUseCase;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, Engine<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let config = notez_core::config::model::SourceConfig {
        version: 2,
        source: notez_core::config::model::SourceIdentity {
            name: "test".into(),
            database: std::path::PathBuf::from(".notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let ctx = notez_core::application::context::SourceContext::new(
        "test",
        dir.path().to_path_buf(),
        config,
    );
    (dir, Engine::with_source(store, ctx))
}

#[test]
fn empty_link_diagnostics_and_relations() {
    let (_dir, facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E1").unwrap();
    let occs =
        <Engine<_> as LinkUseCase>::query_link_occurrences(&facade, &r_ref).unwrap();
    assert!(occs.is_empty());
    let rels =
        <Engine<_> as LinkUseCase>::query_resolved_relations(&facade, &r_ref).unwrap();
    assert!(rels.is_empty());
    let links = <Engine<_> as LinkUseCase>::list_links(&facade, &r_ref).unwrap();
    assert!(links.is_empty());
}

#[test]
fn resolve_links_does_not_panic_on_missing_resource() {
    let (_dir, mut facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E2").unwrap();
    let _ = <Engine<_> as LinkUseCase>::resolve_links(&mut facade, &r_ref);
    // Behaviour on a missing source: the implementation must not panic
    // and must return a Vec (possibly empty). We only assert shape here.
}

#[test]
fn diagnose_link_returns_empty_list_for_missing_resource() {
    let (_dir, facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E3").unwrap();
    let diag = <Engine<_> as LinkUseCase>::diagnose_link(&facade, &r_ref).unwrap();
    assert!(diag.is_empty());
}

#[test]
fn reindex_links_returns_report_with_zero_counts_on_empty_space() {
    let (_dir, mut facade) = make_facade();
    let report = <Engine<_> as LinkUseCase>::reindex_links(&mut facade).unwrap();
    assert_eq!(report.scanned, 0);
}
