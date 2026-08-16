//! GraphPage — the full-space force-directed graph view.
//!
//! v0.3 of the web client adds a graph navigation mode: every
//! space gets a `/space/.../graph` route that renders the entire
//! resource graph as a static SVG. The graph is built server-side
//! by `Graph::from_facade` and laid out with `layout_force`; the
//! SVG goes straight into the SSR HTML so the page is readable
//! without hydration.
//!
//! The graph is a read-only view: nodes are clickable (they
//! navigate to the corresponding resource detail page), but
//! edges are not (no JS for hover highlight). That trade-off keeps
//! the page dead-simple to ship.

use dioxus::prelude::*;

use crate::pages::{use_space_layout, PageHeader};
use crate::pages::ui::Breadcrumb;
use crate::router::{encode_space, route_for_space_list};
use crate::server::list_graph;
use notez_core::application::{layout_force, Graph, GraphEdge, GraphNode};
use crate::space_ctx::{SpaceState, SpaceStatus};

const SVG_WIDTH: f64 = 900.0;
const SVG_HEIGHT: f64 = 600.0;
const SVG_ITERATIONS: usize = 220;

#[component]
pub fn GraphPage(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space().map(|s| s.path.clone());
    let active_path_for_fetch = active_path.clone();
    let active_path_for_legend = active_path.clone();
    let active_encoded = space().map(|s| s.encoded.clone());
    let active_encoded_for_render = active_encoded.clone();

    // Server-side fetch: `use_server_future` blocks SSR until the
    // graph is ready, so the SVG lands fully laid out in the
    // first paint. No JS is required to view the result.
    let graph_resource = use_server_future(move || {
        let p = active_path_for_fetch.clone();
        async move {
            match p {
                Some(p) => list_graph(p).await.unwrap_or_else(|_| Graph {
                    nodes: vec![],
                    edges: vec![],
                    total_nodes: 0,
                    truncated: false,
                }),
                None => Graph {
                    nodes: vec![],
                    edges: vec![],
                    total_nodes: 0,
                    truncated: false,
                },
            }
        }
    })?;

    let graph: Graph = graph_resource.cloned().unwrap_or(Graph {
        nodes: vec![],
        edges: vec![],
        total_nodes: 0,
        truncated: false,
    });

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

    rsx! {
        PageHeader {}
        div { class: "page",
            Breadcrumb { segments: vec![
                crate::pages::ui::BreadcrumbSegment::link("notez", "/"),
                crate::pages::ui::BreadcrumbSegment::link(
                    space().as_ref().map(|s| s.path.clone()).unwrap_or_default()
                        .rsplit('/').next().unwrap_or("space")
                        .to_string(),
                    active_path_for_legend
                        .as_ref()
                        .map(|p| route_for_space_list(p))
                        .unwrap_or_else(|| "/".to_string()),
                ),
                crate::pages::ui::BreadcrumbSegment::here("graph".to_string()),
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
    // Compute layout positions once during the SSR pass. The
    // computation is fast (≤ 220 iterations of a simple O(N²)
    // force-directed pass) and bounded by `MAX_NODES`.
    let positions = layout_force(&graph, SVG_WIDTH, SVG_HEIGHT, SVG_ITERATIONS);

    rsx! {
        svg {
            class: "graph-svg",
            view_box: "0 0 {SVG_WIDTH} {SVG_HEIGHT}",
            width: "{SVG_WIDTH}",
            height: "{SVG_HEIGHT}",
            role: "img",
            "aria-label": "graph",
            // Edges first (so circles cover endpoints).
            for e in graph.edges.iter() {
                GraphEdgeSvg { edge: e.clone(), positions: positions.clone() }
            }
            for n in graph.nodes.iter() {
                GraphNodeSvg {
                    node: n.clone(),
                    positions: positions.clone(),
                    space_encoded: space_encoded.clone(),
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
    // Edges that didn't resolve cleanly get the accent colour so
    // the reader can spot them at a glance.
    let active = if edge.kind == "ok" { "" } else { "is-active" };
    rsx! {
        line {
            class: "graph-link {active}",
            x1: "{sx}",
            y1: "{sy}",
            x2: "{tx}",
            y2: "{ty}",
        }
    }
}

#[component]
fn GraphNodeSvg(
    node: GraphNode,
    positions: std::collections::BTreeMap<String, (f64, f64)>,
    space_encoded: String,
) -> Element {
    let (x, y) = positions
        .get(&node.ref_str)
        .copied()
        .unwrap_or((SVG_WIDTH / 2.0, SVG_HEIGHT / 2.0));
    // Radius scales with log2(degree+1), capped, so a hub with
    // dozens of links doesn't dwarf its neighbours.
    let r: f64 = 5.0 + 4.0 * ((node.degree as f64 + 1.0).log2()).min(3.5);
    let kind_cls = match node.kind.as_str() {
        "document" => "is-doc",
        "heading" => "is-hd",
        "attachment" => "is-att",
        _ => "is-blk",
    };
    // Truncate label so the text doesn't overflow the circle.
    let label: String = node.title.chars().take(14).collect();
    // Clicking a node jumps to the detail page. The detail page
    // route needs the encoded space root and the encoded ref.
    let detail_href = format!(
        "/space/{}/resource/{}",
        space_encoded,
        encode_space(&node.ref_str)
    );
    rsx! {
        a { href: "{detail_href}",
            g { transform: "translate({x},{y})",
                circle {
                    class: "graph-node {kind_cls}",
                    r: "{r}",
                    cx: "0",
                    cy: "0",
                }
                text {
                    class: "graph-label",
                    y: "{r + 11.0}",
                    "{label}"
                }
            }
        }
    }
}
