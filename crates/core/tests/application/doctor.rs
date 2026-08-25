use crate::application::ApplicationService;
use std::fs;
use crate::storage::SqliteProjection;

#[test]
fn space_doctor_integrity_diagnostics() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let dot_notez = source_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let doc = source_root.join("healthy.org");
    fs::write(
        &doc,
        "#+title: Healthy Doc\n#+ID: 01J00000000000000000000001\n",
    )
    .unwrap();

    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(source_root).unwrap();

    let report = service.source_doctor(source_root).unwrap();
    assert_eq!(report.issues.len(), 0);
    assert_eq!(report.status, "healthy");
}
