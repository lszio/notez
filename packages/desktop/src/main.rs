//! Dioxus desktop shell for Notez.
//!
//! Renders a small Workspace home view that:
//! 1. Lists every registered space (EmbeddedBackend::list_spaces).
//! 2. Picks the default space, scans it once, then queries resources
//!    through the shared ui::Backend trait.
//! 3. Renders each resource as a NzCard with a NzBadge for kind.
//!
//! The same ui::Backend trait will be reused by the upcoming mobile
//! surface — desktop just happens to implement it with an in-process
//! engine (no HTTP).

use dioxus::prelude::*;

mod backend;
mod views;

use backend::EmbeddedBackend;

const MAIN_CSS: Asset = asset!("/assets/main.css");
use views::Home;

fn default_space_root() -> Option<std::path::PathBuf> {
    if let Ok(env_root) = std::env::var("NOTEZ_DEFAULT_SPACE") {
        let p = std::path::PathBuf::from(env_root);
        if p.exists() {
            return Some(p);
        }
    }
    let cwd = std::env::current_dir().ok()?;
    if cwd.join("notez.toml").exists() {
        Some(cwd)
    } else {
        None
    }
}

fn main() {
    let default_root = default_space_root();
    let backend = EmbeddedBackend::new(default_root.clone());
    let initial_space = default_root
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    use_context_provider(move || backend);
    use_context_provider(move || initial_space);

    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        // The workspace shell/tree/document styles are shared with the
        // web surface (packages/ui/assets/workspace.css).
        style { dangerous_inner_html: ui::WORKSPACE_CSS }
        Router::<Route> {}
    }
}

#[derive(Debug, Clone, Routable, PartialEq, Eq)]
#[rustfmt::skip]
enum Route {
    #[layout(WorkspaceNavbar)]
    #[route("/")]
    Home {},
}

#[component]
fn WorkspaceNavbar() -> Element {
    rsx! {
        ui::Navbar {
            Link { to: Route::Home {}, "Workspace" }
        }
        Outlet::<Route> {}
    }
}
