//! Resource detail page.
//!
//! v0.2 surfaces the active resource's ref into the shared
//! `Signal<Option<String>>` context so the right-rail properties +
//! graph panels can render. The body preview comes from
//! `get_resource` (server-rendered markdown / org).

use dioxus::prelude::*;

use crate::pages::ui::{Breadcrumb, BreadcrumbSegment, KindIcon};
use crate::pages::use_space_layout;
use crate::router::{route_for_space_list, route_for_space_home};
use crate::server::get_resource;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn DetailPage(encoded: String, encoded_ref: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let decoded_ref = crate::router::decode_space(&encoded_ref);

    // Publish the active ref so the right rail can render.
    let mut resource_ref_ctx = use_context::<Signal<Option<String>>>();
    resource_ref_ctx.set(Some(decoded_ref.clone()));

    let space_snapshot = space().clone();
    let path_for_fetch = space_snapshot.as_ref().map(|s| s.path.clone());
    let current_encoded = space_snapshot
        .as_ref()
        .map(|s| s.encoded.clone())
        .unwrap_or_default();
    let source_name = match space_snapshot.as_ref() {
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

    let decoded_space = crate::router::decode_space(&current_encoded);
    let list_href = route_for_space_list(&decoded_space);
    let home_href = route_for_space_home(&decoded_space);

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
        div { class: "page",
            Breadcrumb {
                segments: vec![
                    BreadcrumbSegment::link("notez", "/"),
                    BreadcrumbSegment::link(home_href.clone(), home_href.clone()),
                    BreadcrumbSegment::link(source_name.clone(), list_href.clone()),
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
                            "Source status: "
                            {match &s.status {
                                SpaceStatus::Resolving => "resolving…".to_string(),
                                SpaceStatus::Ready(_) => "ready".to_string(),
                                SpaceStatus::Error(e) => format!("error: {e}"),
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
                        current_encoded: current_encoded.clone(),
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
fn DetailBody(row: crate::model::ResourceRow, list_href: String, current_encoded: String) -> Element {
    let decoded_space = crate::router::decode_space(&current_encoded);
    rsx! {
        div { class: "detail-main",
            h1 { "{row.title}" }
            p { class: "ref-line",
                "{row.ref_str}"
                span { class: "dim", "  ·  " }
                "← "
                a { href: "{list_href}", "back to index" }
            }

            if !row.body_html.is_empty() {
                div { class: "detail-body",
                    div { dangerous_inner_html: "{row.body_html}" }
                }
            }

            details { class: "meta-drawer",
                summary { "metadata" }
                div { class: "meta-drawer-body",
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
                    }
                }
            }

            h2 { "Properties" }
            if row.properties.is_empty() {
                p { class: "props-empty", "no properties attached to this resource." }
            } else {
                div { class: "props",
                    for (k, v) in row.properties.iter() {
                        div { class: "row",
                            span { class: "k", "{k}" }
                            span { class: "v", "{v}" }
                        }
                    }
                }
            }
        }
    }
}