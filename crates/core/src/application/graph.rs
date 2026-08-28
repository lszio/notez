//! `Graph` — the read-model that backs the web client's graph views.
//!
//! v0.3 of the web client renders two graph views:
//! - A local 1-hop subgraph on the resource detail page.
//! - A full force-directed graph on the dedicated `/space/.../graph`
//!   page.
//!
//! Both views share the same `Graph` data structure and the same
//! `layout_force` algorithm. The web layer is responsible for
//! projecting the `Graph` into a static SVG; this build has no
//! client-side hydration, so all rendering happens during the SSR
//! pass.
//!
//! `Graph::from_facade` walks every resource in a space plus the
//! `resolved_relations` rows to build the full graph. For very
//! large sources, the caller can opt for a sampled or pruned
//! graph; the v0.3 implementation just truncates the node list at
//! `MAX_NODES` and reports `truncated: true` so the renderer can
//! surface the gap.

use std::collections::{BTreeMap, BTreeSet};
use crate::domain::{ProjectionReader, ProjectionWrite};

use crate::application::service::Engine;
use crate::application::use_cases::{LinkUseCase, ResourceUseCase, ScanUseCase};
use crate::domain::{ProjectionStore, ResolvedRelation, Resource, ResourceRef, Selector};
use serde::{Deserialize, Serialize};

/// A node in the graph. One per `Resource` in the space.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub ref_str: String,
    /// Kind tag (`document`, `heading`, `attachment`, `block`,
    /// `missing` for ghost nodes whose backing resource is gone).
    pub kind: String,
    /// Display title, truncated to 80 chars for layout readability.
    pub title: String,
    /// Total degree (in + out) at the time of build. The renderer
    /// uses this to size the node circle.
    pub degree: usize,
}

/// An undirected edge in the graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    /// `ok` / `unresolved` / `ambiguous` / `external` / `invalid`.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub total_nodes: usize,
    pub truncated: bool,
}

/// Hard cap on node count to keep the SSR HTML size sane.
pub const MAX_NODES: usize = 500;

