//! GraphPage — the full-space force-directed graph view.

use dioxus::prelude::*;

use crate::pages::ui::{Breadcrumb, BreadcrumbSegment};
use crate::pages::use_space_layout;
use crate::router::{route_for_space_list, route_for_space_resource};
use crate::server::list_graph;
use notez_core::application::{layout_force, Graph, GraphEdge, GraphNode};
use crate::space_ctx::{SpaceState, SpaceStatus};

const SVG_WIDTH: f64 = 900.0;
const SVG_HEIGHT: f64 = 600.0;
const SVG_ITERATIONS: usize = 220;

fn empty_graph() -> Graph {
    Graph {
        nodes: vec![],
        edges: vec![],
        total_nodes: 0,
        truncated: false,
    }
}

#[component]
pub fn GraphPage(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();
    let mut resource_ref_ctx = use_context::<Signal<Option<String>>>();
    resource_ref_ctx.set(None);

    let active_path = space().map(|s| s.path.clone());
    let active_encoded_for_render = space().map(|s| s.encoded.clone());

    let active_path_for_fetch = active_path.clone();
    let active_path_for_legend = active_path.clone();
    let active_path_for_crumbs = active_path.clone();

    let graph_resource = use_server_future(move || {
        let p = active_path_for_fetch.clone();
        async move {
            match p {
                Some(p) => list_graph(p).await.unwrap_or_else(|_| empty_graph()),
                None => empty_graph(),
            }
        }
    })?;

    let graph: Graph = graph_resource.cloned().unwrap_or_else(empty_graph);

    let (eyebrow, h1, lede) = match space() {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => (
            "graph".to_string(),
            format!("{} · graph", s.name),
            format!(
                "{} resources · {} links{}",
                graph.total_nodes,
                graph.edges.len(),
                if graph.truncated { " · truncated to the busiest 500 nodes" } else { "" }
            ),
        ),
        Some(SpaceState { status: SpaceStatus::Resolving, .. }) => (
            "graph".to_string(),
            "graph · resolving…".to_string(),
            "validating the space root…".to_string(),
        ),
        Some(SpaceState { status: SpaceStatus::Error(e), .. }) => (
            "graph".to_string(),
            "graph".to_string(),
            format!("space error: {e}"),
        ),
        None => (
            "graph".to_string(),
            "graph".to_string(),
            "pick a space from the sidebar to begin.".to_string(),
        ),
    };

    let space_path_for_crumbs = active_path_for_crumbs.clone().unwrap_or_default();
    let space_path_for_legend = active_path_for_legend.clone();
    let leaf_for_crumb = space_path_for_crumbs
        .rsplit('/')
        .next()
        .unwrap_or("space")
        .to_string();

    rsx! {
        div { class: "page",
            Breadcrumb { segments: vec![
                BreadcrumbSegment::link("notez", "/"),
                BreadcrumbSegment::link(
                    leaf_for_crumb.clone(),
                    space_path_for_legend
                        .as_ref()
                        .map(|p| route_for_space_list(p))
                        .unwrap_or_else(|| "/".to_string()),
                ),
                BreadcrumbSegment::here("graph".to_string()),
            ] }
            div { class: "page-h",
                p { class: "eyebrow", "{eyebrow}" }
                h1 { "{h1}" }
                p { class: "lede", "{lede}" }
            }

            div { class: "graph-page",
                aside { class: "graph-controls",
                    h2 { "graph" }
                    dl { class: "gc-stats",
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
                    h2 { "legend" }
                    div { class: "gc-legend",
                        div { span { class: "swatch is-doc" } "document" }
                        div { span { class: "swatch is-hd" }  "heading" }
                        div { span { class: "swatch is-att" } "attachment" }
                        div { span { class: "swatch is-blk" } "block" }
                    }
                    if let Some(p) = active_path.as_ref() {
                        p { class: "mono-sm",
                            "path: "
                            span { "{p}" }
                        }
                    }
                }

                if graph.nodes.is_empty() {
                    div { class: "graph-empty",
                        "this space has no resources yet. run "
                        code { "notez scan" }
                        " first."
                    }
                } else {
                    GraphSvg { graph: graph.clone(), space_encoded: active_encoded_for_render.unwrap_or_default() }
                }
            }
        }
    }
}

#[component]
fn GraphSvg(graph: Graph, space_encoded: String) -> Element {
    let positions = layout_force(&graph, SVG_WIDTH, SVG_HEIGHT, SVG_ITERATIONS);
    let space_decoded = crate::router::decode_space(&space_encoded);
    rsx! {
        svg {
            class: "graph-svg",
            view_box: "0 0 {SVG_WIDTH} {SVG_HEIGHT}",
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
) -> Element {
    let (x, y) = positions.get(&node.ref_str).copied().unwrap_or((SVG_WIDTH / 2.0, SVG_HEIGHT / 2.0));
    let href = route_for_space_resource(&space_decoded, &node.ref_str);
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