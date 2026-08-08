//! PageHeader — the "notebook spine" bar that tops every page.
//!
//! Renders the brand on the left, the active-space status (or a
//! "no space" hint) in the middle, and the SpacePicker trigger on
//! the right. A thin hairline under the bar separates it from the
//! page content.

use dioxus::prelude::*;

use crate::pages::SpacePicker;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn PageHeader() -> Element {
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();

    let space_label = match space_ctx() {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => format!("space · {}", s.name),
        Some(SpaceState { status: SpaceStatus::Resolving, .. }) => "space · resolving…".to_string(),
        Some(SpaceState { status: SpaceStatus::Error(_), .. }) => "space · error".to_string(),
        None => "no space selected".to_string(),
    };

    rsx! {
        header { class: "spine",
            div { class: "spine-inner",
                a { class: "brand", href: "/", "Notez" }
                span { class: "spine-meta", "{space_label}" }
                div { class: "picker",
                    SpacePicker {}
                }
            }
        }
    }
}