/// Translate a `ResolutionStatus` into a short tag the SVG can
/// style on.
fn status_kind(s: &crate::domain::ResolutionStatus) -> &'static str {
    use crate::domain::ResolutionStatus::*;
    match s {
        Resolved => "ok",
        Unresolved => "unresolved",
        Ambiguous => "ambiguous",
        External => "external",
        Invalid => "invalid",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

impl Graph {
    /// Build the full graph for a space using the storage facade.
    pub fn from_facade<S>(
        facade: &Engine<S>,
    ) -> Result<Self, crate::application::ApplicationError>
    where
        S: ProjectionStore,
        S: ProjectionReader<Error = crate::storage::StorageError>
            + ProjectionWrite<Error = crate::storage::StorageError>,
    {
        let page = facade.query(&Selector::new())?;
        let resources: Vec<Resource> = page.items;

        // Pass 1: collect every node and build a ref-index.
        let mut nodes: Vec<GraphNode> = Vec::with_capacity(resources.len());
        let mut known_refs: BTreeSet<String> = BTreeSet::new();
        let truncated = resources.len() > MAX_NODES;
        for r in resources.iter().take(MAX_NODES) {
            let ref_str = r.r#ref.to_string();
            known_refs.insert(ref_str.clone());
            nodes.push(GraphNode {
                ref_str,
                kind: r.kind.to_string(),
                title: truncate(&r.title, 80),
                degree: 0,
            });
        }
        let total_nodes = resources.len();

        // Pass 2: walk every node's `resolved_relations`. Because
        // `query_resolved_relations` is per-source, we iterate
        // every source node and accumulate edges.
        let known_list: Vec<String> = known_refs.iter().cloned().collect();
        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        for ref_str in &known_list {
            let Ok(parsed) = ref_str.parse::<ResourceRef>() else {
                continue;
            };
            let relations = facade.query_resolved_relations(&parsed)?;
            for r in relations {
                let (s, t) = (r.source_ref.to_string(), r.target_ref.to_string());
                if !known_refs.contains(&s) || !known_refs.contains(&t) {
                    continue;
                }
                let key = if s <= t {
                    (s.clone(), t.clone())
                } else {
                    (t.clone(), s.clone())
                };
                if !seen.insert(key) {
                    continue;
                }
                edges.push(GraphEdge {
                    source: s,
                    target: t,
                    kind: status_kind(&r.status).to_string(),
                });
            }
        }

        // Pass 3: compute degree.
        let mut degree: BTreeMap<String, usize> = BTreeMap::new();
        for e in &edges {
            *degree.entry(e.source.clone()).or_default() += 1;
            *degree.entry(e.target.clone()).or_default() += 1;
        }
        for n in &mut nodes {
            n.degree = degree.get(&n.ref_str).copied().unwrap_or(0);
        }

        Ok(Graph {
            nodes,
            edges,
            total_nodes,
            truncated,
        })
    }

    /// Build a 1-hop subgraph around `focus`.
    pub fn neighborhood<S>(
        facade: &Engine<S>,
        focus_ref: &str,
    ) -> Result<Self, crate::application::ApplicationError>
    where
        S: ProjectionStore,
        S: ProjectionReader<Error = crate::storage::StorageError>
            + ProjectionWrite<Error = crate::storage::StorageError>,
    {
        let focus = focus_ref.to_string();
        let page = facade.query(&Selector::new())?;
        let focus_resource = page.items.iter().find(|r| r.r#ref.to_string() == focus);

        let mut nodes: BTreeMap<String, GraphNode> = BTreeMap::new();
        if let Some(r) = focus_resource {
            nodes.insert(
                focus.clone(),
                GraphNode {
                    ref_str: focus.clone(),
                    kind: r.kind.to_string(),
                    title: truncate(&r.title, 80),
                    degree: 0,
                },
            );
        } else {
            nodes.insert(
                focus.clone(),
                GraphNode {
                    ref_str: focus.clone(),
                    kind: "missing".to_string(),
                    title: truncate(focus_ref, 80),
                    degree: 0,
                },
            );
        }

        let Ok(focus_parsed) = focus.parse::<ResourceRef>() else {
            return Ok(Graph {
                nodes: nodes.into_values().collect(),
                edges: Vec::new(),
                total_nodes: 1,
                truncated: false,
            });
        };

        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();

        // Outgoing: focus is the source.
        let relations = facade.query_resolved_relations(&focus_parsed)?;
        for r in relations {
            let s = r.source_ref.to_string();
            let t = r.target_ref.to_string();
            let other = if s == focus { t.clone() } else { s.clone() };
            let (a, b) = if s <= t {
                (s.clone(), t.clone())
            } else {
                (t.clone(), s.clone())
            };
            edges.push(GraphEdge {
                source: s,
                target: t,
                kind: status_kind(&r.status).to_string(),
            });
            seen.insert((a, b));
            if !nodes.contains_key(&other) {
                if let Some(r2) = page.items.iter().find(|r| r.r#ref.to_string() == other) {
                    nodes.insert(
                        other.clone(),
                        GraphNode {
                            ref_str: other.clone(),
                            kind: r2.kind.to_string(),
                            title: truncate(&r2.title, 80),
                            degree: 0,
                        },
                    );
                } else {
                    nodes.insert(
                        other.clone(),
                        GraphNode {
                            ref_str: other.clone(),
                            kind: "missing".to_string(),
                            title: truncate(&other, 80),
                            degree: 0,
                        },
                    );
                }
            }
        }

        // Incoming: walk every other resource looking for edges
        // pointing at focus.
        for r in &page.items {
            if r.r#ref.to_string() == focus {
                continue;
            }
            let src = r.r#ref.to_string();
            let Ok(src_parsed) = src.parse::<ResourceRef>() else {
                continue;
            };
            let relations = facade.query_resolved_relations(&src_parsed)?;
            for rel in relations {
                if rel.target_ref.to_string() != focus {
                    continue;
                }
                let s = rel.source_ref.to_string();
                let t = rel.target_ref.to_string();
                let key = if s <= t {
                    (s.clone(), t.clone())
                } else {
                    (t.clone(), s.clone())
                };
                if !seen.insert(key) {
                    continue;
                }
                edges.push(GraphEdge {
                    source: s,
                    target: t,
                    kind: status_kind(&rel.status).to_string(),
                });
                if !nodes.contains_key(&src) {
                    nodes.insert(
                        src.clone(),
                        GraphNode {
                            ref_str: src.clone(),
                            kind: r.kind.to_string(),
                            title: truncate(&r.title, 80),
                            degree: 0,
                        },
                    );
                }
            }
        }

        // Compute degrees.
        let mut degree: BTreeMap<String, usize> = BTreeMap::new();
        for e in &edges {
            *degree.entry(e.source.clone()).or_default() += 1;
            *degree.entry(e.target.clone()).or_default() += 1;
        }
        let mut nodes: Vec<GraphNode> = nodes.into_values().collect();
        for n in &mut nodes {
            n.degree = degree.get(&n.ref_str).copied().unwrap_or(0);
        }
        let total = 1 + edges.len();

        Ok(Graph {
            nodes,
            edges,
            total_nodes: total,
            truncated: false,
        })
    }
}

/// Force-directed layout. A small Fruchterman–Reingold variant.
pub fn layout_force(
    graph: &Graph,
    width: f64,
    height: f64,
    iterations: usize,
) -> BTreeMap<String, (f64, f64)> {
    let n = graph.nodes.len();
    if n == 0 {
        return BTreeMap::new();
    }
    if n == 1 {
        let mut out = BTreeMap::new();
        out.insert(graph.nodes[0].ref_str.clone(), (width / 2.0, height / 2.0));
        return out;
    }

    let area = width * height;
    let k = (area / n as f64).sqrt() * 0.7;

    // Initial positions on a circle.
    let mut pos: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
        let r = (width.min(height)) * 0.4;
        let x = width / 2.0 + r * angle.cos();
        let y = height / 2.0 + r * angle.sin();
        pos.insert(node.ref_str.clone(), (x, y));
    }

    let t_initial = (width.min(height)) * 0.1;
    let t_final = k * 0.05;
    let mut temperature = t_initial;
    let cool = (t_initial - t_final) / iterations.max(1) as f64;

    for _ in 0..iterations {
        // Repulsive: every pair.
        let mut disp: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        for (i, ni) in graph.nodes.iter().enumerate() {
            let mut dx = 0.0;
            let mut dy = 0.0;
            let (xi, yi) = pos[&ni.ref_str];
            for (j, nj) in graph.nodes.iter().enumerate() {
                if i == j {
                    continue;
                }
                let (xj, yj) = pos[&nj.ref_str];
                let mut ddx = xi - xj;
                let mut ddy = yi - yj;
                let mut dist = (ddx * ddx + ddy * ddy).sqrt();
                if dist < 0.01 {
                    ddx = (i as f64) - (j as f64);
                    ddy = (i as f64) * 0.5;
                    dist = (ddx * ddx + ddy * ddy).sqrt();
                }
                let force = k * k / dist;
                dx += (ddx / dist) * force;
                dy += (ddy / dist) * force;
            }
            dx += (width / 2.0 - xi) * 0.05;
            dy += (height / 2.0 - yi) * 0.05;
            disp.insert(ni.ref_str.clone(), (dx, dy));
        }
        // Attractive: along edges.
        for e in &graph.edges {
            let (xs, ys) = pos[&e.source];
            let (xt, yt) = pos[&e.target];
            let ddx = xs - xt;
            let ddy = ys - yt;
            let dist = (ddx * ddx + ddy * ddy).sqrt().max(0.01);
            let force = (dist * dist) / k;
            let entry_s = disp.get_mut(&e.source).unwrap();
            entry_s.0 -= (ddx / dist) * force;
            entry_s.1 -= (ddy / dist) * force;
            let entry_t = disp.get_mut(&e.target).unwrap();
            entry_t.0 += (ddx / dist) * force;
            entry_t.1 += (ddy / dist) * force;
        }
        // Apply displacement.
        for node in &graph.nodes {
            let (dx, dy) = disp[&node.ref_str];
            let disp_mag = (dx * dx + dy * dy).sqrt().max(0.01);
            let clamp = disp_mag.min(temperature);
            let (px, py) = pos.get_mut(&node.ref_str).unwrap();
            *px += (dx / disp_mag) * clamp;
            *py += (dy / disp_mag) * clamp;
            *px = px.clamp(40.0, width - 40.0);
            *py = py.clamp(40.0, height - 40.0);
        }
        temperature = (temperature - cool).max(t_final);
    }

    pos
}

