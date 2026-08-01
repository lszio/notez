//! Verify that the type alias `ApplicationService` still resolves to the
//! renamed `ApplicationFacade` and that the new name is exported from
//! `notez_core::application`.

use notez_core::application::{ApplicationFacade, ApplicationService};
use notez_core::storage::SqliteProjection;
use std::any::TypeId;

#[test]
fn application_service_is_a_type_alias_for_application_facade() {
    assert_eq!(
        TypeId::of::<ApplicationService<SqliteProjection>>(),
        TypeId::of::<ApplicationFacade<SqliteProjection>>(),
    );
}

#[test]
fn application_facade_can_be_constructed_without_space() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let _facade = ApplicationFacade::new(store);
}
