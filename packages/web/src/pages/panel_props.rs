//! `PropertiesPanel` — the top half of the right column.

use dioxus::prelude::*;

use crate::model::ResourceRow;
use crate::server::{get_resource, list_kind_counts};
use crate::space_ctx::{SpaceState, SpaceStatus};
use crate::tree::KindCounts;

#[component]
pub fn PropertiesPanel(active_encoded: Option<String>) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());

    let resource_ref = use_context::<Signal<Option<String>>>();

    let ref_str = resource_ref.cloned().unwrap_or_default();
    let active_path_for_res = active_path.clone();
    let active_path_for_counts = active_path.clone();

    let resource = use_server_future(move || {
        let p = active_path_for_res.clone();
        let r = ref_str.clone();
        async move {
            match (p, r) {
                (Some(path), r) if !r.is_empty() => get_resource(path, r).await,
                _ => Ok(None),
            }
        }
    })?;

    let counts_resource = use_server_future(move || {
        let p = active_path_for_counts.clone();
        async move {
            match p {
                Some(p) => list_kind_counts(p).await,
                None => Ok(KindCounts::default()),
            }
        }
    })?;

    let counts: Option<KindCounts> = counts_resource.cloned().and_then(|r| r.ok());
    let row: Option<ResourceRow> = resource.cloned().and_then(|r| r.ok()).flatten();

    let space_status = space().map(|s| s.status.clone());

    rsx! {
        div { class: "props-panel",
            div { class: "props-head",
                span { class: "props-label", "properties" }
            }
            div { class: "props-body",
                if let Some(row) = row.clone() {
                    div { class: "props-detail",
                        div { class: "props-title", "{row.title}" }
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
                } else if let Some(c) = counts.clone() {
                    div { class: "props-rollup",
                        div { class: "props-rollup-title", "kind rollup" }
                        dl { class: "props-list",
                            div { class: "props-row",
                                dt { "documents" }
                                dd { "{c.document}" }
                            }
                            div { class: "props-row",
                                dt { "headings" }
                                dd { "{c.heading}" }
                            }
                            div { class: "props-row",
                                dt { "attachments" }
                                dd { "{c.attachment}" }
                            }
                            div { class: "props-row",
                                dt { "blocks" }
                                dd { "{c.block}" }
                            }
                            div { class: "props-row",
                                dt { "total" }
                                dd { "{c.total()}" }
                            }
                        }
                    }
                } else {
                    {
                        match space_status {
                            Some(SpaceStatus::Resolving) => rsx! { p { class: "props-empty", "loading…" } },
                            Some(SpaceStatus::Error(e)) => rsx! { p { class: "props-empty err-text", "error: {e}" } },
                            _ => rsx! { p { class: "props-empty", "no space selected." } },
                        }
                    }
                }
            }
        }
    }
}