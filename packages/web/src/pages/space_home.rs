//! `SpaceHome` — the per-space welcome at `/space/:encoded`.

use dioxus::prelude::*;

use crate::pages::ui::KindIcon;
use crate::pages::use_space_layout;
use crate::router::{route_for_space_list, route_for_space_preview, route_for_space_resource};
use crate::server::{get_resource, list_filesystem, resolve_index};
use crate::space_ctx::{SpaceState, SpaceStatus};
use crate::tree::IndexEntryDto;

#[component]
pub fn SpaceHome(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space().map(|s| s.path.clone());
    let current_encoded = space().map(|s| s.encoded.clone()).unwrap_or_default();

    let path_for_index = active_path.clone();
    let index_resource = use_server_future(move || {
        let p = path_for_index.clone();
        async move {
            match p {
                Some(p) => resolve_index(p).await,
                None => Ok(None),
            }
        }
    })?;

    let index_entry: Option<IndexEntryDto> = index_resource.cloned().and_then(|r| r.ok()).flatten();

    let path_for_doc = active_path.clone();
    let index_doc_ref_str = index_entry.as_ref().map(|i| i.ref_str.clone());
    let index_doc_resource = use_server_future(move || {
        let p = path_for_doc.clone();
        let r = index_doc_ref_str.clone();
        async move {
            match (p, r) {
                (Some(p), Some(r)) => get_resource(p, r).await,
                _ => Ok(None),
            }
        }
    })?;

    let path_for_files = active_path.clone();
    let files_resource = use_server_future(move || {
        let p = path_for_files.clone();
        async move {
            match p {
                Some(p) => list_filesystem(p).await,
                None => Ok(Vec::new()),
            }
        }
    })?;

    let index_doc = index_doc_resource.cloned().and_then(|r| r.ok()).flatten();
    let files = files_resource.cloned().and_then(|r| r.ok()).unwrap_or_default();

    let space_snapshot = space().clone();
    let (eyebrow, h1, lede) = match &space_snapshot {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => {
            let count = files.len();
            (
                format!("space · {}", s.name),
                index_entry.as_ref().map(|i| i.title.clone()).unwrap_or_else(|| "Welcome".into()),
                if index_doc.is_some() {
                    format!("{count} files · index rendered inline below.")
                } else {
                    format!("{count} files on disk · no index document found.")
                },
            )
        }
        Some(SpaceState { status: SpaceStatus::Resolving, .. }) => (
            "resolving".into(),
            "Welcome".into(),
            "looking up the space…".into(),
        ),
        Some(SpaceState { status: SpaceStatus::Error(e), .. }) => (
            "error".into(),
            "Welcome".into(),
            format!("space error: {e}"),
        ),
        None => ("no space".into(), "Welcome".into(), "pick a space to begin.".into()),
    };

    let decoded = crate::router::decode_space(&current_encoded);
    let active_path_for_list = active_path.clone().unwrap_or_default();

    rsx! {
        div { class: "page",
            div { class: "page-h",
                p { class: "eyebrow", "{eyebrow}" }
                h1 { "{h1}" }
                p { class: "lede", "{lede}" }
            }

            if let Some(doc) = index_doc.clone() {
                div { class: "welcome-index",
                    div { class: "welcome-index-meta mono-sm",
                        KindIcon { kind: doc.kind.clone() }
                        a { href: "{route_for_space_resource(&decoded, &doc.ref_str)}", "{doc.ref_str}" }
                        span { class: "dim", " · {doc.locator}" }
                    }
                    if !doc.body_html.is_empty() {
                        div { class: "detail-body",
                            div { dangerous_inner_html: "{doc.body_html}" }
                        }
                    } else {
                        p { class: "props-empty", "this document has no inline body." }
                    }
                }
            }

            div { class: "welcome-files",
                div { class: "welcome-files-head",
                    span { class: "welcome-files-label", "files on disk" }
                    a { class: "welcome-files-link mono-sm", href: "{route_for_space_list(&active_path_for_list)}", "open full list →" }
                }
                if files.is_empty() {
                    p { class: "welcome-files-empty", "no files yet." }
                } else {
                    ul { class: "welcome-files-list",
                        for f in files.iter() {
                            {
                                let is_indexed = !f.ref_str.is_empty();
                                let href = if is_indexed {
                                    route_for_space_resource(&decoded, &f.ref_str)
                                } else {
                                    route_for_space_preview(&decoded, &f.display_path)
                                };
                                let label = if f.ext.is_empty() { f.kind.to_uppercase() } else { f.ext.to_uppercase() };
                                rsx! {
                                    li { class: "welcome-files-row", key: "{f.display_path}",
                                        a { class: "welcome-files-link2", href: "{href}",
                                            span { class: "welcome-files-ext kind-{f.kind}", "{label}" }
                                            span { class: "welcome-files-name", "{f.display_path}" }
                                            span { class: "welcome-files-title dim", "{f.title}" }
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