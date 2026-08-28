use crate::application::Engine;
use std::fs;
use crate::storage::SqliteProjection;

#[test]
fn job_manager_and_artifact_freshness_check() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let dot_notez = source_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let store = SqliteProjection::open(&db_path).unwrap();
    let service = Engine::new(store);

    let jobs = service.list_jobs().unwrap();
    assert_eq!(jobs.len(), 0);

    let stale_report = service.check_artifact_freshness(source_root).unwrap();
    assert_eq!(stale_report.stale_artifacts.len(), 0);
    assert_eq!(stale_report.status, "fresh");
}
