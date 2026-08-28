//! Contract tests for public capability descriptors and Engine registration.
use notez_core::application::Engine;
use notez_core::capability::{CapabilityDescriptor, Mutability};
use notez_core::storage::SqliteProjection;

#[test]
fn capability_descriptor_constructor_and_accessors() {
    let desc = CapabilityDescriptor::new("scan", "scan native source", Mutability::Read);
    assert_eq!(desc.id, "scan");
    assert_eq!(desc.description, "scan native source");
    assert_eq!(desc.mutability, Mutability::Read);
}

#[test]
fn register_capability_persists_descriptor_in_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = Engine::new(store);
    let before = facade.capability_catalog().list().len();

    let desc = CapabilityDescriptor::new("custom_test_cap", "custom test cap", Mutability::Read);
    facade.register_capability(&desc);
    facade.register_capability(&desc);

    let after = facade.capability_catalog().list().len();
    assert_eq!(after, before + 1, "register must persist the descriptor");
    let stored = facade
        .capability_catalog()
        .get("custom_test_cap")
        .expect("descriptor is observable in the catalog");
    assert_eq!(stored.description, "custom test cap");
    assert_eq!(stored.mutability, Mutability::Read);
}
