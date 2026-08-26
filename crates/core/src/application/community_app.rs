//! Community-use-case persistence wrapper.
//!
//! The JSON-on-disk cache lives in `storage::cache::SpaceCommunitiesConfig`
//! (infrastructure layer). This module re-exports the typed shape for
//! application-side callers and provides no behavior of its own.

pub use crate::storage::cache::SpaceCommunitiesConfig;