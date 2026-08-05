//! `web` — notez dioxus fullstack web client (v0.1, reader-only).
//!
//! Two routes:
//! - `/`                    — resource list
//! - `/resource/<encoded_ref>` — resource detail
//!
//! The fullstack runtime (axum + tower) is wired in by Dioxus through
//! the `fullstack` feature; `LaunchBuilder::new().launch(app)` boots
//! the SSR + hydration pipeline. Server functions in `server` read
//! `NOTEZ_SPACE_ROOT` and open the SQLite projection lazily per call.

#![allow(non_snake_case)]

use dioxus::prelude::*;

pub mod model;
pub mod pages;
pub mod router;
pub mod server;

pub use router::Route;

#[component]
fn App() -> Element {
    rsx! {
        crate::router::Router {}
    }
}

/// Dioxus fullstack application root.
///
/// Server-renders the first response, then hydrates the client.
pub fn app() -> Element {
    rsx! { App {} }
}
