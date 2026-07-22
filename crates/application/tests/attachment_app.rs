use application::ApplicationService;
use domain::ResourceKind;
use std::fs;
use storage::SqliteProjection;

#[test]
fn attachment_addition_extraction_and_segment_query() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();
    let dot_notez = space_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let file_path = space_root.join("sample_attachment.txt");
    let content =
        "Hello Notez Attachment Content! Detailed text segment 1. Detailed text segment 2.";
    fs::write(&file_path, content).unwrap();

    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);

    let att_ref = service
        .add_attachment(space_root, &file_path, "text/plain")
        .unwrap();

    assert_eq!(att_ref.kind(), ResourceKind::Attachment);

    let res = service
        .read(&att_ref)
        .unwrap()
        .expect("attachment resource should exist");
    assert_eq!(res.title, "sample_attachment.txt");
    assert_eq!(
        res.properties.get("mime").map(|s| s.as_str()),
        Some("text/plain")
    );

    let segments = service.run_extraction(space_root, &att_ref).unwrap();
    assert!(!segments.is_empty());

    let fetched_segments = service.query_segments(&att_ref).unwrap();
    assert_eq!(fetched_segments.len(), segments.len());
    assert!(fetched_segments[0].text.contains("Hello Notez Attachment"));
}
