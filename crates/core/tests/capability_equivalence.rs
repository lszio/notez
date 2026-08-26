//! Equivalence contract for the public capability listing.
//!
//! The CLI `list-capabilities` subcommand and the MCP `list_capabilities`
//! tool must surface the same payload for a given `ApplicationFacade`.
//! Both surfaces read through `ApplicationFacade::capabilities_json`,
//! which delegates to `capability::catalog_to_json_array`.
//!
//! These tests pin the shape of that payload so future refactors cannot
//! silently desynchronise the two surfaces.

use notez_core::application::ApplicationFacade;
use notez_core::capability::{CapabilityCatalog, Mutability, catalog_to_json_array};
use notez_core::storage::SqliteProjection;

fn facade_with_builtins() -> ApplicationFacade<SqliteProjection> {
    let store = SqliteProjection::in_memory().expect("in-memory store");
    ApplicationFacade::new(store)
}

#[test]
fn facade_capabilities_json_matches_catalog_helper() {
    let facade = facade_with_builtins();
    let from_facade = facade.capabilities_json();
    let from_helper = catalog_to_json_array(facade.capability_catalog());
    assert_eq!(
        from_facade, from_helper,
        "facade helper must defer to the shared catalog serializer"
    );
}

#[test]
fn capability_listing_is_array_of_three_field_objects() {
    let facade = facade_with_builtins();
    let arr = facade
        .capabilities_json()
        .as_array()
        .expect("top-level value is an array")
        .clone();
    assert_eq!(arr.len(), 9);
    for entry in &arr {
        let obj = entry.as_object().expect("entry is an object");
        assert_eq!(obj.len(), 3, "each entry has exactly three keys");
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("description"));
        assert!(obj.contains_key("mutability"));
        assert!(obj["id"].is_string());
        assert!(obj["description"].is_string());
        let mutability = obj["mutability"].as_str().expect("mutability is string");
        assert!(
            mutability == "read" || mutability == "write",
            "unexpected mutability `{mutability}`"
        );
    }
}

#[test]
fn every_use_case_trait_has_a_corresponding_builtin_capability() {
    // The 9 use-case trait ids. The catalog is a HashMap so iteration
    // order is not stable; look up each expected id rather than
    // indexing positionally.
    //
    // `scan` mutates the persisted projection (replace_source), so its
    // honest mutability is `write`, not `read`.
    let expected: [(&str, &str); 9] = [
        ("scan", "write"),
        ("resource", "write"),
        ("link", "read"),
        ("task", "write"),
        ("attachment", "write"),
        ("community", "write"),
        ("artifact", "read"),
        ("sync", "write"),
        ("inspect", "read"),
    ];

    let arr = catalog_to_json_array(&CapabilityCatalog::with_builtins());
    let entries = arr.as_array().expect("array");
    assert_eq!(entries.len(), expected.len());
    for (id, kind) in expected.iter() {
        let entry = entries
            .iter()
            .find(|e| e["id"].as_str() == Some(*id))
            .unwrap_or_else(|| panic!("missing capability `{id}`"));
        assert_eq!(entry["mutability"].as_str(), Some(*kind));
    }
}

#[test]
fn register_capability_then_capabilities_json_observes_insertion() {
    let mut facade = facade_with_builtins();
    let before = facade.capabilities_json();
    assert_eq!(before.as_array().expect("array").len(), 9);

    facade.register_capability(&notez_core::capability::CapabilityDescriptor::new(
        "custom_metrics",
        "third-party metrics export",
        Mutability::Read,
    ));

    let after = facade.capabilities_json();
    let entries = after.as_array().expect("array");
    assert_eq!(entries.len(), 10);
    assert!(entries.iter().any(|e| e["id"] == "custom_metrics"
        && e["description"] == "third-party metrics export"
        && e["mutability"] == "read"));
}

#[test]
fn register_capability_overwrites_existing_id() {
    let mut facade = facade_with_builtins();
    facade.register_capability(&notez_core::capability::CapabilityDescriptor::new(
        "scan",
        "scan override",
        Mutability::Write,
    ));
    let arr = facade.capabilities_json();
    let entries = arr.as_array().expect("array");
    assert_eq!(entries.len(), 9, "still 9 entries after overwrite");
    let scan = entries
        .iter()
        .find(|e| e["id"] == "scan")
        .expect("scan entry");
    assert_eq!(scan["description"], "scan override");
    assert_eq!(scan["mutability"], "write");
}
