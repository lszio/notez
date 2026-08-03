//! `app` — notez dioxus fullstack web client (v0.1, reader-only).
//!
//! The `app()` function is the Dioxus fullstack application root invoked by
//! `LaunchBuilder::new().launch(app)` in `bin/web.rs` (and `bin/server.rs`).
//! The actual tree is built by the inline `App` component, which mounts
//! `crate::router::Router` (a custom name — Dioxus also ships a `Router`
//! component, so we keep ours under `crate::router` to avoid clashing).

#![allow(non_snake_case)]

use dioxus::prelude::*;

mod model;
mod pages;
mod router;
mod server;

pub use router::Route;

#[component]
fn App() -> Element {
    rsx! {
        crate::router::Router {}
    }
}

/// Dioxus fullstack application root.
///
/// Server-renders the first response, then hydrates the client. The plan
/// wraps the tree in an `App` component so the `Router` symbol we use here
/// does not collide with the `Router` provided by `dioxus_router` (which
/// the `#[component]` import implicitly brings into scope).
pub fn app() -> Element {
    rsx! { App {} }
}
