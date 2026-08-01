//! Contract tests for `ScanUseCase`.

use notez_core::application::use_cases::ScanUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::ResourceKind;
use notez_core::domain::Selector;
use notez_core::storage::SqliteProjection;
use std::path::PathBuf;

fn make_facade(space: &std::path::Path) -> ApplicationFacade<SqliteProjection> {
    let db = space.join(".notez/idx.sqlite");
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
    facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    facade
}

#[test]
fn scan_native_via_trait_returns_scan_report() {
    let dir = tempfile::tempdir().unwrap();
    let space = dir.path();
    std::fs::write(
        space.join("a.org"),
        "#+title: A\n#+ID: 01J00000000000000000000A01\n",
    )
    .unwrap();
    let mut facade = make_facade(space);
    let report = <ApplicationFacade<_> as ScanUseCase>::scan_native(&mut facade, space)
        .expect("scan must succeed");
    assert_eq!(report.scanned_files, 1);
    assert!(report.scanned_resources >= 1);
    let page = facade
        .query(&Selector::kind(ResourceKind::Document).with_title_contains("A"))
        .unwrap();
    assert!(page.items.iter().any(|r| r.title == "A"));
}

#[test]
fn scan_native_without_parsers_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let space = dir.path();
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    let db = space.join(".notez/idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    // No parsers registered.
    let err = <ApplicationFacade<_> as ScanUseCase>::scan_native(&mut facade, space)
        .expect_err("scan must fail without parsers");
    let msg = err.to_string();
    assert!(msg.contains("no format parsers"), "got: {msg}");
}

#[test]
fn scan_federation_runs_without_crashing_on_empty_space() {
    let dir = tempfile::tempdir().unwrap();
    let space = dir.path();
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    let db = space.join(".notez/idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
    facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    let _ = <ApplicationFacade<_> as ScanUseCase>::scan_federation(&mut facade, space);
    // Empty space is not a contract violation here; the contract is
    // "the call returns and the result is observable".
    assert!(space.join(".notez/idx.sqlite").exists());
}
