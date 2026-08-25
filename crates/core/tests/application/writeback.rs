use crate::application::ApplicationService;
use std::fs;
use crate::storage::SqliteProjection;

/// `writeback_resource` without an explicit `SourceContext` must refuse;
/// it must never fall back to the process working directory.
#[test]
fn writeback_resource_requires_source_context() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let dot_notez = source_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let store = SqliteProjection::open(&db_path).unwrap();
    let service = ApplicationService::new(store);

    let err = service
        .writeback_resource(
            "anytype_src",
            "heading:01J00000000000000000000033",
            "updated_title",
        )
        .expect_err("writeback requires an explicit SourceContext");
    assert!(
        err.to_string().contains("SourceContext"),
        "unexpected error: {err}"
    );
}

/// `relay_sync` remains an unimplemented capability until the 0.6 Change
/// delivery model lands; it must say so instead of pretending success.
#[test]
fn relay_sync_reports_unsupported() {
    let temp_dir = tempfile::tempdir().unwrap();
    let store = SqliteProjection::in_memory().unwrap();
    let service = ApplicationService::new(store);

    let err = service
        .relay_sync("anytype_src", temp_dir.path())
        .expect_err("relay sync is not implemented yet");
    assert!(
        err.to_string().contains("not yet implemented"),
        "unexpected error: {err}"
    );
}
