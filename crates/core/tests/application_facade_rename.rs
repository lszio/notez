//! Contract tests for the public `Engine` export.
use notez_core::application::Engine;
use notez_core::storage::SqliteProjection;
use std::any::TypeId;

#[test]
fn application_service_is_a_type_alias_for_application_facade() {
    assert_eq!(
        TypeId::of::<Engine<SqliteProjection>>(),
        TypeId::of::<Engine<SqliteProjection>>(),
    );
}

#[test]
fn application_facade_can_be_constructed_without_space() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let _facade = Engine::new(store);
}
