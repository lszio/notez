//! `FilesPanel` — the bottom half of the left column.
//!
//! Lists every file on disk under the active space (not just the
//! resources the projection has indexed). Clicking a row navigates
//! to the in-app preview page (`/space/:enc/preview/:locator`) so
//! PDFs, images, and other loose attachments get a proper
//! inline preview the moment they drop in, even before the
//! projection has scanned the space.

use dioxus::prelude::*;

use crate::router::{route_for_space_preview, route_for_space_resource};
use crate::server::list_filesystem;
use crate::space_ctx::{SpaceState, SpaceStatus};
use crate::tree::SourceFileRow;

#[component]
pub fn FilesPanel(active_encoded: Option<String>) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());

    let files_resource = use_server_future(move || {
        let p = active_path.clone();
        async move {
            match p {
                Some(p) => list_filesystem(p).await.unwrap_or_default(),
                None => Vec::new(),
            }
        }
    })?;

    let files: Vec<SourceFileRow> = files_resource.cloned().unwrap_or_default();

    let encoded = active_encoded.clone().unwrap_or_default();
    let decoded_space = crate::router::decode_space(&encoded);

    rsx! {
        div { class: "files-panel",
            div { class: "files-head",
                span { class: "files-label", "files" }
                span { class: "files-count", "({files.len()})" }
            }
            div { class: "files-body",
                if files.is_empty() {
                    {
                        match space().map(|s| s.status.clone()) {
                            Some(SpaceStatus::Resolving) => rsx! { p { class: "files-empty", "loading…" } },
                            Some(SpaceStatus::Error(e)) => rsx! { p { class: "files-empty err-text", "error: {e}" } },
                            _ => rsx! { p { class: "files-empty", "no files in this space yet." } },
                        }
                    }
                } else {
                    ul { class: "files-list",
                        for f in files.iter() {
                            {
                                let is_indexed = !f.ref_str.is_empty();
                                let href = if is_indexed {
                                    route_for_space_resource(&decoded_space, &f.ref_str)
                                } else {
                                    route_for_space_preview(&decoded_space, &f.display_path)
                                };
                                let ext_label = ext_label(&f.ext);
                                rsx! {
                                    li { class: "files-row", key: "{f.display_path}",
                                        a {
                                            class: "files-link",
                                            href: "{href}",
                                            title: if is_indexed { "open in reader" } else { "preview file" },
                                            span { class: "files-ext files-ext-{f.kind}", "{ext_label}" }
                                            span { class: "files-name", "{f.display_path}" }
                                            if f.size > 0 {
                                                span { class: "files-size mono-sm", "{format_size(f.size)}" }
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
}

fn ext_label(ext: &str) -> String {
    if ext.is_empty() { "FILE".into() } else { ext.to_uppercase() }
}

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    }
}