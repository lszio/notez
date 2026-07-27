use application::ApplicationService;
use domain::{Resource, ResourceKind, ResourceRef};
use std::collections::BTreeMap;
use storage::SqliteProjection;

fn fixture(kind: ResourceKind, ulid_str: &str, title: &str, source_id: &str) -> Resource {
    Resource {
        r#ref: ResourceRef::new(kind, ulid_str.parse().unwrap()),
        kind,
        title: title.to_string(),
        revision: "rev1".to_string(),
        source_id: source_id.to_string(),
        locator: format!("/loc/{title}"),
        properties: BTreeMap::new(),
    }
}

#[test]
fn upsert_then_list_recent_then_delete() {
    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);

    let r1 = fixture(
        ResourceKind::Document,
        "01J0000000000000000000000A",
        "alpha",
        "native",
    );
    let r2 = fixture(
        ResourceKind::Heading,
        "01J0000000000000000000000B",
        "beta",
        "apple_notes",
    );
    let r3 = fixture(
        ResourceKind::Attachment,
        "01J0000000000000000000000C",
        "gamma",
        "native",
    );

    service.upsert_resource(r1.clone()).unwrap();
    service.upsert_resource(r2.clone()).unwrap();
    service.upsert_resource(r3.clone()).unwrap();

    // list_recent should return all three, ordered by revision desc.
    let recent = service.list_recent(10).unwrap();
    assert_eq!(recent.len(), 3);
    assert!(recent.iter().any(|r| r.r#ref == r1.r#ref));
    assert!(recent.iter().any(|r| r.r#ref == r2.r#ref));
    assert!(recent.iter().any(|r| r.r#ref == r3.r#ref));

    // list_by_source isolates per adapter.
    let apple = service.list_by_source("apple_notes", 100).unwrap();
    assert_eq!(apple.len(), 1);
    assert_eq!(apple[0].source_id, "apple_notes");

    let native = service.list_by_source("native", 100).unwrap();
    assert_eq!(native.len(), 2);

    // Delete one and confirm it vanishes from listings.
    service.delete_resource(&r2.r#ref).unwrap();
    let apple_after = service.list_by_source("apple_notes", 100).unwrap();
    assert_eq!(apple_after.len(), 0);
    let recent_after = service.list_recent(10).unwrap();
    assert_eq!(recent_after.len(), 2);
    assert!(!recent_after.iter().any(|r| r.r#ref == r2.r#ref));
}

#[test]
fn upsert_then_update_is_visible_via_query() {
    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);

    let mut r = fixture(
        ResourceKind::Heading,
        "01J0000000000000000000000D",
        "draft",
        "native",
    );
    service.upsert_resource(r.clone()).unwrap();

    r.title = "polished".to_string();
    r.revision = "rev2".to_string();
    r.properties
        .insert("TODO".to_string(), "DONE".to_string());
    service.upsert_resource(r.clone()).unwrap();

    let got = service.read(&r.r#ref).unwrap().expect("present");
    assert_eq!(got.title, "polished");
    assert_eq!(got.revision, "rev2");
    assert_eq!(got.properties.get("TODO").map(String::as_str), Some("DONE"));
}

#[test]
fn delete_is_idempotent() {
    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);

    let r = fixture(
        ResourceKind::Document,
        "01J0000000000000000000000E",
        "transient",
        "native",
    );
    service.upsert_resource(r.clone()).unwrap();
    service.delete_resource(&r.r#ref).unwrap();
    // Second delete must not error and must leave the projection clean.
    service.delete_resource(&r.r#ref).unwrap();
    assert!(service.read(&r.r#ref).unwrap().is_none());
}
