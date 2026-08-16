//! Resource detail page.
//!
//! Layout: spine bar, breadcrumb (notez · space · kind · ref),
//! then a 2-column grid — a narrow monospace meta sidebar on the
//! left (kind / source / locator / revision / object_id) and the
//! main column on the right (h1 title, properties definition list).

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::model::ResourceRow;
use crate::pages::{use_space_layout, PageHeader};
use crate::pages::ui::{Breadcrumb, BreadcrumbSegment, KindIcon};
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
        div { class: "page",
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
fn DetailBody(row: ResourceRow, list_href: String, current_encoded: String) -> Element {
    rsx! {
        div { class: "detail-main",
            h1 { "{row.title}" }
            p { class: "ref-line",
                "{row.ref_str}"
                span { class: "dim", "  ·  " }
                "← "
                a { href: "{list_href}", "back to index" }
            }

            // Inline body preview (markdown/org rendered server-side).
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

            // Local neighborhood graph.
            div { class: "neighbor-graph",
                h2 { "Relations" }
                p { class: "ng-caption", "1-hop neighborhood" }
                NeighborGraph {
                    space_encoded: current_encoded.clone(),
                    focus_ref: row.ref_str.clone(),
                }
            }
        }
    }
}

#[component]
fn NeighborGraph(space_encoded: String, focus_ref: String) -> Element {
    use crate::server::neighbor_graph;
    use notez_core::application::{Graph, GraphEdge, GraphNode, layout_force};

    
    let focus_for_fetch = focus_ref.clone();
    let enc = space_encoded.clone();
    let graph_resource = use_server_future(move || {
        let f = focus_for_fetch.clone();
        let p = crate::router::decode_space(&enc);
        async move {
            neighbor_graph(p, f).await.unwrap_or(Graph { nodes: vec![], edges: vec![], total_nodes: 0, truncated: false })
        }
    })?;

    if let Some(graph) = graph_resource.cloned() {
        if graph.nodes.is_empty() {
            return rsx! { p { class: "skel", "no relations" } };
        }
        
        let w = 400.0;
        let h = 280.0;
        let positions = layout_force(&graph, w, h, 120);
        
        rsx! {
            svg {
                class: "ng-svg",
                view_box: "0 0 {w} {h}",
                width: "100%",
                role: "img",
                "aria-label": "local graph",
                for e in graph.edges.iter() {
                    {
                        let (sx, sy) = positions.get(&e.source).copied().unwrap_or((0.0, 0.0));
                        let (tx, ty) = positions.get(&e.target).copied().unwrap_or((0.0, 0.0));
                        let active = if e.kind == "ok" { "" } else { "is-active" };
                        rsx! {
                            line { class: "ng-link {active}", x1: "{sx}", y1: "{sy}", x2: "{tx}", y2: "{ty}" }
                        }
                    }
                }
                for n in graph.nodes.iter() {
                    {
                        let (x, y) = positions.get(&n.ref_str).copied().unwrap_or((w/2.0, h/2.0));
                        let is_focus = n.ref_str == focus_ref;
                        let state_cls = if is_focus { "is-focus" } else { "is-neighbor" };
                        let r = if is_focus { 8.0 } else { 5.0 };
                        let label: String = n.title.chars().take(12).collect();
                        let href = format!("/space/{}/resource/{}", space_encoded, crate::router::encode_space(&n.ref_str));
                        rsx! {
                            a { href: "{href}",
                                g { transform: "translate({x},{y})",
                                    circle { class: "ng-node {state_cls}", r: "{r}", cx: "0", cy: "0" }
                                    text { class: "ng-label {state_cls}", y: "{r + 9.0}", "{label}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        rsx! { p { class: "skel", "loading graph…" } }
    }
}
