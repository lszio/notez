//! PageHeader — the sticky "notebook spine" bar.
//!
//! v0.3 of the web client keeps the spine very small: brand on the
//! left, the active-space hint in the middle (and a couple of
//! per-space action buttons when a space is active), and nothing on
//! the right — the spaces list lives in the left sidebar, not here.

use dioxus::prelude::*;

use crate::router::{encode_space, route_for_space_list, route_for_space_graph};
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn PageHeader() -> Element {
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();

    let s = space_ctx();
    let (label, name) = match s.as_ref() {
        Some(SpaceState { status: SpaceStatus::Ready(dto), .. }) => {
            (format!("{}", dto.name), Some(dto.name.clone()))
        }
        Some(SpaceState { status: SpaceStatus::Resolving, .. }) => {
            ("resolving…".to_string(), None)
        }
        Some(SpaceState { status: SpaceStatus::Error(_), .. }) => {
            ("space error".to_string(), None)
        }
        None => ("no space".to_string(), None),
    };
    let active_path = s.as_ref().map(|s| s.path.clone());
    let active_encoded = s.as_ref().map(|s| s.encoded.clone());

    rsx! {
        header { class: "spine",
            div { class: "spine-inner",
                a { class: "brand", href: "/", "Notez" }
                span { class: "spine-meta",
                    if let Some(n) = name {
                        "space · "
                        span { class: "cur-space", "{n}" }
                    } else {
                        "{label}"
                    }
                }
                div { class: "spine-actions",
                    if let (Some(p), Some(_)) = (active_path, active_encoded.as_ref()) {
                        // Per-space actions: scan, watch, graph.
                        form {
                            class: "spine-form",
                            action: "/api/spaces/scan",
                            method: "post",
                            input {
                                r#type: "hidden",
                                name: "space_root",
                                value: "{p}",
                            }
                            button {
                                class: "spine-action",
                                r#type: "submit",
                                title: "Re-index the space",
                                "scan"
                            }
                        }
                        a {
                            class: "spine-action",
                            href: "{route_for_space_list(&p)}",
                            title: "Resource list",
                            "list"
                        }
                        if let Some(enc) = active_encoded.as_ref() {
                            a {
                                class: "spine-action",
                                href: "{route_for_space_graph(enc)}",
                                title: "Graph view",
                                "graph"
                            }
                        }
                    } else {
                        a { class: "spine-action", href: "/", "home" }
                    }
                }
            }
        }
    }
}
