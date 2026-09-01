//! `GraphPanel` — the 1-hop neighborhood graph in the note-page right
//! rail. Takes the focus ref as a prop (SSR-deterministic).

use dioxus::prelude::*;
use notez_core::application::{layout_force, Graph};

use crate::server::neighbor_graph;
use crate::space_ctx::SpaceState;

const SVG_W: f64 = 260.0;
const SVG_H: f64 = 180.0;
const SVG_ITERS: usize = 90;

#[component]
pub fn GraphPanel(active_encoded: Option<String>, active_ref: String) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());

    let path_for_graph = active_path.clone();
    let focus_for_graph = active_ref.clone();
    let graph_resource = use_server_future(move || {
        let p = path_for_graph.clone();
        let f = focus_for_graph.clone();
        async move {
            match (p, f.is_empty()) {
                (Some(p), false) => neighbor_graph(p, f).await.ok(),
                _ => None,
            }
        }
    })?;

    let graph = graph_resource
        .cloned()
        .flatten()
        .unwrap_or(Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            total_nodes: 0,
            truncated: false,
        });

    let space_decoded = crate::router::decode_space(&active_encoded.clone().unwrap_or_default());

    rsx! {
        section { class: "rail-card", "data-rail": "local-graph",
            h3 { class: "rail-card-title", "Local graph" }
            div { class: "rail-card-body rail-graph",
                if graph.nodes.len() > 1 {
                    svg {
                        class: "graph-svg graph-svg-mini",
                        view_box: "0 0 {SVG_W} {SVG_H}",
                        width: "100%",
                        role: "img",
                        "aria-label": "local graph",
                        MiniSvg { graph, space_decoded }
                    }
                } else {
                    p { class: "rail-empty", "no links yet" }
                }
            }
        }
    }
}

#[component]
fn MiniSvg(graph: Graph, space_decoded: String) -> Element {
    let positions = layout_force(&graph, SVG_W, SVG_H, SVG_ITERS);
    rsx! {
        for e in graph.edges.iter() {
            {
                let (sx, sy) = positions.get(&e.source).copied().unwrap_or((0.0, 0.0));
                let (tx, ty) = positions.get(&e.target).copied().unwrap_or((0.0, 0.0));
                rsx! {
                    line { class: "graph-link", x1: "{sx}", y1: "{sy}", x2: "{tx}", y2: "{ty}" }
                }
            }
        }
        for n in graph.nodes.iter() {
            {
                let (x, y) = positions.get(&n.ref_str).copied().unwrap_or((SVG_W / 2.0, SVG_H / 2.0));
                let cls = match n.kind.as_str() {
                    "heading" => "graph-node is-hd",
                    "attachment" => "graph-node is-att",
                    "block" => "graph-node is-blk",
                    _ => "graph-node is-doc",
                };
                let href = crate::router::route_for_space_note(&space_decoded, &n.ref_str);
                let label: String = n.title.chars().take(10).collect();
                rsx! {
                    a { href: "{href}",
                        g { transform: "translate({x},{y})",
                            circle { class: "{cls}", r: "5", cx: "0", cy: "0" }
                            text { class: "graph-label", y: "-8", "{label}" }
                        }
                    }
                }
            }
        }
    }
}
