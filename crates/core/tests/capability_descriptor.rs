//! Contract tests for the public `CapabilityDescriptor` and the
//! `register_capability` no-op on `ApplicationService`.

use notez_core::application::ApplicationService;
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
fn register_capability_is_a_no_op_that_accepts_descriptors() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationService::new(store);
    let desc = CapabilityDescriptor::new("scan", "scan native source", Mutability::Read);
    // Must not panic, must not error, must not change observable state.
    facade.register_capability(&desc);
    facade.register_capability(&desc);
}
