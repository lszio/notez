//! `GraphPanel` — the bottom half of the right column.
//!
//! On a detail page it renders the 1-hop neighborhood graph of the
//! active resource. On every other page it renders nothing — the
//! full-space graph has its own `/space/:encoded/graph` route.

use dioxus::prelude::*;
use notez_core::application::{layout_force, Graph};

use crate::router::{encode_space, route_for_space_resource};
use crate::server::neighbor_graph;
use crate::space_ctx::{SpaceState, SpaceStatus};

const SVG_W: f64 = 280.0;
const SVG_H: f64 = 200.0;
const SVG_ITERS: usize = 100;

#[component]
pub fn GraphPanel(active_encoded: Option<String>) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());
    let resource_ref = use_context::<Signal<Option<String>>>();

    let focus = resource_ref.cloned().unwrap_or_default();
    let path_for_graph = active_path.clone();
    let focus_for_graph = focus.clone();

    let graph_resource = use_server_future(move || {
        let p = path_for_graph.clone();
        let f = focus_for_graph.clone();
        async move {
            match (p, f) {
                (Some(p), f) if !f.is_empty() => neighbor_graph(p, f).await.unwrap_or(Graph {
                    nodes: vec![],
                    edges: vec![],
                    total_nodes: 0,
                    truncated: false,
                }),
                _ => Graph {
                    nodes: vec![],
                    edges: vec![],
                    total_nodes: 0,
                    truncated: false,
                },
            }
        }
    })?;

    let graph = graph_resource.cloned().unwrap_or(Graph {
        nodes: vec![],
        edges: vec![],
        total_nodes: 0,
        truncated: false,
    });

    let encoded = active_encoded.clone().unwrap_or_default();

    rsx! {
        div { class: "graph-panel",
            div { class: "graph-head",
                span { class: "graph-label", "neighborhood" }
                if !focus.is_empty() {
                    span { class: "graph-count mono-sm",
                        "{graph.nodes.len()} nodes · {graph.edges.len()} edges"
                    }
                }
            }
            div { class: "graph-body",
                if focus.is_empty() {
                    p { class: "graph-empty",
                        "open a resource to see its 1-hop neighborhood."
                    }
                } else if graph.nodes.is_empty() {
                    p { class: "graph-empty", "no relations" }
                } else {
                    NeighborhoodSvg { graph: graph.clone(), encoded: encoded.clone(), focus: focus.clone() }
                }
            }
        }
    }
}

#[component]
fn NeighborhoodSvg(graph: Graph, encoded: String, focus: String) -> Element {
    let positions = layout_force(&graph, SVG_W, SVG_H, SVG_ITERS);

    rsx! {
        svg {
            class: "np-svg",
            view_box: "0 0 {SVG_W} {SVG_H}",
            width: "100%",
            role: "img",
            "aria-label": "neighborhood graph",
            for e in graph.edges.iter() {
                {
                    let (sx, sy) = positions.get(&e.source).copied().unwrap_or((0.0, 0.0));
                    let (tx, ty) = positions.get(&e.target).copied().unwrap_or((0.0, 0.0));
                    let cls = if e.kind == "ok" { "" } else { "is-active" };
                    rsx! {
                        line { class: "np-link {cls}", x1: "{sx}", y1: "{sy}", x2: "{tx}", y2: "{ty}" }
                    }
                }
            }
            for n in graph.nodes.iter() {
                {
                    let (x, y) = positions.get(&n.ref_str).copied().unwrap_or((SVG_W / 2.0, SVG_H / 2.0));
                    let is_focus = n.ref_str == focus;
                    let state_cls = if is_focus { "is-focus" } else { "is-neighbor" };
                    let r = if is_focus { 5.0 } else { 3.0 };
                    let label: String = n.title.chars().take(8).collect();
                    let href = if encoded.is_empty() {
                        "/".to_string()
                    } else {
                        let space = crate::router::decode_space(&encoded);
                        route_for_space_resource(&space, &n.ref_str)
                    };
                    rsx! {
                        a { href: "{href}",
                            g { transform: "translate({x},{y})",
                                circle { class: "np-node {state_cls}", r: "{r}", cx: "0", cy: "0" }
                                text { class: "np-label {state_cls}", y: "{r + 8.0}", "{label}" }
                            }
                        }
                    }
                }
            }
        }
    }
}