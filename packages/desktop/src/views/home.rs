//! Desktop home view — embedded workspace list + scan + query.

use dioxus::prelude::*;
use ui::{Backend, NzBadge, NzCard, ResourceRow};

use crate::backend::EmbeddedBackend;

#[derive(Clone, PartialEq)]
struct SpaceReport {
    root: String,
    display_name: String,
    scan_count: Option<u32>,
    scan_error: Option<String>,
    rows: Vec<ResourceRow>,
}

#[component]
pub fn Home() -> Element {
    let backend = use_context::<EmbeddedBackend>();

    let backend_for_load = backend.clone();
    let initial_space = use_context::<String>();

    // One-shot load: spaces -> scan each -> query resources for each.
    let reports = use_resource(move || {
        let backend = backend_for_load.clone();
        async move {
            let spaces = backend.list_spaces().await.unwrap_or_default();
            let mut out = Vec::with_capacity(spaces.len());
            for s in spaces {
                let root = s.root.to_string_lossy().to_string();
                let scan_count = backend.scan_space(&root).await.ok();
                let scan_error = if scan_count.is_none() {
                    backend
                        .scan_space(&root)
                        .await
                        .err()
                        .or(Some("scan failed".into()))
                } else {
                    None
                };
                let rows = backend
                    .query_resources(&root, None, None, Some(50))
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

    rsx! {
        main { class: "workspace",
            NzCard { padded: true,
                h1 { "Notez desktop workspace" }
                p { class: "muted", "Local embedded engine — opens a space, scans it, lists resources." }
                NzBadge { text: "embedded".to_string(), tone: "info".to_string() }
                p { class: "dim mono-sm", "Initial space: {initial_space}" }
            }

            match reports.cloned() {
                Some(reps) if !reps.is_empty() => rsx! {
                    NzCard { padded: true,
                        h2 { "Spaces" }
                        for r in reps.iter() {
                            SpaceBlock { report: r.clone() }
                        }
                    }
                },
                Some(_) => rsx! {
                    NzCard { padded: true,
                        h2 { "No spaces" }
                        p { class: "muted", "Set NOTEZ_DEFAULT_SPACE or run inside a space root." }
                    }
                },
                None => rsx! {
                    NzCard { padded: true,
                        h2 { "Loading spaces…" }
                    }
                },
            }
        }
    }
}

#[component]
fn SpaceBlock(report: SpaceReport) -> Element {
    rsx! {
        section { class: "space-entry",
            header { class: "space-head",
                strong { "{report.display_name}" }
                span { class: "mono-sm dim", "{report.root}" }
            }
            match (report.scan_count, report.scan_error) {
                (Some(n), _) => rsx! { p { class: "muted", "Scanned resources: {n}" } },
                (None, Some(err)) => rsx! { p { class: "err-text", "Scan error: {err}" } },
                (None, None) => rsx! { p { class: "muted", "Scanning…" } },
            }
            ul {
                for r in report.rows.iter() {
                    li {
                        NzCard { padded: true,
                            strong { "{r.title}" }
                            span { class: "mono-sm dim", "{r.locator}" }
                            NzBadge { text: r.kind.clone(), tone: "muted".to_string() }
                        }
                    }
                }
            }
        }
    }
}
