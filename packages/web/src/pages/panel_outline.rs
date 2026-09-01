//! `OutlinePanel` — document headings for the note page right rail.
//!
//! The outline is parsed server-side from the raw note source (see
//! `server::parse_outline`), so it renders deterministically during
//! SSR. Clicking an entry scrolls to the Nth heading of the rendered
//! body — that mapping is wired by the progressive-enhancement script
//! in `public/index.html` (`[data-outline-idx]` → `querySelectorAll`).

use dioxus::prelude::*;

use crate::server::{get_outline, OutlineItemDto};
use crate::space_ctx::SpaceState;

#[component]
pub fn OutlinePanel(active_encoded: Option<String>, active_ref: String) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());
    let ref_str = active_ref.clone();

    let path_for_fetch = active_path.clone();
    let ref_for_fetch = ref_str.clone();
    let outline_resource = use_server_future(move || {
        let p = path_for_fetch.clone();
        let r = ref_for_fetch.clone();
        async move {
            match (p, r.is_empty()) {
                (Some(p), false) => get_outline(p, r).await.unwrap_or_default(),
                _ => Vec::new(),
            }
        }
    })?;

    let items: Vec<OutlineItemDto> = outline_resource.cloned().unwrap_or_default();

    rsx! {
        section { class: "rail-card", "data-rail": "outline",
            h3 { class: "rail-card-title", "Outline" }
            if items.is_empty() {
                p { class: "rail-empty", "no headings" }
            } else {
                ul { class: "outline-list",
                    for (idx, item) in items.iter().enumerate() {
                        li { class: "outline-item outline-l{item.level}",
                            button {
                                class: "outline-link",
                                "data-outline-idx": "{idx}",
                                r#type: "button",
                                "{item.text}"
                            }
                        }
                    }
                }
            }
        }
    }
}
