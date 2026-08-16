use crate::application::ApplicationService;
use std::fs;
use crate::storage::SqliteProjection;

#[test]
fn application_sync_push_pull_and_conflicts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_a = temp_dir.path().join("space_a");
    let space_b = temp_dir.path().join("space_b");
    let shared_folder = temp_dir.path().join("shared");

    fs::create_dir_all(space_a.join(".notez")).unwrap();
    fs::create_dir_all(space_b.join(".notez")).unwrap();

    let doc_a = space_a.join("note.org");
    fs::write(
        &doc_a,
        "#+title: Note A\n#+ID: 01J00000000000000000000011\n",
    )
    .unwrap();

    let store_a = SqliteProjection::open(&space_a.join(".notez/index.sqlite")).unwrap();
    let mut service_a = ApplicationService::new(store_a);

    let push_report = service_a
        .sync_push("actor_a", &space_a, &shared_folder)
        .unwrap();
    assert_eq!(push_report.pushed_files, 1);

    let store_b = SqliteProjection::open(&space_b.join(".notez/index.sqlite")).unwrap();
    let mut service_b = ApplicationService::new(store_b);

    let pull_report = service_b
        .sync_pull("actor_b", &space_b, &shared_folder)
        .unwrap();
    assert_eq!(pull_report.pulled_files, 1);

    assert!(space_b.join("note.org").exists());

    let conflicts = service_b.list_conflicts().unwrap();
    assert_eq!(conflicts.len(), 0);
}
