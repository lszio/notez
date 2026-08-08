//! App shell.
//!
//! The Layout is a thin context provider + error banner. The
//! header (with the SpacePicker that uses `use_navigator`) lives
//! inside each page component so it always renders as a
//! descendant of the Router context.
//!
//! What Layout does:
//! 1. Provides the `space_ctx` Signal that the per-page
//!    `use_space_layout` populates and the picker / error banner
//!    read.
//! 2. Renders a global error banner driven by
//!    `space_ctx.status`.
//! 3. Renders the page children below.

use dioxus::prelude::*;

use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn Layout(children: Element) -> Element {
    use_context_provider(|| Signal::new(None::<SpaceState>));
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();

    rsx! {
        div { class: "shell",
            // Global error banner driven by space_ctx.
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
            {children}
        }
    }
}
