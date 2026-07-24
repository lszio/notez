use domain::{ProjectionStore, Resource, ResourceKind, ResourceRef};
use std::collections::BTreeMap;
use storage::{SegmentRecord, SqliteProjection};

#[test]
fn test_attachment_resource_and_segments_schema() {
    let mut store = SqliteProjection::in_memory().unwrap();

    let att_ref = ResourceRef::parse("attachment:01J00000000000000000000990").unwrap();
    assert_eq!(att_ref.kind(), ResourceKind::Attachment);

    let mut props = BTreeMap::new();
    props.insert("hash".to_string(), "abcd1234hash".to_string());
    props.insert("mime".to_string(), "text/plain".to_string());
    props.insert("size_bytes".to_string(), "1024".to_string());

    let att_resource = Resource {
        r#ref: att_ref,
        kind: ResourceKind::Attachment,
        title: "document.pdf".to_string(),
        revision: "rev1".to_string(),
        source_id: "native".to_string(),
        locator: "/space/document.pdf".to_string(),
        properties: props,
    };

    store
        .replace_source("native", vec![att_resource.clone()], vec![], vec![])
        .unwrap();

    let fetched = store
        .get(&att_ref)
        .unwrap()
        .expect("attachment should exist");
    assert_eq!(fetched, att_resource);

    let seg1 = SegmentRecord {
        id: "seg_1".to_string(),
        attachment_ref: att_ref.to_string(),
        text: "Extracted segment text part 1".to_string(),
        offset_start: 0,
        offset_end: 29,
    };

    store.insert_segments(std::slice::from_ref(&seg1)).unwrap();

    let segments = store.query_segments(&att_ref.to_string()).unwrap();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0], seg1);
}
