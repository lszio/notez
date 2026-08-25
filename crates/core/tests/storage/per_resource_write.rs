use crate::domain::{Resource, ResourceKind, ResourceRef, Selector};
use std::collections::BTreeMap;
use crate::storage::{ProjectionStore, SqliteProjection};

fn sample(kind: ResourceKind, ulid_str: &str, title: &str, source_id: &str, revision: &str) -> Resource {
    Resource {
        r#ref: ResourceRef::new(kind, ulid_str.parse().unwrap()),
        kind,
        title: title.to_string(),
        revision: revision.to_string(),
        source_id: source_id.to_string(),
        locator: format!("/path/{title}.org"),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    }
}

#[test]
fn upsert_inserts_then_updates_in_place() {
    let mut store = SqliteProjection::in_memory().unwrap();

    let r1 = sample(ResourceKind::Heading, "01J00000000000000000000001", "draft", "native", "r1");
    store.upsert_resource(&r1).unwrap();
    let got = store.get(&r1.r#ref).unwrap().expect("inserted");
    assert_eq!(got.title, "draft");
    assert_eq!(got.revision, "r1");

    let r2 = sample(ResourceKind::Heading, "01J00000000000000000000001", "polished", "native", "r2");
    store.upsert_resource(&r2).unwrap();
    let got = store.get(&r1.r#ref).unwrap().expect("still present after upsert");
    assert_eq!(got.title, "polished");
    assert_eq!(got.revision, "r2");
}

#[test]
fn delete_resource_is_idempotent() {
    let mut store = SqliteProjection::in_memory().unwrap();
    let r = sample(ResourceKind::Attachment, "01J0000000000000000000000A", "img", "native", "r1");
    store.upsert_resource(&r).unwrap();

    store.delete_resource(&r.r#ref).unwrap();
    assert!(store.get(&r.r#ref).unwrap().is_none());

    // Second delete must NOT error.
    store.delete_resource(&r.r#ref).unwrap();
}

#[test]
fn source_id_filter_isolates_results() {
    let mut store = SqliteProjection::in_memory().unwrap();
    let a = sample(ResourceKind::Document, "01J0000000000000000000000B", "native-doc", "native", "r1");
    let b = sample(ResourceKind::Document, "01J0000000000000000000000C", "apple-doc", "apple_notes", "r1");
    store.upsert_resource(&a).unwrap();
    store.upsert_resource(&b).unwrap();

    let mut selector = Selector::new();
    let all = store.query(&selector).unwrap();
    assert_eq!(all.items.len(), 2);

    selector.source_id = Some("apple_notes".into());
    let only_apple = store.query(&selector).unwrap();
    assert_eq!(only_apple.items.len(), 1);
    assert_eq!(only_apple.items[0].source_id, "apple_notes");

    selector.source_id = Some("native".into());
    let only_native = store.query(&selector).unwrap();
    assert_eq!(only_native.items.len(), 1);
    assert_eq!(only_native.items[0].source_id, "native");
}

#[test]
fn query_results_are_stably_ordered_by_ref() {
    let mut store = SqliteProjection::in_memory().unwrap();
    let refs = [
        "01J0000000000000000000000Z",
        "01J0000000000000000000000A",
        "01J0000000000000000000000M",
    ];
    for (i, ulid) in refs.iter().enumerate() {
        let r = sample(
            ResourceKind::Heading,
            ulid,
            &format!("t{i}"),
            "native",
            "r1",
        );
        store.upsert_resource(&r).unwrap();
    }
    let page = store.query(&Selector::new()).unwrap();
    let titles: Vec<_> = page.items.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, vec!["t1", "t2", "t0"]);
}
