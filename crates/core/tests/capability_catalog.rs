//! Contract tests for `CapabilityCatalog`.

use notez_core::capability::{CapabilityCatalog, CapabilityDescriptor, Mutability};
use std::collections::HashSet;

#[test]
fn new_catalog_is_empty() {
    let cat = CapabilityCatalog::new();
    assert!(cat.list().is_empty());
    assert!(!cat.contains("scan"));
    assert_eq!(cat.get("scan"), None);
}

#[test]
fn with_builtins_registers_nine_capabilities() {
    let cat = CapabilityCatalog::with_builtins();
    let ids: HashSet<&'static str> = cat.list().iter().map(|d| d.id).collect();
    for id in [
        "scan",
        "resource",
        "link",
        "task",
        "attachment",
        "community",
        "artifact",
        "sync",
        "inspect",
    ] {
        assert!(ids.contains(id), "missing capability `{id}`");
    }
    assert_eq!(cat.list().len(), 9);
}

#[test]
fn register_overwrites_existing_descriptor() {
    let mut cat = CapabilityCatalog::new();
    cat.register(CapabilityDescriptor::new(
        "custom",
        "first",
        Mutability::Read,
    ));
    cat.register(CapabilityDescriptor::new(
        "custom",
        "second",
        Mutability::Write,
    ));
    assert_eq!(cat.get("custom").unwrap().description, "second");
    assert_eq!(cat.get("custom").unwrap().mutability, Mutability::Write);
    assert_eq!(cat.list().len(), 1);
}

#[test]
fn get_returns_hit_or_none() {
    let cat = CapabilityCatalog::with_builtins();
    assert!(cat.get("scan").is_some());
    assert_eq!(cat.get("nonexistent"), None);
}

#[test]
fn iter_yields_each_registered_pair_once() {
    let cat = CapabilityCatalog::with_builtins();
    let mut seen = HashSet::new();
    for (id, desc) in cat.iter() {
        assert!(seen.insert(id), "duplicate id `{id}` in iter");
        assert_eq!(id, desc.id);
    }
    assert_eq!(seen.len(), 9);
}

#[test]
fn application_facade_default_catalog_covers_use_case_traits() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = notez_core::storage::SqliteProjection::open(&db).unwrap();
    let facade = notez_core::application::ApplicationFacade::new(store);
    let cat = facade.capability_catalog();
    let ids: std::collections::HashSet<&'static str> = cat.list().iter().map(|d| d.id).collect();
    for id in [
        "scan",
        "resource",
        "link",
        "task",
        "attachment",
        "community",
        "artifact",
        "sync",
        "inspect",
    ] {
        assert!(ids.contains(id), "facade missing `{id}`");
    }
    assert_eq!(cat.list().len(), 9);
}
