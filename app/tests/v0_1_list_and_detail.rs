//! End-to-end test: build a minimal Space on disk, upsert resources
//! directly into the SQLite projection, and exercise the
//! `list_resources_impl` / `get_resource_impl` helpers from
//! `app::server` exactly as the HTTP server functions would.

use app::model::ResourceRow;
use notez_core::application::ApplicationFacade;
use notez_core::domain::{
    derived_object_id, ProjectionStore, Resource, ResourceKind, ResourceRef, Selector,
};
use notez_core::storage::SqliteProjection;
use std::collections::BTreeMap;
use tempfile::tempdir;

const REF_STR: &str = "heading:01ARZ3NDEKTSV4RRFFQ69G5FAV";

fn seed(dir: &std::path::Path) {
    // Build a Space layout: notez.toml + a Markdown file. We bypass the
    // ApplicationFacade scan path because v0.1 is reader-only and the
    // test focuses on the projection read paths.
    std::fs::write(
        dir.join("notez.toml"),
        "[space]\nname = \"test-space\"\ndatabase = \".notez/index.sqlite\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join(".notez")).unwrap();
    let db_path = dir.join(".notez/index.sqlite");
    let mut store = SqliteProjection::open(&db_path).unwrap();
    let r_ref = ResourceRef::parse(REF_STR).unwrap();
    let res = Resource {
        r#ref: r_ref,
        kind: ResourceKind::Heading,
        title: "Integration Test".to_string(),
        revision: "r1".to_string(),
        source_id: "src".to_string(),
        locator: "loc".to_string(),
        properties: BTreeMap::from([("k".to_string(), "v".to_string())]),
        object_id: derived_object_id("h", "loc", "h:0"),
    };
    store.upsert_resource(&res).unwrap();

    // Also exercise the ApplicationFacade::query path so we know the
    // server-function wrapper will work on a real projection file.
    let facade = ApplicationFacade::new(SqliteProjection::open(&db_path).unwrap());
    let page = facade.query(&Selector::new()).unwrap();
    assert_eq!(page.items.len(), 1);
}

#[tokio::test]
async fn list_then_detail_roundtrip() {
    let dir = tempdir().unwrap();
    seed(dir.path());

    let rows: Vec<ResourceRow> =
        app::server::list_resources_impl(dir.path()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].ref_str, REF_STR);
    assert_eq!(rows[0].title, "Integration Test");
    assert_eq!(rows[0].kind, "heading");

    let row: Option<ResourceRow> =
        app::server::get_resource_impl(dir.path(), REF_STR).await.unwrap();
    let row = row.expect("detail row should exist");
    assert_eq!(row.ref_str, REF_STR);
    assert_eq!(row.title, "Integration Test");
    assert_eq!(row.properties.get("k").map(String::as_str), Some("v"));
}

#[tokio::test]
async fn get_missing_returns_none() {
    let dir = tempdir().unwrap();
    seed(dir.path());
    let row = app::server::get_resource_impl(
        dir.path(),
        "heading:01ARZ3NDEKTSV4RRFFQ69G5FAW",
    )
    .await
    .unwrap();
    assert!(row.is_none());
}