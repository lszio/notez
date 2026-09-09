//! The workspace shell: sidebar (space switcher, filter, page tree,
//! scan/refresh/watch footer) and the two-column layout.

use dioxus::prelude::*;

use crate::data::space::{self, Entry, Space};
use crate::data::urls;

use ui::{NzShell, NzTree, TreeFile};

/// Tree files for the sidebar, with view hrefs.
pub fn tree_files(space: &Space, entries: &[Entry]) -> Vec<TreeFile> {
    entries
        .iter()
        .map(|e| TreeFile {
            path: e.locator.clone(),
            href: urls::view_url(&space.encoded, &e.locator),
            kind: e.kind.clone(),
            badge: if e.editable() {
                String::new()
            } else {
                e.ext.clone()
            },
        })
        .collect()
}

/// Full page shell. Every page renders through this so the sidebar is
/// identical everywhere.
#[component]
pub fn Shell(
    space: Space,
    /// Page title (browser tab).
    title: String,
    /// Locator of the page being viewed, if any.
    current: Option<String>,
    /// Where the Scan button should return to.
    return_to: String,
    topbar: Element,
    children: Element,
) -> Element {
    let entries = space::entries(&space.root).unwrap_or_default();
    let files = tree_files(&space, &entries);
    let nodes = ui::build_tree(&files);
    let doc_count = entries.iter().filter(|e| e.editable()).count();
    let file_count = entries.len() - doc_count;
    let spaces = space::list_spaces();
    let (fingerprint, _) = space::fingerprint(&space.root);
    let encoded = space.encoded.clone();
    let scan_action = format!("/scan/{encoded}");
    let space_name = space.name.clone();

    rsx! {
        document::Title { "{title} · notez" }
        NzShell {
            space: encoded.clone(),
            topbar: topbar,
            sidebar: rsx! {
                div { class: "brand",
                    a { href: "/", "notez" }
                    details { class: "space-switch",
                        summary { "{space_name}" }
                        for other in spaces.iter() {
                            a {
                                href: "{urls::space_url(&other.encoded)}",
                                class: if other.encoded == encoded { "current" } else { "" },
                                "{other.name}"
                                span { class: "path", "{other.root.display()}" }
                            }
                        }
                    }
                }
                div { class: "sb-top",
                    input {
                        class: "filter",
                        id: "filter",
                        r#type: "text",
                        placeholder: "filter pages…  ( / )",
                        autocomplete: "off",
                        spellcheck: "false",
                    }
                    span { class: "edit-hint", id: "count", "{entries.len()}" }
                }
                div { class: "tree-wrap", id: "tree",
                    NzTree { nodes: nodes, current: current.clone() }
                }
                div { class: "sb-foot",
                    span { class: "edit-hint", "{doc_count} docs · {file_count} files" }
                    form { method: "post", action: "{scan_action}",
                        input { r#type: "hidden", name: "next", value: "{return_to}" }
                        button { class: "btn ghost", r#type: "submit", title: "Re-index this space", "Scan" }
                    }
                    a { class: "btn ghost", href: "{return_to}", title: "Reload this page", "Refresh" }
                    a {
                        class: "watch-pill",
                        id: "watch-pill",
                        href: "{return_to}",
                        hidden: true,
                        "data-fingerprint": "{fingerprint}",
                        "changed on disk — reload"
                    }
                }
            },
            {children}
        }
    }
}

/// Topbar for a space-level page.
pub fn topbar_space(space: &Space, title: &str) -> Element {
    let new_url = urls::new_url(&space.encoded);
    rsx! {
        span { class: "title", "{title}" }
        span { class: "spacer" }
        a { class: "btn primary", href: "{new_url}", "+ New" }
    }
}

/// Topbar for a document or attachment page.
pub fn topbar_doc(
    space: &Space,
    locator: &str,
    title: &str,
    editing: bool,
    editable: bool,
) -> Element {
    let view = urls::view_url(&space.encoded, locator);
    let edit = urls::edit_url(&space.encoded, locator);
    let raw = urls::raw_url(&space.encoded, locator);
    rsx! {
        span { class: "title", "{title}" }
        span { class: "crumb", "{locator}" }
        span { class: "spacer" }
        if editing {
            a { class: "btn", href: "{view}", "Cancel" }
        } else if editable {
            a { class: "btn primary", href: "{edit}", "Edit" }
            a { class: "btn ghost", href: "{raw}", "Raw" }
        } else {
            a { class: "btn ghost", href: "{raw}", target: "_blank", rel: "noopener", "Open raw" }
        }
    }
}
