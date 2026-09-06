//! Dioxus mobile shell for Notez.
//!
//! Talks to a remote notez server through [`HttpBackend`]. The user
//! configures the host via `NOTEZ_REMOTE_URL` (defaults to
//! `http://127.0.0.1:8700`) and optionally `NOTEZ_DEFAULT_SOURCE`;
//! otherwise the workspace view asks the server for its default
//! source and lists resources from there.

use dioxus::prelude::*;

mod backend;
mod views;

use views::Workspace;
use backend::HttpBackend;

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    let backend = HttpBackend::from_env()
        .unwrap_or_else(|err| panic!("notez mobile: cannot build HttpBackend: {err}"));

    use_context_provider(move || backend);

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(MobileNavbar)]
    #[route("/")]
    Workspace {},
}

#[component]
fn MobileNavbar() -> Element {
    rsx! {
        ui::Navbar {
            Link {
                to: Route::Workspace {},
                "Workspace"
            }
        }
        Outlet::<Route> {}
    }
}