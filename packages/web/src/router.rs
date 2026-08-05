use dioxus::prelude::*;

use crate::pages::{DetailPage, ListPage};

#[derive(Routable, Clone, Debug, PartialEq)]
pub enum Route {
    #[route("/", ListPage)]
    List {},
    #[route("/resource/:encoded_ref", DetailPage)]
    Detail { encoded_ref: String },
}

/// Top-level router. Wraps the `dioxus_router::Router` (which provides
/// the routing context) and renders the current `Route` variant.
#[component]
pub fn Router() -> Element {
    rsx! {
        dioxus_router::Router::<Route> {}
    }
}
