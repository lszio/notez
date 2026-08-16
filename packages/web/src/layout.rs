//! App shell.
//!
//! The Layout is the top-level container that holds the error
//! banner, the left sidebar, and the per-page main column. The
//! `space_ctx` signal is provided here so the sidebar, error
//! banner, and pages can all read the active space's metadata.

use dioxus::prelude::*;

use crate::pages::SpaceSidebar;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn Layout(children: Element) -> Element {
    use_context_provider(|| Signal::new(None::<SpaceState>));
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space_ctx().map(|s| s.path.clone());

    rsx! {
        div { class: "shell",
            // ----- Global error banner (above the spine) -----
            {
                let s = space_ctx();
                if let Some(SpaceState { status: SpaceStatus::Error(e), path, .. }) = s.clone() {
                    rsx! {
                        div { class: "banner-err",
                            div { class: "banner-err-inner",
                                span { class: "label", "space error →" }
                                span { class: "mono-sm", "{path}" }
                                span { class: "label", "—" }
                                span { "{e}" }
                            }
                        }
                    }
                } else {
                    rsx! { Fragment {} }
                }
            }

            div { class: "shell-body",
                SpaceSidebar { active_path }
                main { class: "main",
                    {children}
                }
            }
        }
    }
}
