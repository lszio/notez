//! Public capability descriptors.
//!
//! `ApplicationFacade` owns a [`CapabilityCatalog`] seeded with the nine
//! built-in capabilities (one per `core::application::use_cases` trait).
//! `ApplicationFacade::register_capability` inserts/replaces a
//! [`CapabilityDescriptor`] in that catalog; the public-facing CLI
//! `list-capabilities` subcommand and the MCP `list_capabilities` tool
//! both read the same catalog, so the two surfaces stay in lockstep.
//!
//! The catalog is a `HashMap<&'static str, CapabilityDescriptor>` keyed
//! by id. See [`CapabilityCatalog::with_builtins`] for the canonical
//! descriptor set.

use serde::{Deserialize, Serialize};

/// Whether a capability can mutate persisted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mutability {
    /// Pure read against the current state.
    Read,
    /// May change persisted state.
    Write,
}

/// Stable description of a capability exposed by the system. Capability
/// descriptors are the unit that MCP tools, CLI subcommands, and
/// previewer registrations consume. `ApplicationFacade::register_capability`
/// inserts the descriptor into the live `CapabilityCatalog`; readers such
/// as `ApplicationFacade::capability_catalog` and the JSON serializers
/// below observe the resulting set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub id: &'static str,
    pub description: &'static str,
    pub mutability: Mutability,
}

impl CapabilityDescriptor {
    pub fn new(id: &'static str, description: &'static str, mutability: Mutability) -> Self {
        Self {
            id,
            description,
            mutability,
        }
    }
}

use std::collections::HashMap;

/// Registry of public capabilities exposed by the system.
///
/// Capabilities are static descriptors (id, description, mutability)
/// that callers — CLI help text, MCP tool listings, documentation
/// generators — can query at runtime. The catalog is a
/// `HashMap<&'static str, CapabilityDescriptor>` keyed by id.
///
/// `with_builtins()` populates the nine capabilities that map
/// 1-to-1 to the nine `UseCase` traits in `core::application::use_cases`.
/// Third-party code may register additional descriptors via
/// `register`; the public API does not distinguish between built-in
/// and externally-registered entries.
pub struct CapabilityCatalog {
    descriptors: HashMap<&'static str, CapabilityDescriptor>,
}

impl Default for CapabilityCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityCatalog {
    /// Construct an empty catalog.
    pub fn new() -> Self {
        Self {
            descriptors: HashMap::new(),
        }
    }

    /// Construct a catalog pre-populated with the nine built-in
    /// capabilities. This is the default state used by
    /// `ApplicationFacade::new` and `ApplicationFacade::with_source`.
    pub fn with_builtins() -> Self {
        let mut c = Self::new();
        c.register(CapabilityDescriptor::new(
            "scan",
            "scan native source and rebuild projection",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "resource",
            "per-resource CRUD, query, read, list",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "link",
            "link occurrence and resolution queries",
            Mutability::Read,
        ));
        c.register(CapabilityDescriptor::new(
            "task",
            "agenda, PARA overview, state transitions",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "attachment",
            "attachment add, extraction, segment query",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "community",
            "community create, list",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "artifact",
            "derived artifacts (summary, llms.txt, context-pack, skill)",
            Mutability::Read,
        ));
        c.register(CapabilityDescriptor::new(
            "sync",
            "sync push, pull, relay, conflict list",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "inspect",
            "rule inspection, doctor, jobs, artifact freshness",
            Mutability::Read,
        ));
        c
    }

    /// Register `descriptor`. If a descriptor already exists for
    /// `descriptor.id`, the new one replaces it.
    pub fn register(&mut self, descriptor: CapabilityDescriptor) {
        self.descriptors.insert(descriptor.id, descriptor);
    }

    /// Look up a descriptor by id.
    pub fn get(&self, id: &str) -> Option<&CapabilityDescriptor> {
        self.descriptors.get(id)
    }

    /// List all registered descriptors in arbitrary order.
    pub fn list(&self) -> Vec<&CapabilityDescriptor> {
        self.descriptors.values().collect()
    }

    /// True if a descriptor is registered for `id`.
    pub fn contains(&self, id: &str) -> bool {
        self.descriptors.contains_key(id)
    }

    /// Iterate over `(id, descriptor)` pairs in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &CapabilityDescriptor)> {
        self.descriptors.iter().map(|(id, d)| (*id, d))
    }
}

/// Stable JSON shape used by both the CLI `list-capabilities` subcommand
/// and the MCP `list_capabilities` tool. The exact key set is part of
/// the public contract for capability listings:
///
/// ```json
/// { "id": "<static str>",
///   "description": "<static str>",
///   "mutability": "read" | "write" }
/// ```
///
/// `CapabilityDescriptor::to_json` produces a `serde_json::Value` in this
/// shape, and [`catalog_to_json_array`] renders a full catalog as an
/// array of such entries.
impl CapabilityDescriptor {
    /// Render this descriptor as a `serde_json::Value` in the stable
    /// public shape.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "description": self.description,
            "mutability": match self.mutability {
                Mutability::Read => "read",
                Mutability::Write => "write",
            },
        })
    }
}

/// Render a full [`CapabilityCatalog`] as a JSON array of
/// [`CapabilityDescriptor::to_json`] entries. Used by the CLI
/// `list-capabilities` subcommand and the MCP `list_capabilities`
/// tool so both surfaces emit byte-identical payloads.
pub fn catalog_to_json_array(catalog: &CapabilityCatalog) -> serde_json::Value {
    let entries: Vec<serde_json::Value> = catalog
        .list()
        .into_iter()
        .map(CapabilityDescriptor::to_json)
        .collect();
    serde_json::Value::Array(entries)
}

#[cfg(test)]
mod json_shape_tests {
    use super::*;

    #[test]
    fn descriptor_to_json_uses_stable_keys() {
        let desc = CapabilityDescriptor::new("scan", "scan source", Mutability::Read);
        let v = desc.to_json();
        assert_eq!(v["id"], "scan");
        assert_eq!(v["description"], "scan source");
        assert_eq!(v["mutability"], "read");
    }

    #[test]
    fn write_mutability_serialises_as_write() {
        let desc = CapabilityDescriptor::new("task", "task", Mutability::Write);
        assert_eq!(desc.to_json()["mutability"], "write");
    }

    #[test]
    fn catalog_to_json_array_emits_one_entry_per_descriptor() {
        let cat = CapabilityCatalog::with_builtins();
        let arr = catalog_to_json_array(&cat);
        let entries = arr.as_array().expect("array");
        assert_eq!(entries.len(), 9);
        for entry in entries {
            assert!(entry.get("id").is_some());
            assert!(entry.get("description").is_some());
            assert!(entry.get("mutability").is_some());
        }
    }
}
