//! `web` — notez dioxus fullstack web client (v0.1, reader-only).
//!
//! Three routes:
//! - `/`                                     — picker home (onboarding card)
//! - `/space/:encoded/list`                  — resource list inside a space
//! - `/space/:encoded/resource/:encoded_ref` — resource detail inside a space
//!
//! Spaces are picked at runtime through the picker in the header; the
//! `space` path segment is the urlencoded absolute path of the space
//! root. The Dioxus fullstack runtime is wired in via the `fullstack`
//! feature; `LaunchBuilder::new().launch(app)` boots SSR + hydration.

#![allow(non_snake_case)]

use dioxus::prelude::*;

pub mod layout;
pub mod model;
pub mod pages;
pub mod router;
pub mod server;
pub mod space_ctx;

pub use router::Route;

#[component]
fn App() -> Element {
    rsx! {
        crate::layout::Layout {
            crate::router::Router {}
        }
    }
}

pub fn app() -> Element {
    rsx! { App {} }
}
