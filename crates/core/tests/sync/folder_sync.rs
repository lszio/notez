use std::fs;
use crate::sync::engine::SyncEngine;
use crate::sync::transport::FolderTransport;
use tempfile::tempdir;

#[test]
fn test_folder_sync_between_two_devices() {
    let temp = tempdir().unwrap();

    let device_a = temp.path().join("device_a");
    let device_b = temp.path().join("device_b");
    let shared_folder = temp.path().join("shared_folder");

    fs::create_dir_all(&device_a).unwrap();
    fs::create_dir_all(&device_b).unwrap();
    fs::create_dir_all(&shared_folder).unwrap();

    fs::write(
        device_a.join("shared_note.org"),
        "#+title: Device A Note\n#+ID: 01J00000000000000000000099\n",
    )
    .unwrap();

    let transport_a = FolderTransport::new(&shared_folder);
    let engine_a = SyncEngine::new("device_a", &device_a, transport_a);

    let push_report = engine_a.push().unwrap();
    assert_eq!(push_report.pushed_manifests, 1);

    let transport_b = FolderTransport::new(&shared_folder);
    let engine_b = SyncEngine::new("device_b", &device_b, transport_b);

    let pull_report = engine_b.pull().unwrap();
    assert_eq!(pull_report.pulled_files, 1);

    assert!(device_b.join("shared_note.org").exists());
    let content_b = fs::read_to_string(device_b.join("shared_note.org")).unwrap();
    assert!(content_b.contains("Device A Note"));
}
