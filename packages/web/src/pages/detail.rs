//! Resource detail page.
//!
//! Layout: spine bar, breadcrumb (notez · space · kind · ref),
//! then a 2-column grid — a narrow monospace meta sidebar on the
//! left (kind / source / locator / revision / object_id) and the
//! main column on the right (h1 title, properties definition list).

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::model::ResourceRow;
use crate::pages::{use_space_layout, Breadcrumb, BreadcrumbSegment, KindIcon, PageHeader};
use crate::router::route_for_space_list;
use crate::server::get_resource;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn DetailPage(encoded: String, encoded_ref: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let decoded_ref = crate::router::decode_space(&encoded_ref);

    // Snapshot everything we need into owned Strings before
    // building the future closure, so the closure captures by
    // move cleanly and the outer scope can still use the values
    // for the breadcrumb / sidebar.
    let space_snapshot = space().clone();
    let path_for_fetch = space_snapshot.as_ref().map(|s| s.path.clone());
    let current_encoded = space_snapshot
        .as_ref()
        .map(|s| s.encoded.clone())
        .unwrap_or_default();
    let space_name = match space_snapshot.as_ref() {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => s.name.clone(),
        _ => "?".to_string(),
    };

    let r_for_fetch = decoded_ref.clone();
    let resource = use_server_future(move || {
        let p = path_for_fetch.clone();
        let r = r_for_fetch.clone();
        async move {
            match p {
                Some(path) => get_resource(path, r).await,
                None => Ok(None),
            }
        }
    })?;

    let list_href = route_for_space_list(&crate::router::decode_space(&current_encoded));

    // Breadcrumb segments: notez / space / kind / ref-tail.
    let ref_tail = decoded_ref
        .rsplit_once(':')
        .map(|(_, id)| id.to_string())
        .unwrap_or_else(|| decoded_ref.clone());
    let ref_tail_short = if ref_tail.len() > 12 {
        format!("…{}", &ref_tail[ref_tail.len() - 12..])
    } else {
        ref_tail.clone()
    };

    let crumb_kind = match resource() {
        Some(Ok(Some(r))) => r.kind.clone(),
        _ => "?".to_string(),
    };

    rsx! {
        PageHeader {}
        main { class: "page",
            Breadcrumb {
                segments: vec![
                    BreadcrumbSegment::link("notez", "/"),
                    BreadcrumbSegment::link(space_name.clone(), list_href.clone()),
                    BreadcrumbSegment::link(crumb_kind.clone(), list_href.clone()),
                    BreadcrumbSegment::here(ref_tail_short.clone()),
                ],
            }

            match (space_snapshot.as_ref(), resource()) {
                (Some(s), _) if !matches!(s.status, SpaceStatus::Ready(_)) => rsx! {
                    div { class: "page-h",
                        p { class: "eyebrow", "space" }
                        h1 { "{decoded_ref}" }
                        p { class: "lede",
                            "Space 状态："
                            {match &s.status {
                                SpaceStatus::Resolving => "正在解析…".to_string(),
                                SpaceStatus::Ready(_) => "已就绪".to_string(),
                                SpaceStatus::Error(e) => format!("错误：{e}"),
                            }}
                        }
                    }
                },
                (_, Some(Err(e))) => rsx! {
                    div { class: "page-h",
                        p { class: "eyebrow", "error" }
                        h1 { "load failed" }
                        p { class: "lede err-text", "{e}" }
                    }
                },
                (_, Some(Ok(None))) => rsx! {
                    div { class: "page-h",
                        p { class: "eyebrow", "not found" }
                        h1 { "{decoded_ref}" }
                        p { class: "lede", "no resource with that ref in this space." }
                    }
                },
                (_, Some(Ok(Some(row)))) => rsx! {
                    DetailBody {
                        row: row.clone(),
                        list_href: list_href.clone(),
                    }
                },
                (_, None) => rsx! {
                    div { class: "page-h",
                        p { class: "skel", "loading…" }
                    }
                },
            }
        }
    }
}

#[component]
fn DetailBody(row: ResourceRow, list_href: String) -> Element {
    rsx! {
        div { class: "page-h",
            p { class: "eyebrow", "{row.kind}" }
            h1 { "{row.title}" }
            p { class: "lede mono-sm",
                "{row.ref_str}"
                span { class: "dim", "  ·  " }
                "←"
                a { href: "{list_href}", "back to index" }
            }
        }

        div { class: "detail-grid",
            aside { class: "detail-side",
                dl { class: "meta-list",
                    dt { "kind" }
                    dd {
                        KindIcon { kind: row.kind.clone() }
                        span { class: "mono-sm", "{row.kind}" }
                    }
                    dt { "source" }
                    dd { "{row.source_id}" }
                    dt { "locator" }
                    dd { class: "muted", "{row.locator}" }
                    dt { "revision" }
                    dd { class: "muted", "{row.revision}" }
                    dt { "object_id" }
                    dd { class: "muted", "{row.object_id}" }
                    dt { "ref" }
                    dd { class: "muted", "{row.ref_str}" }
                }
            }

            section { class: "detail-main",
                h2 { "Properties" }
                PropertiesView { properties: row.properties.clone() }
            }
        }

        div { class: "footer-rule",
            span { "notez · reader" }
            span { "·" }
            span { "details" }
        }
    }
}

#[component]
fn PropertiesView(properties: BTreeMap<String, String>) -> Element {
    let entries: Vec<(String, String)> = properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if entries.is_empty() {
        return rsx! {
            p { class: "props-empty", "no properties on this resource." }
        };
    }
    rsx! {
        div { class: "props",
            for (k, v) in entries.iter() {
                div { class: "row",
                    span { class: "k", "{k}" }
                    span { class: "v", "{v}" }
                }
            }
        }
    }
}
