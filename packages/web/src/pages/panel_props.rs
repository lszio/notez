//! `PropertiesPanel` — the active note's metadata in the note-page
//! right rail. Takes the ref as a prop (SSR-deterministic; no context
//! signal dependency).

use dioxus::prelude::*;

use crate::server::get_resource;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn PropertiesPanel(active_encoded: Option<String>, active_ref: String) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());

    let active_path_for_res = active_path.clone();
    let ref_for_res = active_ref.clone();
    let resource = use_server_future(move || {
        let p = active_path_for_res.clone();
        let r = ref_for_res.clone();
        async move {
            match (p, r.is_empty()) {
                (Some(p), false) => get_resource(p, r).await.ok().flatten(),
                _ => None,
            }
        }
    })?;

    let row: Option<crate::model::ResourceRow> = resource.cloned().flatten();
    let space_status = space().map(|s| s.status.clone());

    rsx! {
        section { class: "rail-card", "data-rail": "properties",
            h3 { class: "rail-card-title", "Properties" }
            div { class: "rail-card-body",
                if let Some(row) = row {
                    div { class: "props-detail",
                        div { class: "props-ref mono-sm", "{row.ref_str}" }
                        div { class: "props-meta",
                            div { class: "props-meta-row",
                                span { class: "props-meta-k", "kind" }
                                span { class: "props-meta-v", "{row.kind}" }
                            }
                            div { class: "props-meta-row",
                                span { class: "props-meta-k", "locator" }
                                span { class: "props-meta-v mono-sm", "{row.locator}" }
                            }
                            div { class: "props-meta-row",
                                span { class: "props-meta-k", "source" }
                                span { class: "props-meta-v", "{row.source_id}" }
                            }
                            div { class: "props-meta-row",
                                span { class: "props-meta-k", "revision" }
                                span { class: "props-meta-v mono-sm", "{row.revision}" }
                            }
                        }
                        if !row.properties.is_empty() {
                            div { class: "props-section-title", "properties" }
                            dl { class: "props-list",
                                for (k, v) in row.properties.iter() {
                                    div { class: "props-row",
                                        dt { "{k}" }
                                        dd { "{v}" }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    match space_status {
                        Some(SpaceStatus::Resolving) => rsx! { p { class: "rail-empty", "loading…" } },
                        Some(SpaceStatus::Error(e)) => rsx! { p { class: "rail-empty err-text", "error: {e}" } },
                        _ => rsx! { p { class: "rail-empty", "no properties" } },
                    }
                }
            }
        }
    }
}
