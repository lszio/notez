//! `TreePanel` — the top half of the left column.

use dioxus::prelude::*;

use crate::router::{route_for_space_preview, route_for_space_note};
use crate::server::list_space_tree;
use crate::space_ctx::SpaceState;
use crate::tree::{TreeChild, TreeNode};

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
                div { class: "tree-head-main", span { class: "tree-label", "DOCUMENTS" }, span { class: "tree-subtitle", "Browse by heading" } }
                span { class: "tree-count", if let Some(t) = tree.as_ref() { "{t.count}" } else { "…" } }
            }
            if let Some(t) = tree { TreeNodeView { node: t, encoded: encoded_for_href.clone() } }
            else { p { class: "tree-empty", "Loading document tree…" } }
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
                                    route_for_space_note(&decoded_for_child, &ref_str)
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