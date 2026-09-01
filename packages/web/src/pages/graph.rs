//! `GraphPage` — the full-space force-directed graph view.
//!
//! `GraphSvg` is `pub`: the configurable home dashboard embeds the
//! same renderer as a mini widget with smaller dimensions.

use dioxus::prelude::*;
use notez_core::application::{layout_force, Graph, GraphEdge, GraphNode};

use crate::pages::use_space_layout;
use crate::router::{route_for_space_files, route_for_space_note};
use crate::server::list_graph;
use crate::space_ctx::{SpaceState, SpaceStatus};

const SVG_WIDTH: f64 = 900.0;
const SVG_HEIGHT: f64 = 600.0;
const SVG_ITERATIONS: usize = 220;

#[component]
pub fn GraphPage(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();
    let mut resource_ref_ctx = use_context::<Signal<Option<String>>>();
    use_effect(move || resource_ref_ctx.set(None));

    let active_path = space().map(|s| s.path.clone());
    let active_encoded_for_render = space().map(|s| s.encoded.clone());

    let active_path_for_fetch = active_path.clone();
    let graph_resource = use_server_future(move || {
        let p = active_path_for_fetch.clone();
        async move {
            match p {
                Some(p) => list_graph(p).await.unwrap_or_else(|_| Graph {
                    nodes: Vec::new(),
                    edges: Vec::new(),
                    total_nodes: 0,
                    truncated: false,
                }),
                None => Graph {
                    nodes: Vec::new(),
                    edges: Vec::new(),
                    total_nodes: 0,
                    truncated: false,
                },
            }
        }
    })?;

    let graph: Graph = graph_resource.cloned().unwrap_or(Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        total_nodes: 0,
        truncated: false,
    });

    let space_snapshot = space().clone();
    let (h1, lede) = match &space_snapshot {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => {
            (format!("{} · graph", s.name), "notes, headings and attachments and how they link.".to_string())
        }
        Some(SpaceState { status: SpaceStatus::Error(e), .. }) => {
            ("graph unavailable".to_string(), format!("{e}"))
        }
        _ => ("graph".to_string(), "resolve a space first.".to_string()),
    };

    let space_decoded = crate::router::decode_space(&active_encoded_for_render.clone().unwrap_or_default());

    rsx! {
        div { class: "page page-graph",
            header { class: "page-head",
                p { class: "page-eyebrow", "graph" }
                h1 { class: "page-title", "{h1}" }
                p { class: "page-lede", "{lede}" }
            }

            div { class: "graph-layout",
                aside { class: "graph-side",
                    dl { class: "graph-stats",
                        dt { "nodes shown" }
                        dd { "{graph.nodes.len()}" }
                        dt { "edges" }
                        dd { "{graph.edges.len()}" }
                        dt { "resources in space" }
                        dd { "{graph.total_nodes}" }
                        if graph.truncated {
                            dt { "truncated" }
                            dd { "yes (>500 nodes)" }
                        }
                    }
                    div { class: "graph-legend",
                        div { span { class: "swatch is-doc" } "document" }
                        div { span { class: "swatch is-hd" } "heading" }
                        div { span { class: "swatch is-att" } "attachment" }
                        div { span { class: "swatch is-blk" } "block" }
                    }
                    a { class: "btn", href: "{route_for_space_files(&space_decoded)}", "back to files" }
                }

                if graph.nodes.is_empty() {
                    div { class: "graph-empty",
                        "this space has no resources yet — add notes and rescan."
                    }
                } else {
                    GraphSvg {
                        graph: graph.clone(),
                        space_decoded: space_decoded.clone(),
                        width: SVG_WIDTH,
                        height: SVG_HEIGHT,
                        iterations: SVG_ITERATIONS,
                    }
                }
            }
        }
    }
}

/// Force-directed SVG graph. `space_decoded` is the plain source path
/// used to build note links.
#[component]
pub fn GraphSvg(
    graph: Graph,
    space_decoded: String,
    width: f64,
    height: f64,
    iterations: usize,
) -> Element {
    let positions = layout_force(&graph, width, height, iterations);
    rsx! {
        svg {
            class: "graph-svg",
            view_box: "0 0 {width} {height}",
            width: "100%",
            role: "img",
            "aria-label": "space resource graph",
            for e in graph.edges.iter() {
                GraphEdgeSvg {
                    edge: e.clone(),
                    positions: positions.clone(),
                }
            }
            for n in graph.nodes.iter() {
                GraphNodeSvg {
                    node: n.clone(),
                    positions: positions.clone(),
                    space_decoded: space_decoded.clone(),
                    width,
                    height,
                }
            }
        }
    }
}

#[component]
fn GraphEdgeSvg(
    edge: GraphEdge,
    positions: std::collections::BTreeMap<String, (f64, f64)>,
) -> Element {
    let (sx, sy) = positions.get(&edge.source).copied().unwrap_or((0.0, 0.0));
    let (tx, ty) = positions.get(&edge.target).copied().unwrap_or((0.0, 0.0));
    let cls = if edge.kind == "ok" { "" } else { "is-active" };
    rsx! {
        line { class: "graph-link {cls}", x1: "{sx}", y1: "{sy}", x2: "{tx}", y2: "{ty}" }
    }
}

#[component]
fn GraphNodeSvg(
    node: GraphNode,
    positions: std::collections::BTreeMap<String, (f64, f64)>,
    space_decoded: String,
    width: f64,
    height: f64,
) -> Element {
    let (x, y) = positions
        .get(&node.ref_str)
        .copied()
        .unwrap_or((width / 2.0, height / 2.0));
    let href = route_for_space_note(&space_decoded, &node.ref_str);
    let cls = match node.kind.as_str() {
        "heading" => "graph-node is-hd",
        "attachment" => "graph-node is-att",
        "block" => "graph-node is-blk",
        _ => "graph-node is-doc",
    };
    let label: String = node.title.chars().take(14).collect();
    rsx! {
        a { href: "{href}",
            g { transform: "translate({x},{y})",
                circle { class: "{cls}", r: "6", cx: "0", cy: "0" }
                text { class: "graph-label", y: "-9", "{label}" }
            }
        }
    }
}
