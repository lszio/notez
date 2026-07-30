//! Dioxus server-renderable components for the `notez` web frontend.
//!
//! Each module exposes one or more `#[component]` functions that the
//! `crates/web` server composes into SSR HTML. The rendering is
//! intentionally plain — no client-side runtime is needed; the generated
//! HTML is suitable for axum responses.

use serde::{Deserialize, Serialize};

/// Top-level summary of a `notez` space as displayed in the hub index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceSummary {
    pub name: String,
    pub root: std::path::PathBuf,
}

pub mod escape;
pub mod layout;
pub mod agenda_list;
pub mod attachment_panel;
pub mod resource_card;
pub mod space_hub;
pub mod space_list;
pub mod outline;
pub mod code_block;
pub mod link_embed;
pub mod block_embed;
pub mod mermaid_block;
pub mod d2_block;
pub mod iframe_block;