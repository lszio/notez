//! Web server functions for the v0.1 reader.
//!
//! `#[server]`-attributed functions are the public, Dioxus-managed HTTP
//! entry points. They read `NOTEZ_SPACE_ROOT`, open the SQLite projection,
//! construct an `ApplicationFacade`, and delegate to the `_impl` helpers
//! below. The `_impl` helpers take an explicit `&Path` so they can be
//! unit-tested directly with an in-memory-or-file `SqliteProjection`
//! without going through the Dioxus fullstack runtime.

use std::path::Path;

use dioxus::prelude::*;
use notez_core::application::ApplicationFacade;
use notez_core::domain::{ProjectionStore, Resource, ResourceRef, Selector};
use notez_core::storage::SqliteProjection;

use crate::model::ResourceRow;

#[server]
pub async fn list_resources() -> Result<Vec<ResourceRow>, ServerFnError> {
    let space_root = std::env::var("NOTEZ_SPACE_ROOT")
        .map_err(|_| ServerFnError::ServerError("NOTEZ_SPACE_ROOT not set".into()))?;
    list_resources_impl(Path::new(&space_root))
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))
}

#[server]
pub async fn get_resource(ref_str: String) -> Result<Option<ResourceRow>, ServerFnError> {
    let space_root = std::env::var("NOTEZ_SPACE_ROOT")
        .map_err(|_| ServerFnError::ServerError("NOTEZ_SPACE_ROOT not set".into()))?;
    get_resource_impl(Path::new(&space_root), &ref_str)
        .await
        .map_err(|e| ServerFnError::ServerError(e.to_string()))
}

/// Implementation that can be exercised by unit tests with a
/// file-backed SqliteProjection without going through the Dioxus
/// fullstack runtime. The async signature matches the server
/// functions so the wrapper is a one-liner.
pub(crate) async fn list_resources_impl(
    space_root: &Path,
) -> Result<Vec<ResourceRow>, String> {
    let db_path = space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    let page = facade.query(&Selector::new()).map_err(|e| e.to_string())?;
    Ok(page.items.into_iter().map(ResourceRow::from).collect())
}

pub(crate) async fn get_resource_impl(
    space_root: &Path,
    ref_str: &str,
) -> Result<Option<ResourceRow>, String> {
    let r_ref = ResourceRef::parse(ref_str)
        .map_err(|e| format!("invalid ref: {e}"))?;
    let db_path = space_root.join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).map_err(|e| e.to_string())?;
    let facade = ApplicationFacade::new(store);
    let res: Option<Resource> = facade.read(&r_ref).map_err(|e| e.to_string())?;
    Ok(res.map(ResourceRow::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::domain::{
        derived_object_id, ObjectId, Resource, ResourceKind, ResourceRef,
    };
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn seed_resource(store: &mut SqliteProjection, r_ref: ResourceRef, title: &str) {
        let object_id = derived_object_id("h", "loc", "h:0");
        let res = Resource {
            r#ref: r_ref,
            kind: ResourceKind::Heading,
            title: title.to_string(),
            revision: "r1".to_string(),
            source_id: "src".to_string(),
            locator: "loc".to_string(),
            properties: BTreeMap::new(),
            object_id,
        };
        store.upsert_resource(&res).unwrap();
    }

    #[tokio::test]
    async fn list_resources_impl_returns_upserted_resources() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".notez")).unwrap();
        let db_path = dir.path().join(".notez/index.sqlite");

        let mut store = SqliteProjection::open(&db_path).unwrap();
        let r = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        seed_resource(&mut store, r, "X");
        drop(store);

        let rows = list_resources_impl(dir.path()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "X");
    }

    #[tokio::test]
    async fn get_resource_impl_returns_upserted_resource() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".notez")).unwrap();
        let db_path = dir.path().join(".notez/index.sqlite");
        let mut store = SqliteProjection::open(&db_path).unwrap();
        let r = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        seed_resource(&mut store, r, "Detail");
        drop(store);

        let row = get_resource_impl(dir.path(), "heading:01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .await
            .unwrap();
        assert!(row.is_some());
        let row = row.unwrap();
        assert_eq!(row.title, "Detail");
        assert_eq!(row.kind, "heading");
    }

    #[tokio::test]
    async fn get_resource_impl_returns_none_for_missing() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".notez")).unwrap();
        let _ = SqliteProjection::open(&dir.path().join(".notez/index.sqlite")).unwrap();

        let row = get_resource_impl(dir.path(), "heading:01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .await
            .unwrap();
        assert!(row.is_none());
    }

    #[test]
    fn object_id_serializes_through_resource_row() {
        let oid = ObjectId::default();
        let r_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        let row = ResourceRow::new(
            r_ref,
            ResourceKind::Heading,
            "t".to_string(),
            "s".to_string(),
            "l".to_string(),
            oid.to_string(),
            "r".to_string(),
            BTreeMap::new(),
        );
        assert_eq!(row.object_id, oid.to_string());
    }
}