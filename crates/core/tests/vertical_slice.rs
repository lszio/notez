use notez_core::application::{Engine, ResolveResult};
use notez_core::domain::{ResourceKind, ResourceRef, Selector};
use notez_core::storage::SqliteProjection;
use std::fs;
/// Build an `Engine` with the canonical Org/Markdown parsers registered.
/// Mirrors the composition root used by the CLI binary.
fn build_service(
    root: &std::path::Path,
    db_path: &std::path::Path,
) -> Engine<SqliteProjection> {
    let store = SqliteProjection::open(db_path).unwrap();
    let config = notez_core::config::model::SourceConfig {
        version: 2,
        source: notez_core::config::model::SourceIdentity {
            name: "test".into(),
            database: std::path::PathBuf::from(".notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
    };
    let ctx =
        notez_core::application::context::SourceContext::new("test", root.to_path_buf(), config);
    let mut service = Engine::with_source(store, ctx);
    service.register_format_parser(Box::new(orgmode::OrgParser::new()));
    service.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    service
}

#[test]
fn vertical_slice_scan_query_resolve_rebuild() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let dot_notez = source_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let file1 = source_root.join("file1.org");
    let content1 = r#"#+title: Index Document
#+ID: 01J00000000000000000000010

* NEXT Sync design heading
:PROPERTIES:
:ID: 01J00000000000000000000011
:END:
"#;
    fs::write(&file1, content1).unwrap();

    let file2 = source_root.join("file2.org");
    let content2 = r#"#+title: Notes
#+ID: 01J00000000000000000000020
"#;
    fs::write(&file2, content2).unwrap();

    let mut service = build_service(source_root, &db_path);

    let scan_report = service.scan_native().unwrap();
    assert_eq!(scan_report.scanned_files, 2);
    assert_eq!(scan_report.scanned_resources, 3); // 2 docs + 1 heading

    let page = service
        .query(&Selector::kind(ResourceKind::Heading).with_title_contains("sync"))
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "Sync design heading");

    let heading_ref = ResourceRef::parse("heading:01J00000000000000000000011").unwrap();
    let resolve_res = service.resolve("01J00000000000000000000011").unwrap();
    assert_eq!(resolve_res, ResolveResult::Found(heading_ref));

    let read_res = service
        .read(&heading_ref)
        .unwrap()
        .expect("should read resource");
    assert_eq!(read_res.title, "Sync design heading");

    // Delete database file to simulate loss of SQLite cache
    drop(service);
    fs::remove_file(&db_path).unwrap();

    let mut service2 = build_service(source_root, &db_path);
    service2.rebuild().unwrap();

    let page_rebuilt = service2
        .query(&Selector::kind(ResourceKind::Heading).with_title_contains("sync"))
        .unwrap();
    assert_eq!(page_rebuilt.items.len(), 1);
    assert_eq!(page_rebuilt.items[0].r#ref, heading_ref);
}

#[test]
fn scan_corrupt_org_preserves_valid_projection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let dot_notez = source_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let valid_file = source_root.join("valid.org");
    fs::write(
        &valid_file,
        "#+title: Valid\n#+ID: 01J00000000000000000000099\n",
    )
    .unwrap();

    let mut service = build_service(source_root, &db_path);

    let report = service.scan_native().unwrap();
    assert_eq!(report.scanned_resources, 1);

    // Add a corrupt org file with an invalid ULID
    let corrupt_file = source_root.join("corrupt.org");
    fs::write(&corrupt_file, "#+title: Corrupt\n#+ID: invalid-ulid!\n").unwrap();

    let err = service.scan_native().unwrap_err();
    assert!(err.to_string().contains("corrupt.org"));

    // Verify existing projection was preserved
    let page = service.query(&Selector::new()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "Valid");
}
