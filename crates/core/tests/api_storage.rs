use notez_core::domain::{
    LinkOccurrence, LinkTarget, ProjectionStore, RelationDirection, RelationType, ResolutionStatus,
    ResolvedRelation, Resource, ResourceKind, ResourceRef, ResourceRelation, SegmentRecord,
    Selector, TextSpan,
};
use notez_core::storage::{BlobMeta, BlobStore, SqliteProjection, StorageError};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const DOC_ID: &str = "01J00000000000000000000001";
const HEADING_ID: &str = "01J00000000000000000000002";

fn rref(kind: ResourceKind, id: &str) -> ResourceRef {
    ResourceRef::parse(&format!("{}:{id}", kind.as_str())).unwrap()
}

fn resource(kind: ResourceKind, id: &str, title: &str, source: &str, revision: &str) -> Resource {
    Resource {
        r#ref: rref(kind, id),
        kind,
        title: title.into(),
        revision: revision.into(),
        source_id: source.into(),
        locator: format!("{source}/{title}"),
        properties: BTreeMap::from([(String::from("TYPE"), String::from("note"))]),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: source.into(),
    }
}

fn occurrence(source_ref: ResourceRef, raw: &str) -> LinkOccurrence {
    LinkOccurrence {
        source_ref,
        target: LinkTarget::title(raw, None),
        raw: raw.into(),
        display_text: Some(raw.into()),
        span: TextSpan {
            line: 2,
            col_start: 3,
            col_end: 3 + raw.len(),
        },
    }
}

