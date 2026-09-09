//! Shared workspace components — the cross-platform rendering core.
//!
//! These are *presentational*: they take data and hrefs as props and
//! know nothing about how a surface loads or writes them. The web
//! host renders them during SSR, the desktop and mobile shells render
//! them natively, so the workspace looks and behaves the same
//! everywhere.

use dioxus::prelude::*;

/// One file to place in the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeFile {
    /// Space-relative POSIX path (`projects/index.org`).
    pub path: String,
    /// Where clicking it goes (platform-specific URL).
    pub href: String,
    /// `document` | `attachment`.
    pub kind: String,
    /// Optional badge shown after the name (file extension).
    pub badge: String,
}

/// A node in the sidebar tree: either a folder (with children) or a
/// file (empty `children`, `href` set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Display name (`index.org`, `projects`).
    pub name: String,
    /// Space-relative path of the node.
    pub path: String,
    /// Link target for files; empty for folders.
    pub href: String,
    pub kind: String,
    pub badge: String,
    /// Number of files below this node (folders only).
    pub count: usize,
    pub children: Vec<TreeEntry>,
}

impl TreeEntry {
    pub fn is_dir(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Build a folder tree from a flat file list.
///
/// Folders sort before files, both alphabetically (case-insensitive);
/// folder `count` is the number of descendant files.
pub fn build_tree(files: &[TreeFile]) -> Vec<TreeEntry> {
    let mut root: Vec<TreeEntry> = Vec::new();
    for file in files {
        let segments: Vec<&str> = file.path.split('/').collect();
        let mut level = &mut root;
        let mut prefix = String::new();
        for (i, seg) in segments.iter().enumerate() {
            let last = i + 1 == segments.len();
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(seg);
            let pos = level.iter().position(|n| n.name == *seg);
            let idx = match pos {
                Some(p) => p,
                None => {
                    level.push(TreeEntry {
                        name: seg.to_string(),
                        path: prefix.clone(),
                        href: if last { file.href.clone() } else { String::new() },
                        kind: if last { file.kind.clone() } else { String::new() },
                        badge: if last { file.badge.clone() } else { String::new() },
                        count: 0,
                        children: Vec::new(),
                    });
                    level.len() - 1
                }
            };
            if last {
                break;
            }
            level = &mut level[idx].children;
        }
    }
    sort_and_count(&mut root);
    root
}

fn sort_and_count(nodes: &mut Vec<TreeEntry>) -> usize {
    for node in nodes.iter_mut() {
        if node.children.is_empty() {
            node.count = 1;
            continue;
        }
        node.count = sort_and_count(&mut node.children);
    }
    nodes.sort_by(|a, b| {
        let a_dir = a.is_dir();
        let b_dir = b.is_dir();
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    nodes.iter().map(|n| n.count).sum()
}

/// The sidebar page tree. Folders are native `<details>` elements, so
/// expansion works without JavaScript.
#[component]
pub fn NzTree(
    nodes: Vec<TreeEntry>,
    /// Locator of the page currently open (highlighted).
    current: Option<String>,
) -> Element {
    rsx! {
        ul { class: "tree",
            for node in nodes {
                li {
                    if node.is_dir() {
                        details {
                            class: "tree-dir",
                            "data-dir": "{node.path}",
                            "data-k": "{node.path.to_lowercase()}",
                            open: node.count <= 40,
                            summary {
                                span { class: "tree-name", "{node.name}" }
                                span { class: "tree-count", "{node.count}" }
                            }
                            NzTree { nodes: node.children.clone(), current: current.clone() }
                        }
                    } else if node.href.is_empty() {
                        // A surface without a reader yet still gets the tree.
                        span {
                            class: if Some(node.path.clone()) == current { "tree-file current" } else { "tree-file" },
                            "data-k": "{node.path.to_lowercase()}",
                            title: "{node.path}",
                            span { class: "tree-name", "{node.name}" }
                            if !node.badge.is_empty() {
                                span { class: "tree-badge", "{node.badge}" }
                            }
                        }
                    } else {
                        a {
                            class: if Some(node.path.clone()) == current { "tree-file current" } else { "tree-file" },
                            href: "{node.href}",
                            "data-k": "{node.path.to_lowercase()}",
                            title: "{node.path}",
                            span { class: "tree-name", "{node.name}" }
                            if !node.badge.is_empty() {
                                span { class: "tree-badge", "{node.badge}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Rendered document body (HTML produced by the preview catalog).
#[component]
pub fn NzDocBody(html: String) -> Element {
    rsx! {
        article { class: "doc", dangerous_inner_html: "{html}" }
    }
}

/// Two-column workspace layout: sidebar + (topbar, content).
#[component]
pub fn NzShell(
    /// Space locator/name, exposed as `data-space` for host scripts.
    space: String,
    /// Sidebar contents (brand, filter, tree, footer).
    sidebar: Element,
    /// Topbar contents (title, actions).
    topbar: Element,
    children: Element,
) -> Element {
    rsx! {
        div { class: "shell", "data-space": "{space}",
            aside { class: "sidebar", id: "sidebar", {sidebar} }
            div { class: "main",
                header { class: "topbar", {topbar} }
                main { class: "content", {children} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> TreeFile {
        TreeFile {
            path: path.to_string(),
            href: format!("/view/{path}"),
            kind: "document".to_string(),
            badge: String::new(),
        }
    }

    #[test]
    fn builds_nested_folders_with_counts() {
        let nodes = build_tree(&[
            file("a/b/c.md"),
            file("a/b/d.md"),
            file("a/e.md"),
            file("top.md"),
        ]);
        // Folders sort before files at every level.
        assert_eq!(nodes[0].name, "a");
        assert_eq!(nodes[1].name, "top.md");
        assert_eq!(nodes[0].count, 3);
        let b = &nodes[0].children[0];
        assert_eq!(b.name, "b");
        assert_eq!(b.count, 2);
        assert_eq!(b.children[0].name, "c.md");
        assert_eq!(b.children[0].href, "/view/a/b/c.md");
        assert!(!b.children[0].is_dir());
    }

    #[test]
    fn same_named_folders_are_merged() {
        let nodes = build_tree(&[file("a/x.md"), file("a/y.md")]);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].children.len(), 2);
        assert_eq!(nodes[0].count, 2);
    }
}
