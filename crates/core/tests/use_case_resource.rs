//! Contract tests for `ResourceUseCase`.

use notez_core::application::ApplicationFacade;
use notez_core::application::ResolveResult;
use notez_core::application::use_cases::ResourceUseCase;
use notez_core::domain::{ProjectionStore, Resource, ResourceKind, ResourceRef, Selector};
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, ApplicationFacade<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
    facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    (dir, facade)
}

#[test]
fn upsert_then_read_round_trip_via_traits() {
    let (_dir, mut facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000B1").unwrap();
    let res = Resource {
        r#ref: r_ref,
        kind: ResourceKind::Document,
        title: "T".to_string(),
        revision: "r1".to_string(),
        source_id: "native".to_string(),
        locator: "/x.org".to_string(),
        properties: Default::default(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    };
    <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, res).unwrap();
    let got = <ApplicationFacade<_> as ResourceUseCase>::read(&facade, &r_ref).unwrap();
    assert!(got.is_some());
}

#[test]
fn resolve_and_resolve_address_share_lookup() {
    let (_dir, mut facade) = make_facade();
    let r_ref = ResourceRef::parse("heading:01J000000000000000000000C1").unwrap();
    let res = Resource {
        r#ref: r_ref,
        kind: ResourceKind::Heading,
        title: "T".to_string(),
        revision: "r1".to_string(),
        source_id: "native".to_string(),
        locator: "/x.org".to_string(),
        properties: Default::default(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    };
    <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, res).unwrap();
    let r =
        <ApplicationFacade<_> as ResourceUseCase>::resolve(&facade, "01J000000000000000000000C1")
            .unwrap();
    assert!(matches!(r, ResolveResult::Found(_)));
}

#[test]
fn query_returns_empty_selector_page() {
    let (_dir, facade) = make_facade();
    let page = <ApplicationFacade<_> as ResourceUseCase>::query(&facade, &Selector::new()).unwrap();
    assert_eq!(page.items.len(), 0);
}

#[test]
fn delete_via_trait_removes_resource() {
    let (_dir, mut facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000D1").unwrap();
    let res = Resource {
        r#ref: r_ref,
        kind: ResourceKind::Document,
        title: "T".to_string(),
        revision: "r1".to_string(),
        source_id: "native".to_string(),
        locator: "/x.org".to_string(),
        properties: Default::default(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    };
    <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, res).unwrap();
    <ApplicationFacade<_> as ResourceUseCase>::delete_resource(&mut facade, &r_ref).unwrap();
    let got = <ApplicationFacade<_> as ResourceUseCase>::read(&facade, &r_ref).unwrap();
    assert!(got.is_none());
}

#[test]
fn list_recent_and_list_by_source_return_stable_shape() {
    let (_dir, facade) = make_facade();
    let r1 = <ApplicationFacade<_> as ResourceUseCase>::list_recent(&facade, 10).unwrap();
    let r2 =
        <ApplicationFacade<_> as ResourceUseCase>::list_by_source(&facade, "native", 10).unwrap();
    assert_eq!(r1.len(), 0);
    assert_eq!(r2.len(), 0);
}
