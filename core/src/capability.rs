//! Public capability descriptors.
//!
//! This module exists so that subsequent P1 capability-directory work
//! has a stable public surface to extend. The current revision only
//! ships the data type and the [`Mutability`] enum. `ApplicationFacade`
//! accepts a descriptor via `register_capability` but the call is a
//! no-op; it is intentionally a future hook, not a behavior.

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
/// previewer registrations will eventually consume. Today they are only
/// accepted by `ApplicationFacade::register_capability` as a no-op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub id: &'static str,
    pub description: &'static str,
    pub mutability: Mutability,
}

impl CapabilityDescriptor {
    pub fn new(
        id: &'static str,
        description: &'static str,
        mutability: Mutability,
    ) -> Self {
        Self { id, description, mutability }
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
        Self { descriptors: HashMap::new() }
    }

    /// Construct a catalog pre-populated with the nine built-in
    /// capabilities. This is the default state used by
/// `ApplicationFacade::new` and `ApplicationFacade::with_space`.
    pub fn with_builtins() -> Self {
        let mut c = Self::new();
        c.register(CapabilityDescriptor::new(
            "scan",
            "scan native source and rebuild projection",
            Mutability::Read,
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
