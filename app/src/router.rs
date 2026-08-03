use dioxus::prelude::*;

use crate::pages::{DetailPage, ListPage};

/// Top-level URL → page dispatcher.
///
/// Two routes:
/// - `/`                          → `ListPage`, the resource index.
/// - `/resource/:encoded_ref`     → `DetailPage`, one resource's properties.
///
/// Dioxus 0.6 derives `Routable` to get the URL parser / matcher; we use
/// `use_route::<Route>()` inside the `Router` component to read the
/// current match and `rsx!` the right page component.
#[derive(Routable, Clone, Debug, PartialEq)]
pub enum Route {
    #[route("/", ListPage)]
    List {},
    #[route("/resource/:encoded_ref", DetailPage)]
    Detail { encoded_ref: String },
}

#[component]
pub fn Router() -> Element {
    let route = use_route::<Route>();
    match route {
        Route::List {} => rsx! { ListPage {} },
        Route::Detail { encoded_ref } => rsx! { DetailPage { encoded_ref } },
    }
}
