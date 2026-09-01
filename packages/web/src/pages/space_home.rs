//! `SpaceHome` — the per-space landing at `/source/:encoded`.
//!
//! The landing mode is configurable per source (`web.toml`, edited
//! via the selector on this page):
//!
//! - `journal` — today's entry (open-or-create) + recent entries
//! - `index`   — the `index.org` / `index.md` / `README.*` document
//! - `files`   — recently modified files in the space
//!
//! The mode defaults to `index` and falls back gracefully when the
//! chosen surface has no content (missing index → files listing, no
//! journal dir → create prompt).

use dioxus::prelude::*;

use crate::pages::journal::JournalWidget;
use crate::pages::panel_files::format_mtime;
use crate::pages::use_space_layout;
use crate::router::{route_for_space_files, route_for_space_note, route_for_space_preview};
use crate::server::{
    get_space_ui, list_source_files, load_index_document, IndexDocumentDto,
};
use crate::tree::SourceFileRow;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn SpaceHome(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space().map(|s| s.path.clone());
    let current_encoded = space().map(|s| s.encoded.clone()).unwrap_or_default();

    // Config for the landing mode (web.toml).
    let path_for_ui = active_path.clone();
    let ui_resource = use_server_future(move || {
        let p = path_for_ui.clone();
        async move {
            match p {
                Some(p) => get_space_ui(p).await.ok(),
                None => None,
            }
        }
    })?;
    let landing = ui_resource
        .cloned()
        .flatten()
        .map(|ui| ui.landing)
        .unwrap_or_else(|| "index".to_string());

    // Index document (needed in `index` mode and as fallback).
    let path_for_index = active_path.clone();
    let index_resource = use_server_future(move || {
        let p = path_for_index.clone();
        async move {
            match p {
                Some(p) => load_index_document(p).await.ok(),
                None => None,
            }
        }
    })?;
    let index_doc: Option<IndexDocumentDto> = index_resource.cloned().flatten();

    // Files listing (`files` mode + fallbacks).
    let path_for_files = active_path.clone();
    let files_resource = use_server_future(move || {
        let p = path_for_files.clone();
        async move {
            match p {
                Some(p) => {
                    let mut files = list_source_files(p).await.unwrap_or_default();
                    files.sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms));
                    files.truncate(12);
                    files
                }
                None => Vec::new(),
            }
        }
    })?;
    let files: Vec<SourceFileRow> = files_resource.cloned().unwrap_or_default();

    let space_snapshot = space().clone();
    let source_name = match &space_snapshot {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => s.name.clone(),
        _ => "space".to_string(),
    };
    let space_error = match space_snapshot.as_ref().map(|s| &s.status) {
        Some(SpaceStatus::Error(e)) => Some(e.to_string()),
        _ => None,
    };
    let no_space = matches!(space_snapshot, None);

    let decoded_space = crate::router::decode_space(&current_encoded);
    let files_href = route_for_space_files(&decoded_space);

    let index_entry = index_doc.as_ref().and_then(|d| d.entry.clone());
    let rendered_doc = index_doc.as_ref().and_then(|d| d.document.as_ref());

    // Headline copy per state.
    let (eyebrow, h1, lede) = if space_resolving(&space_snapshot) {
        ("space".to_string(), "resolving…".to_string(), "checking the source path.".to_string())
    } else if let Some(err) = space_error.as_ref() {
        ("space error".to_string(), source_name.clone(), err.clone())
    } else if no_space {
        ("notez".to_string(), "no space selected".to_string(), "pick a source from the sidebar.".to_string())
    } else {
        ("space".to_string(), source_name.clone(), match landing.as_str() {
            "journal" => "daily journal landing — today's note is one click away.".to_string(),
            "files" => "recent files landing — the freshest notes first.".to_string(),
            _ => "index document landing.".to_string(),
        })
    };

    rsx! {
        div { class: "page page-space-home",
            header { class: "page-head",
                p { class: "page-eyebrow", "{eyebrow}" }
                h1 { class: "page-title", "{h1}" }
                p { class: "page-lede", "{lede}" }
            }

            if space_error.is_none() && !no_space {
                details { class: "customize customize-landing",
                    summary { "Landing" }
                    form { class: "landing-form", method: "post", action: "/api/web/landing",
                        input { r#type: "hidden", name: "source_root", value: "{decoded_space}" }
                        label { class: "radio",
                            input { r#type: "radio", name: "landing", value: "journal", checked: landing == "journal" }
                            span { "Journal (today's note)" }
                        }
                        label { class: "radio",
                            input { r#type: "radio", name: "landing", value: "index", checked: landing == "index" }
                            span { "Index document" }
                        }
                        label { class: "radio",
                            input { r#type: "radio", name: "landing", value: "files", checked: landing == "files" }
                            span { "Recent files" }
                        }
                        button { class: "btn", r#type: "submit", "Save" }
                    }
                }

                {match landing.as_str() {
                    "journal" => rsx! {
                        if space_ready_snapshot(&space_snapshot) {
                            JournalWidget { decoded_space: decoded_space.clone() }
                        }
                    },
                    "files" => rsx! {
                        section { class: "widget widget-recent",
                            div { class: "widget-head",
                                h2 { class: "widget-title", "Recent files" }
                                a { class: "widget-more", href: "{files_href}", "all →" }
                            }
                            FilesTable { files: files.clone(), decoded_space: decoded_space.clone() }
                        }
                    },
                    _ => rsx! {
                        match (index_entry.as_ref(), rendered_doc) {
                            (Some(_), Some(doc)) if !doc.body_html.is_empty() => rsx! {
                                article { class: "note-body note-body-landing",
                                    div { dangerous_inner_html: "{doc.body_html}" }
                                }
                            },
                            (Some(entry), _) => rsx! {
                                section { class: "widget",
                                    div { class: "widget-body",
                                        p { class: "empty-hint", "Index document has no rendered body yet." }
                                        a { class: "btn", href: "{route_for_space_note(&decoded_space, &entry.ref_str)}", "open {entry.title}" }
                                    }
                                }
                            },
                            _ => rsx! {
                                section { class: "widget widget-recent",
                                    div { class: "widget-head",
                                        h2 { class: "widget-title", "Recent files" }
                                        a { class: "widget-more", href: "{files_href}", "all →" }
                                    }
                                    p { class: "empty-hint", "No index document (index.org / index.md / README) — showing recent files instead." }
                                    FilesTable { files: files.clone(), decoded_space: decoded_space.clone() }
                                }
                            },
                        }
                    },
                }}
            }
        }
    }
}

fn space_resolving(s: &Option<SpaceState>) -> bool {
    matches!(s.as_ref().map(|x| &x.status), Some(SpaceStatus::Resolving))
}

fn space_ready_snapshot(s: &Option<SpaceState>) -> bool {
    matches!(s.as_ref().map(|x| &x.status), Some(SpaceStatus::Ready(_)))
}

#[component]
fn FilesTable(files: Vec<SourceFileRow>, decoded_space: String) -> Element {
    if files.is_empty() {
        return rsx! { p { class: "empty-hint", "No files indexed yet." } };
    }
    rsx! {
        div { class: "widget-body",
            ul { class: "recent-list",
                for f in files.iter() {
                    li { class: "recent-row",
                        a {
                            class: "recent-link",
                            href: if f.ref_str.is_empty() {
                                route_for_space_preview(&decoded_space, &f.display_path)
                            } else {
                                route_for_space_note(&decoded_space, &f.ref_str)
                            },
                            "{f.title}"
                        }
                        span { class: "recent-when dim", "{format_mtime(f.mtime_ms)}" }
                    }
                }
            }
        }
    }
}
