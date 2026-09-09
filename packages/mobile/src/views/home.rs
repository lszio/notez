//! Mobile workspace view — the same shell/tree components as web and
//! desktop, driven by the remote `HttpBackend`.

use dioxus::prelude::*;
use ui::{
    build_tree, Backend, NzBadge, NzCard, NzShell, NzTree, ResourceRow, TreeEntry, TreeFile,
};

use crate::backend::HttpBackend;

#[derive(Clone, PartialEq)]
struct SpaceReport {
    root: String,
    display_name: String,
    scan_count: Option<u32>,
    scan_error: Option<String>,
    rows: Vec<ResourceRow>,
}

fn tree_of(rows: &[ResourceRow]) -> Vec<TreeEntry> {
    let files: Vec<TreeFile> = rows
        .iter()
        .filter(|r| !r.locator.is_empty())
        .map(|r| TreeFile {
            path: r.locator.clone(),
            href: String::new(),
            kind: r.kind.clone(),
            badge: r.kind.clone(),
        })
        .collect();
    build_tree(&files)
}

#[component]
pub fn Workspace() -> Element {
    let backend = use_context::<HttpBackend>();
    let mut reload = use_signal(|| 0u32);

    let backend_for_load = backend.clone();
    let reports = use_resource(move || {
        let backend = backend_for_load.clone();
        let _ = reload();
        async move {
            let spaces = backend.list_spaces().await.unwrap_or_default();
            let mut out = Vec::with_capacity(spaces.len());
            for s in spaces {
                let root = s.root.to_string_lossy().to_string();
                let scan_count = backend.scan_space(&root).await.ok();
                let scan_error = if scan_count.is_none() {
                    backend.scan_space(&root).await.err()
                } else {
                    None
                };
                let rows = backend
                    .query_resources(&root, None, None, Some(500))
                    .await
                    .unwrap_or_default();
                out.push(SpaceReport {
                    root,
                    display_name: s.display_name.clone(),
                    scan_count,
                    scan_error,
                    rows,
                });
            }
            out
        }
    });

    let reports_value = reports.cloned().unwrap_or_default();
    let active = reports_value.first().cloned();
    let space_name = active
        .as_ref()
        .map(|r| r.display_name.clone())
        .unwrap_or_else(|| "notez".to_string());
    let tree = active.as_ref().map(|r| tree_of(&r.rows)).unwrap_or_default();

    rsx! {
        NzShell {
            space: space_name.clone(),
            sidebar: rsx! {
                div { class: "brand",
                    span { class: "tree-name", "notez" }
                    span { class: "space-name", "{space_name}" }
                }
                div { class: "tree-wrap", id: "tree",
                    NzTree { nodes: tree, current: None }
                }
                div { class: "sb-foot",
                    span { class: "edit-hint", "mobile · http" }
                    button { class: "btn ghost", onclick: move |_| reload += 1, "Scan" }
                }
            },
            topbar: rsx! {
                span { class: "title", "{space_name}" }
                span { class: "spacer" }
                NzBadge { text: "http".to_string(), tone: "info".to_string() }
            },
            main { class: "workspace",
                if reports_value.is_empty() {
                    NzCard { padded: true,
                        h2 { "No spaces" }
                        p { class: "edit-hint", "Set NOTEZ_DEFAULT_SOURCE on the server, or specify a source per call." }
                    }
                }
                for report in reports_value.iter() {
                    NzCard { padded: true,
                        h2 { "{report.display_name}" }
                        p { class: "edit-hint", "{report.root}" }
                        match (report.scan_count, report.scan_error.clone()) {
                            (Some(n), _) => rsx! { p { class: "edit-hint", "{n} indexed resources" } },
                            (None, Some(err)) => rsx! { p { class: "banner err", "Scan error: {err}" } },
                            (None, None) => rsx! { p { class: "edit-hint", "Scanning…" } },
                        }
                        ul { class: "doc",
                            for row in report.rows.iter().take(200) {
                                li {
                                    "{row.title}"
                                    span { class: "tree-badge", "{row.kind}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
