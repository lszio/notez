//! `PreviewPage` — in-app preview of any file on disk under a space.
//!
//! Route: `/source/:encoded/preview/:encoded_locator`. The locator
//! is base64url-encoded into the path segment so paths containing
//! `/` (which would otherwise break the Dioxus router's single
//! `:param` match) survive intact. The page delegates to
//! `body::render_path` for loose files and to `body::render_body`
//! for indexed resources — the rendering is unified, so PDFs get
//! an `<iframe>`, images get an `<img>`, XLSX gets an inline
//! table, and unknown types get a download link to the raw
//! endpoint.

use dioxus::prelude::*;

use crate::pages::use_space_layout;
use crate::router::route_for_space_list;
use crate::server::render_preview;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn PreviewPage(encoded: String, encoded_locator: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();
    let mut resource_ref_ctx = use_context::<Signal<Option<String>>>();
    use_effect(move || resource_ref_ctx.set(None));

    let active_path = space().map(|s| s.path.clone());
    let current_encoded = space().map(|s| s.encoded.clone()).unwrap_or_default();
    let decoded_space = crate::router::decode_space(&current_encoded);
    let locator = crate::router::decode_locator(&encoded_locator);

    let path_for_fetch = active_path.clone();
    let locator_for_fetch = locator.clone();
    let preview_resource = use_server_future(move || {
        let p = path_for_fetch.clone();
        let l = locator_for_fetch.clone();
        async move {
            match (p, l) {
                (Some(p), l) if !l.is_empty() => render_preview(p, l).await,
                _ => Ok(None),
            }
        }
    })?;

    let preview_body: Option<crate::server::Preview> = preview_resource
        .cloned()
        .and_then(|r| r.ok())
        .flatten();

    let space_snapshot = space().clone();
    let space_path = active_path.clone().unwrap_or_default();
    let list_href = route_for_space_list(&space_path);

    rsx! {
        div { class: "page",
            div { class: "page-h",
                p { class: "eyebrow",
                    "preview"
                    if let Some(ref p) = preview_body {
                        span { class: "mono-sm", " · {p.display_path}" }
                    }
                }
                h1 { if let Some(ref p) = preview_body { "{p.title}" } else { "Preview" } }
                p { class: "lede",
                    if let Some(ref p) = preview_body {
                        span { "size: {format_size(p.size)} · kind: {p.kind}" }
                    } else {
                        "no locator provided."
                    }
                }
            }

            if let Some(ref p) = preview_body {
                div { class: "preview-toolbar",
                    a { class: "control-action", href: "{list_href}", "← back to index" }
                    a {
                        class: "control-action",
                        href: "/api/sources/attachment/raw?source_root={urlencoding::encode(&decoded_space)}&locator={urlencoding::encode(&p.display_path)}",
                        target: "_blank",
                        "open raw ↗"
                    }
                }
            }

            match (space_snapshot.as_ref(), preview_resource.cloned()) {
                (Some(s), _) if !matches!(s.status, SpaceStatus::Ready(_)) => rsx! {
                    p { class: "err-text", "space {s.path.clone()} is {s.status:?}" }
                },
                (_, Some(Err(e))) => rsx! {
                    p { class: "err-text", "preview failed: {e}" }
                },
                (_, Some(Ok(None))) => rsx! {
                    p { class: "err-text", "file not found." }
                },
                (_, Some(Ok(Some(p)))) => rsx! {
                    div { class: "detail-body",
                        div { dangerous_inner_html: "{p.body_html}" }
                    }
                },
                (_, None) => rsx! {
                    p { class: "skel", "loading…" }
                },
            }
        }
    }
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