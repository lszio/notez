//! Dioxus application: the workspace's rendering core.
//!
//! Pages are server-rendered from data read during the render (no
//! `#[server]` functions, no client-side data fetching), so the HTML
//! the browser receives is complete and the "stuck loading" class of
//! bug cannot occur. Interaction that HTML alone cannot express lives
//! in the `app.js` island.

pub mod edit;
pub mod pages;
pub mod route;
pub mod shell;

use dioxus::prelude::*;
pub use route::Route;

/// Root component. Styles and the island script are served by
/// [`crate::data`] as embedded assets.
#[component]
pub fn App() -> Element {
    let version = crate::data::ASSET_VERSION;
    rsx! {
        document::Link { rel: "stylesheet", href: "/app.css?v={version}" }
        document::Link {
            rel: "icon",
            href: "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'><text y='26' font-size='26'>📝</text></svg>"
        }
        Router::<Route> {}
        script { defer: true, src: "/app.js?v={version}" }
    }
}
