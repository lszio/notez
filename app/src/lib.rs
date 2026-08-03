//! `app` — notez dioxus fullstack web client (v0.1, reader-only).

#![allow(non_snake_case)]

use dioxus::prelude::*;

mod model;
mod pages;
mod router;
mod server;

/// Dioxus fullstack application root.
///
/// Server-renders the first response, then hydrates the client.
/// The full `Router::<Route>` wiring is added in Task 4 once the
/// `router` and `pages` modules are populated.
pub fn app() -> Element {
    rsx! {
        div { "notez v0.1" }
    }
}
