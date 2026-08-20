//! `TreePanel` — the top half of the left column.

use dioxus::prelude::*;

use crate::router::{route_for_space_preview, route_for_space_resource};
use crate::server::list_space_tree;
use crate::space_ctx::{SpaceState, SpaceStatus};
use crate::tree::{TreeChild, TreeNode};

fn empty_tree() -> TreeNode {
    TreeNode {
        name: String::new(),
        path: String::new(),
        depth: 0,
        count: 0,
        children: Vec::new(),
    }
}

#[component]
pub fn TreePanel(active_encoded: Option<String>) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());

    let tree_resource = use_server_future(move || {
        let p = active_path.clone();
        async move {
            match p {
                Some(p) => list_space_tree(p).await.ok(),
                None => None,
            }
        }
    })?;

    let tree: Option<TreeNode> = tree_resource.cloned().flatten();

    let encoded = active_encoded.clone().unwrap_or_default();
    let encoded_for_href = encoded.clone();

    rsx! {
        div { class: "tree-panel",
            div { class: "tree-head",
                span { class: "tree-label", "directory tree" }
                if let Some(ref t) = tree {
                    span { class: "tree-count", "({t.count})" }
                }
            }
            div { class: "tree-body",
                match &tree {
                    Some(t) => rsx! { TreeNodeView { node: t.clone(), encoded: encoded_for_href.clone() } },
                    None => rsx! {
                        match space().map(|s| s.status.clone()) {
                            Some(SpaceStatus::Resolving) => rsx! { p { class: "tree-empty", "loading…" } },
                            Some(SpaceStatus::Error(e)) => rsx! { p { class: "tree-empty err-text", "error: {e}" } },
                            _ => rsx! { p { class: "tree-empty", "no resources" } },
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn TreeNodeView(node: TreeNode, encoded: String) -> Element {
    let name = if node.path.is_empty() { "(root)".to_string() } else { node.name.clone() };
    let encoded_for_children = encoded.clone();
    let decoded_space = crate::router::decode_space(&encoded);
    rsx! {
        details { class: "tree-node", open: true,
            summary { class: "tree-summary",
                span { class: "tree-marker", "▾" }
                span { class: "tree-name", "{name}" }
                span { class: "tree-count-pill", "({node.count})" }
            }
            ul { class: "tree-children",
                for c in node.children.iter() {
                    {
                        let encoded_for_child = encoded_for_children.clone();
                        let decoded_for_child = decoded_space.clone();
                        match c.clone() {
                            TreeChild::Folder(f) => rsx! {
                                TreeNodeView { node: f, encoded: encoded_for_child }
                            },
                            TreeChild::Leaf { name, path, kind, title, ref_str } => {
                                // Indexed files → resource reader;
                                // loose files (ref_str empty) → preview page.
                                let href = if ref_str.is_empty() {
                                    route_for_space_preview(&decoded_for_child, &path)
                                } else {
                                    route_for_space_resource(&decoded_for_child, &ref_str)
                                };
                                rsx! {
                                    li { class: "tree-leaf",
                                        a { class: "tree-leaf-link kind-{kind}", href: "{href}", title: "{path}",
                                            span { class: "tree-leaf-name", "{name}" }
                                            span { class: "tree-leaf-kind", "{title}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}