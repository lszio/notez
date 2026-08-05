use crate::application::ApplicationService;
use std::fs;
use crate::storage::SqliteProjection;

#[test]
fn writeback_resource_and_relay_sync() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();
    let dot_notez = space_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let store = SqliteProjection::open(&db_path).unwrap();
    let service = ApplicationService::new(store);

    let report = service
        .writeback_resource(
            "anytype_src",
            "heading:01J00000000000000000000033",
            "updated_title",
        )
        .unwrap();
    assert!(report.committed);

    let relay_report = service.relay_sync("anytype_src", space_root).unwrap();
    assert!(relay_report.synced_via_relay);
}
