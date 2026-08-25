use crate::application::ApplicationService;
use crate::domain::ResourceKind;
use std::fs;
use crate::storage::SqliteProjection;

#[test]
fn attachment_addition_extraction_and_segment_query() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let dot_notez = source_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let file_path = source_root.join("sample_attachment.txt");
    let content =
        "Hello Notez Attachment Content! Detailed text segment 1. Detailed text segment 2.";
    fs::write(&file_path, content).unwrap();

    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);

    let att_ref = service
        .add_attachment(source_root, &file_path, "text/plain")
        .unwrap();

    assert_eq!(att_ref.kind(), ResourceKind::Attachment);

    let res = service
        .read(&att_ref)
        .unwrap()
        .expect("attachment resource should exist");
    assert_eq!(res.title, "sample_attachment.txt");
    // locator must be POSIX-relative so the tree builder can
    // reconstruct the directory hierarchy and the raw attachment
    // endpoint can serve the file under the space root.
    assert_eq!(res.locator, "sample_attachment.txt");
    assert!(
        !res.locator.starts_with('/'),
        "locator must be relative, got {}",
        res.locator
    );
    assert_eq!(
        res.properties.get("mime").map(|s| s.as_str()),
        Some("text/plain")
    );

    let segments = service.run_extraction(source_root, &att_ref).unwrap();
    assert!(!segments.is_empty());

    let fetched_segments = service.query_segments(&att_ref).unwrap();
    assert_eq!(fetched_segments.len(), segments.len());
    assert!(fetched_segments[0].text.contains("Hello Notez Attachment"));
}

#[test]
fn attachment_locator_is_posix_relative_under_space_root() {
    // Add a file in a nested directory; locator must preserve the
    // subdirectory path so the tree builder can reconstruct
    // the hierarchy and the raw endpoint can serve the file.
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();
    let dot_notez = source_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let sub = source_root.join("docs").join("proposals");
    fs::create_dir_all(&sub).unwrap();
    let nested = sub.join("whitepaper.pdf");
    fs::write(&nested, b"fake-pdf").unwrap();

    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);
    let att_ref = service
        .add_attachment(source_root, &nested, "application/pdf")
        .unwrap();
    let res = service.read(&att_ref).unwrap().unwrap();
    assert_eq!(res.locator, "docs/proposals/whitepaper.pdf");
    assert!(!res.locator.contains('\\'), "no backslashes: {}", res.locator);
    assert!(!res.locator.starts_with('/'));
}