// ---- tests ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_handles_ascii() {
        assert_eq!(truncate("hello world", 5), "hello…");
        assert_eq!(truncate("hi", 5), "hi");
    }

    #[test]
    fn truncate_handles_multibyte() {
        let s = "汉字字符串";
        let out = truncate(s, 2);
        assert_eq!(out.chars().count(), 3);
    }

    #[test]
    fn layout_force_empty_and_singleton() {
        let g = Graph {
            nodes: vec![],
            edges: vec![],
            total_nodes: 0,
            truncated: false,
        };
        assert!(layout_force(&g, 100.0, 100.0, 10).is_empty());

        let g = Graph {
            nodes: vec![GraphNode {
                ref_str: "a".into(),
                kind: "doc".into(),
                title: "a".into(),
                degree: 0,
            }],
            edges: vec![],
            total_nodes: 1,
            truncated: false,
        };
        let pos = layout_force(&g, 100.0, 100.0, 10);
        assert_eq!(pos.get("a"), Some(&(50.0, 50.0)));
    }

    #[test]
    fn layout_force_separates_nodes() {
        let g = Graph {
            nodes: vec![
                GraphNode {
                    ref_str: "a".into(),
                    kind: "doc".into(),
                    title: "a".into(),
                    degree: 0,
                },
                GraphNode {
                    ref_str: "b".into(),
                    kind: "doc".into(),
                    title: "b".into(),
                    degree: 0,
                },
            ],
            edges: vec![],
            total_nodes: 2,
            truncated: false,
        };
        let pos = layout_force(&g, 200.0, 200.0, 200);
        let (ax, ay) = pos["a"];
        let (bx, by) = pos["b"];
        let d = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
        assert!(
            d > 5.0,
            "two disconnected nodes should not collapse onto each other; got distance {d}"
        );
    }
}
