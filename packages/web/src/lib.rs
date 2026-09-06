//! `web` — notez dioxus fullstack web client (v0.2, reader-only).
//!
//! Five routes:
//! - `/`                                       — picker home (onboarding card)
//! - `/source/:encoded`                         — per-space welcome (renders `index.org` when present)
//! - `/source/:encoded/list`                    — resource list inside a space
//! - `/source/:encoded/resource/:encoded_ref`   — resource detail inside a space
//! - `/source/:encoded/graph`                   — full-space force-directed graph
//!
//! Spaces are picked at runtime through the picker in the header; the
//! `space` path segment is the base64-urlsafe-encoded absolute path
//! of the space root. The Dioxus fullstack runtime is wired in via
//! the `fullstack` feature; `LaunchBuilder::new().launch(app)` boots
//! SSR + hydration.

use dioxus::prelude::*;
pub use crate::router::Route;
pub mod layout;
pub mod janet;
pub mod body;
pub mod model;
pub mod pages;
pub mod router;
#[cfg(feature = "server")]
pub mod routes;
pub mod server;
#[cfg(feature = "server")]
pub mod host;
pub mod space_ctx;
// The `tree` module defines types used by the SSR server-side
// aggregation helpers; the wasm client has no reason to depend on
// them. Gating also keeps the workspace clean of any platform-only
// references the client doesn't need.
#[cfg(not(target_arch = "wasm32"))]
pub mod tree;
// UI configuration is a server-only concern: the wasm client never
// reads `web.toml` (its server fn bodies run server-side anyway).
// Gating the module keeps `toml` out of the client dependency graph.
#[cfg(not(target_arch = "wasm32"))]
pub mod ui_config;

#[component]
fn App() -> Element {
    rsx! {
        crate::layout::Layout {
            crate::router::AppRouter::<Route> {}
        }
    }
}

pub fn app() -> Element {
    rsx! { App {} }
}