#[test]
fn sqlite_projection_round_trips_sources_queries_and_mutations() {
    let mut store = SqliteProjection::in_memory().unwrap();
    let document = resource(ResourceKind::Document, DOC_ID, "Notes", "native", "1");
    let heading = resource(ResourceKind::Heading, HEADING_ID, "Design", "native", "2");
    let relation = ResourceRelation {
        source_ref: document.r#ref,
        relation: "contains".into(),
        target_ref: heading.r#ref,
        relation_type: RelationType::References,
        direction: RelationDirection::Unknown,
        evidence_json: serde_json::json!({}),
        created_at: String::new(),
        creator: "scan".to_string(),
    };
    let occ = occurrence(document.r#ref, "Design");

    store
        .replace_source(
            "native",
            vec![document.clone(), heading.clone()],
            vec![relation],
            vec![occ.clone()],
        )
        .unwrap();
    assert_eq!(store.get(&document.r#ref).unwrap(), Some(document.clone()));
    assert_eq!(
        store
            .get(&rref(ResourceKind::Document, "01J00000000000000000000099"))
            .unwrap(),
        None
    );
    assert_eq!(
        store.query(&Selector::new()).unwrap().items,
        vec![document.clone(), heading.clone()]
    );
    assert_eq!(
        store
            .query(&Selector::kind(ResourceKind::Heading))
            .unwrap()
            .items,
        vec![heading.clone()]
    );
    assert_eq!(
        store
            .query(&Selector::new().with_source("other"))
            .unwrap()
            .items,
        Vec::<Resource>::new()
    );
    assert_eq!(
        store.query_link_occurrences(&document.r#ref).unwrap(),
        vec![occ.clone()]
    );

    let mut updated = heading.clone();
    updated.title = "Design Updated".into();
    updated.revision = "3".into();
    store.upsert_resource(&updated).unwrap();
    assert_eq!(store.get(&heading.r#ref).unwrap(), Some(updated.clone()));
    store.delete_resource(&heading.r#ref).unwrap();
    store.delete_resource(&heading.r#ref).unwrap();
    assert_eq!(store.get(&heading.r#ref).unwrap(), None);
}

#[test]
fn sqlite_projection_persists_segments_link_resolution_and_diagnostics() {
    let mut store = SqliteProjection::in_memory().unwrap();
    let source = rref(ResourceKind::Document, DOC_ID);
    let target = rref(ResourceKind::Heading, HEADING_ID);
    let occ = occurrence(source, "Design");
    let segments = vec![
        SegmentRecord {
            id: "seg-2".into(),
            attachment_ref: source.to_string(),
            text: "second".into(),
            offset_start: 7,
            offset_end: 13,
        },
        SegmentRecord {
            id: "seg-1".into(),
            attachment_ref: source.to_string(),
            text: "first".into(),
            offset_start: 0,
            offset_end: 5,
        },
    ];
    store.insert_segments(&segments).unwrap();
    let queried = store.query_segments(&source.to_string()).unwrap();
    assert_eq!(
        queried.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["seg-1", "seg-2"]
    );
    assert!(
        store
            .query_segments("attachment:missing")
            .unwrap()
            .is_empty()
    );

    let resolved = ResolvedRelation {
        source_ref: source,
        target_ref: target,
        target: LinkTarget::title("Design", None),
        status: ResolutionStatus::Resolved,
        candidates: vec![target],
        relation_type: RelationType::References,
        direction: RelationDirection::Unknown,
        evidence_json: serde_json::json!({}),
        created_at: String::new(),
        creator: "scan".to_string(),
    };
    store
        .replace_resolved_relations("native", vec![resolved.clone()])
        .unwrap();
    assert_eq!(
        store.query_resolved_relations(&source).unwrap(),
        vec![resolved]
    );

    store
        .replace_link_occurrences("native", vec![occ.clone()])
        .unwrap();
    store
        .write_link_diagnostics(
            "native",
            &[(occ.clone(), ResolutionStatus::Resolved, vec![target])],
        )
        .unwrap();
    let diagnostics = store.list_link_diagnostics(&source).unwrap().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].status, ResolutionStatus::Resolved);
    assert_eq!(diagnostics[0].candidates, vec![target]);

    store
        .replace_resolved_relations("native", Vec::new())
        .unwrap();
    assert!(store.query_resolved_relations(&source).unwrap().is_empty());
    store.clear().unwrap();
    assert!(store.query(&Selector::new()).unwrap().items.is_empty());
    assert!(store.list_link_diagnostics(&source).unwrap().is_none());
}

#[test]
fn sqlite_open_surfaces_storage_error_for_unusable_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing").join("index.sqlite");
    let error = match SqliteProjection::open(&path) {
        Ok(_) => panic!("opening a database below a missing directory must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, StorageError::Sqlite(_)));
    assert!(error.to_string().contains("SQLite error"));
}

#[test]
fn blob_store_content_addressing_mime_detection_and_missing_get() {
    let dir = tempfile::tempdir().unwrap();
    let blobs = BlobStore::new(dir.path());
    let meta = blobs.store_bytes(b"hello", "").unwrap();
    assert_eq!(meta.size_bytes, 5);
    assert_eq!(meta.mime_type, "text/plain");
    assert_eq!(meta.hash.len(), 64);
    assert!(blobs.has(&meta.hash));
    assert_eq!(blobs.get(&meta.hash).unwrap(), Some(b"hello".to_vec()));

    let same = blobs
        .store_bytes(b"hello", "application/octet-stream")
        .unwrap();
    assert_eq!(same, meta);
    let png = blobs.store_bytes(b"\x89PNG\r\n\x1a\nrest", "").unwrap();
    assert_eq!(png.mime_type, "image/png");
    assert!(!blobs.has("bad"));
    assert_eq!(blobs.get("bad").unwrap(), None);
    assert_eq!(blobs.get("00invalid-hash").unwrap(), None);
}

#[test]
fn blob_store_reports_filesystem_errors_when_blob_root_is_file() {
    let dir = tempfile::tempdir().unwrap();
    let notez = dir.path().join(".notez");
    fs::create_dir_all(&notez).unwrap();
    fs::write(notez.join("blobs"), b"not a directory").unwrap();
    let blobs = BlobStore::new(dir.path());
    let error = blobs.store_bytes(b"bytes", "text/plain").unwrap_err();
    assert!(
        error.to_string().contains("Not a directory")
            || error.to_string().contains("not a directory")
    );
}
