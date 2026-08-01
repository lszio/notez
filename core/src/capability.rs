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
