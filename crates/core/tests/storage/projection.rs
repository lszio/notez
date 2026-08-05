use crate::domain::{Resource, ResourceKind, ResourceRef, ResourceRelation, Selector};
use std::collections::BTreeMap;
use crate::storage::{ProjectionStore, SqliteProjection};

fn sample_resource(kind: ResourceKind, title: &str, ulid_str: &str, source_id: &str) -> Resource {
    let r_ref = ResourceRef::new(kind, ulid_str.parse().unwrap());
    Resource {
        r#ref: r_ref,
        kind,
        title: title.to_string(),
        revision: "rev1".to_string(),
        source_id: source_id.to_string(),
        locator: format!("/path/{title}.org"),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectId::default(),
    }
}

#[test]
fn projection_replace_get_query_clear() {
    let mut store = SqliteProjection::in_memory().unwrap();

    let res1 = sample_resource(
        ResourceKind::Document,
        "Sync Design",
        "01J00000000000000000000001",
        "native",
    );
    let res2 = sample_resource(
        ResourceKind::Heading,
        "Architecture Sync",
        "01J00000000000000000000002",
        "native",
    );

    let rel1 = ResourceRelation {
        source_ref: res2.r#ref,
        relation: "id_link".to_string(),
        target_ref: res1.r#ref,
    };

    store
        .replace_source("native", vec![res1.clone(), res2.clone()], vec![rel1], vec![])
        .unwrap();

    let fetched = store
        .get(&res1.r#ref)
        .unwrap()
        .expect("resource 1 should exist");
    assert_eq!(fetched, res1);

    // Case-insensitive title search
    let selector = Selector::kind(ResourceKind::Document).with_title_contains("sync");
    let page = store.query(&selector).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0], res1);

    // Replace native source with only res1
    store
        .replace_source("native", vec![res1.clone()], vec![], vec![])
        .unwrap();
    assert!(store.get(&res2.r#ref).unwrap().is_none());

    // Clear and re-insert
    store.clear().unwrap();
    assert!(store.get(&res1.r#ref).unwrap().is_none());

    store
        .replace_source("native", vec![res1.clone()], vec![], vec![])
        .unwrap();
    let page2 = store.query(&Selector::new()).unwrap();
    assert_eq!(page2.items, vec![res1]);
}

#[test]
fn projection_rollback_on_duplicate_refs() {
    let mut store = SqliteProjection::in_memory().unwrap();
    let res1 = sample_resource(
        ResourceKind::Document,
        "Doc 1",
        "01J00000000000000000000001",
        "native",
    );
    let res2_dup = sample_resource(
        ResourceKind::Document,
        "Doc 2",
        "01J00000000000000000000001",
        "native",
    );
    // Attempting to replace_source with duplicate refs in the same batch should fail and roll back
    let result = store.replace_source("native", vec![res1.clone(), res2_dup], vec![], vec![]);
    assert!(result.is_err());

    // Verify nothing was committed
    assert!(store.get(&res1.r#ref).unwrap().is_none());
}

#[test]
fn projection_disk_persistence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("index.sqlite");

    let res1 = sample_resource(
        ResourceKind::Document,
        "Persistent Doc",
        "01J00000000000000000000005",
        "native",
    );

    {
        let mut store = SqliteProjection::open(&db_path).unwrap();
        store
            .replace_source("native", vec![res1.clone()], vec![], vec![])
            .unwrap();
    }

    // Re-open from disk and check data exists
    let store_reopened = SqliteProjection::open(&db_path).unwrap();
    let fetched = store_reopened
        .get(&res1.r#ref)
        .unwrap()
        .expect("should persist");
    assert_eq!(fetched, res1);
}
