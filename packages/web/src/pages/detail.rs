//! Resource detail page — the document workbench.
//!
//! One page context hosts three modes per `docs/ui-refactoring-v1.org`
//! §5.1:
//!
//! - **Read** — the rendered document body (default).
//! - **Edit** — the raw Markdown/Org editor with save state.
//! - **Source** — the same raw editor with source-file chrome
//!   (locator + on-disk revision), so raw text is always one tab
//!   away without a separate route.
//!
//! Mode switching is a pure-CSS radio-tab pattern (no JavaScript, no
//! reload): the three radios are visually hidden but keyboard
//! reachable; labels toggle the active pane. The editor pane is a
//! single `<textarea>` shared by Edit and Source, so a draft is never
//! duplicated or lost when switching modes. `dangerous_inner_html`
//! + `html_escape::encode_text` keeps SSR textarea content clean
//! (Dioxus hydration comments would otherwise leak into the value).
//!
//! Save/conflict feedback arrives via the `DetailQuery` params that
//! the `/api/sources/document/edit` route redirects back with; stale
//! revisions and read-only sources render as structured banners with
//! next actions, and the browser keeps a `sessionStorage` draft keyed
//! by ref so a failed save does not destroy the user's edits.

use dioxus::prelude::*;
use ui::notez::{NzBadge, NzButton, NzCard};

use crate::model::ResourceRow;
use crate::pages::ui::{Breadcrumb, BreadcrumbSegment, KindIcon};
use crate::pages::use_space_layout;
use crate::router::{DetailQuery, route_for_space_list, route_for_space_home, route_for_space_preview};
use crate::server::{get_resource, read_document_content};
use crate::space_ctx::{SpaceState, SpaceStatus};

/// Combined payload for the workbench: the indexed `ResourceRow` plus
/// the raw on-disk source text (the server's `get_resource` returns
/// rendered HTML only; `read_document_content` supplies the editor).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DocDetail {
    row: ResourceRow,
    raw_content: String,
}

/// Human format label derived from the locator extension.
fn format_label(locator: &str) -> &'static str {
    let ext = std::path::Path::new(locator)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        "md" | "markdown" => "markdown",
        "org" => "org",
        "txt" => "text",
        _ => "document",
    }
}

/// Short tail of a revision hash for display.
fn short_rev(rev: &str) -> String {
    if rev.len() > 10 {
        format!("…{}", &rev[rev.len() - 10..])
    } else {
        rev.to_string()
    }
}

#[component]
pub fn DetailPage(encoded: String, encoded_ref: String, query: DetailQuery) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let decoded_ref = crate::router::decode_space(&encoded_ref);

    // Publish the active ref so the right rail can render.
    let mut resource_ref_ctx = use_context::<Signal<Option<String>>>();
    let decoded_ref_for_effect = decoded_ref.clone();
    use_effect(move || resource_ref_ctx.set(Some(decoded_ref_for_effect.clone())));

    let space_snapshot = space().clone();
    let path_for_fetch = space_snapshot.as_ref().map(|s| s.path.clone());
    let current_encoded = space_snapshot
        .as_ref()
        .map(|s| s.encoded.clone())
        .unwrap_or_default();
    let source_name = match space_snapshot.as_ref() {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => s.name.clone(),
        _ => "?".to_string(),
    };

    // Single round trip: row + raw content together (two sequential
    // `use_server_future`s would suspend the page twice on SSR).
    let r_for_fetch = decoded_ref.clone();
    let detail = use_server_future(move || {
        let p = path_for_fetch.clone();
        let r = r_for_fetch.clone();
        async move {
            match p {
                Some(path) => {
                    let row = get_resource(path.clone(), r.clone()).await;
                    match row {
                        Ok(Some(row)) => {
                            let raw = if row.kind == "document" {
                                read_document_content(path, r)
                                    .await
                                    .ok()
                                    .flatten()
                                    .map(|d| d.content)
                                    .unwrap_or_default()
                            } else {
                                String::new()
                            };
                            Ok(Some(DocDetail { row, raw_content: raw }))
                        }
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                    }
                }
                None => Ok(None),
            }
        }
    })?;

    let decoded_space = crate::router::decode_space(&current_encoded);
    let list_href = route_for_space_list(&decoded_space);
    let home_href = route_for_space_home(&decoded_space);

    let ref_tail = decoded_ref
        .rsplit_once(':')
        .map(|(_, id)| id.to_string())
        .unwrap_or_else(|| decoded_ref.clone());
    let ref_tail_short = if ref_tail.len() > 12 {
        format!("…{}", &ref_tail[ref_tail.len() - 12..])
    } else {
        ref_tail.clone()
    };

    let crumb_kind = match detail.cloned() {
        Some(Ok(Some(d))) => d.row.kind.clone(),
        _ => "?".to_string(),
    };

    rsx! {
        div { class: "page",
            Breadcrumb {
                segments: vec![
                    BreadcrumbSegment::link("notez", "/"),
                    BreadcrumbSegment::link(home_href.clone(), home_href.clone()),
                    BreadcrumbSegment::link(source_name.clone(), list_href.clone()),
                    BreadcrumbSegment::link(crumb_kind.clone(), list_href.clone()),
                    BreadcrumbSegment::here(ref_tail_short.clone()),
                ],
            }

            match (space_snapshot.as_ref(), detail.cloned()) {
                (Some(s), _) if !matches!(s.status, SpaceStatus::Ready(_)) => rsx! {
                    div { class: "page-h",
                        p { class: "eyebrow", "space" }
                        h1 { "{decoded_ref}" }
                        p { class: "lede",
                            "Source status: "
                            {match &s.status {
                                SpaceStatus::Resolving => "resolving…".to_string(),
                                SpaceStatus::Ready(_) => "ready".to_string(),
                                SpaceStatus::Error(e) => format!("error: {e}"),
                            }}
                        }
                    }
                },
                (_, Some(Err(e))) => rsx! {
                    div { class: "page-h",
                        p { class: "eyebrow", "error" }
                        h1 { "load failed" }
                        p { class: "lede err-text", "{e}" }
                    }
                },
                (_, Some(Ok(None))) => rsx! {
                    div { class: "page-h",
                        p { class: "eyebrow", "not found" }
                        h1 { "{decoded_ref}" }
                        p { class: "lede", "no resource with that ref in this space." }
                    }
                },
                (_, Some(Ok(Some(d)))) => rsx! {
                    DetailBody {
                        row: d.row.clone(),
                        raw_content: d.raw_content.clone(),
                        list_href: list_href.clone(),
                        current_encoded: current_encoded.clone(),
                        query: query.clone(),
                    }
                },
                (_, None) => rsx! {
                    div { class: "page-h",
                        p { class: "skel", "loading…" }
                    }
                },
            }
        }
    }
}

#[component]
fn DetailBody(
    row: ResourceRow,
    raw_content: String,
    list_href: String,
    current_encoded: String,
    query: DetailQuery,
) -> Element {
    let decoded_space = crate::router::decode_space(&current_encoded);
    let editable = row.kind == "document";
    let format = format_label(&row.locator);
    let rev_short = short_rev(&row.revision);
    let is_attachment = row.kind == "attachment";

    rsx! {
        div { class: "detail-main",
            // ---- Save / conflict banners (redirected back from the
            // edit route). Structured, with next actions; never
            // colour-only. ----
            div { class: "doc-banners",
                if !query.edit_err.is_empty() {
                    div { class: "doc-banner doc-banner-err", role: "alert",
                        div { class: "doc-banner-head",
                            span { class: "doc-banner-kind", "{query.edit_err}" }
                            if !query.edit_msg.is_empty() {
                                span { class: "doc-banner-msg", "{query.edit_msg}" }
                            }
                        }
                        div { class: "doc-banner-actions",
                            if query.edit_err == "stale_revision" {
                                a { class: "doc-banner-action", href: "{crate::router::route_for_space_resource(&decoded_space, &row.ref_str)}", "reload" }
                                span { class: "doc-banner-note", "your draft is kept in this browser and restored on reload." }
                            } else if query.edit_err == "read_only" || query.edit_err == "unsupported" {
                                a { class: "doc-banner-action", href: "{list_href}", "back to index" }
                            }
                        }
                    }
                } else if query.edited == "1" {
                    div { class: "doc-banner doc-banner-ok", role: "status",
                        div { class: "doc-banner-head",
                            span { class: "doc-banner-kind", "saved" }
                            span { class: "doc-banner-msg", "changes written to the source file." }
                        }
                    }
                }
            }

            // ---- Document header: source / format / editability /
            // revision / save state, all explicit text. ----
            div { class: "doc-header",
                h1 { "{row.title}" }
                div { class: "doc-refline",
                    span { class: "mono-sm", "{row.ref_str}" }
                    span { class: "dim", "  ·  " }
                    a { href: "{list_href}", "back to index" }
                }
                div { class: "doc-status",
                    NzBadge { text: row.source_id.clone(), tone: "info".to_string() }
                    NzBadge { text: format.to_string(), tone: "info".to_string() }
                    if editable {
                        NzBadge { text: "editable".to_string(), tone: "ok".to_string() }
                    } else {
                        NzBadge { text: "read-only".to_string(), tone: "warn".to_string() }
                    }
                    NzBadge { text: format!("rev {}", rev_short.clone()), tone: "info".to_string() }
                    span {
                        class: "doc-save-state",
                        "aria-live": "polite",
                        "data-save-state": "true",
                        if editable { "Saved" } else { "Read-only" }
                    }
                }
            }

            if editable {
                // ---- Read / Edit / Source mode tabs. Pure-CSS radio
                // tabs: works without hydration, keeps every pane in
                // the DOM so drafts survive mode switches. ----
                div { class: "doc-modes",
                    input { id: "doc-mode-read", class: "doc-mode-input", name: "doc_mode", r#type: "radio", value: "read", checked: true }
                    input { id: "doc-mode-edit", class: "doc-mode-input", name: "doc_mode", r#type: "radio", value: "edit" }
                    input { id: "doc-mode-source", class: "doc-mode-input", name: "doc_mode", r#type: "radio", value: "source" }
                    div { class: "doc-mode-tabs", role: "group", "aria-label": "Document mode",
                        label { class: "doc-mode-tab", r#for: "doc-mode-read", "Read" }
                        label { class: "doc-mode-tab", r#for: "doc-mode-edit", "Edit" }
                        label { class: "doc-mode-tab", r#for: "doc-mode-source", "Source" }
                    }
                    div { class: "doc-mode-panes",
                        div { class: "doc-pane doc-pane-read",
                            if !row.body_html.is_empty() {
                                div { class: "detail-body",
                                    div { dangerous_inner_html: "{row.body_html}" }
                                }
                            } else {
                                p { class: "props-empty",
                                    "No rendered body yet — add a `* heading` line to the document and rescan."
                                }
                            }
                        }
                        div { class: "doc-pane doc-pane-editor",
                            div { class: "editor-chrome",
                                span { class: "editor-mode-label em-edit", "Edit · {format}" }
                                span { class: "editor-mode-label em-source", "Source · {format}" }
                                span { class: "editor-source-locator mono-sm", "{row.locator}" }
                            }
                            form {
                                class: "doc-edit-form",
                                method: "post",
                                action: "/api/sources/document/edit",
                                "data-draft-key": "{row.ref_str}",
                                input { r#type: "hidden", name: "source_root", value: "{decoded_space}" }
                                input { r#type: "hidden", name: "ref_str", value: "{row.ref_str}" }
                                input { r#type: "hidden", name: "expected_revision", value: "{row.revision}" }
                                div { class: "editor-save-row",
                                    span { class: "doc-save-state", "aria-live": "polite", "data-save-state": "true", "Saved" }
                                    span { class: "editor-draft-note mono-sm", hidden: true, "draft kept in this browser" }
                                    span { class: "editor-rev mono-sm", "on-disk rev {rev_short}" }
                                }
                                textarea {
                                    name: "content",
                                    class: "edit-source-textarea",
                                    rows: "20",
                                    "aria-label": "Source content for {row.title}",
                                    dangerous_inner_html: "{html_escape::encode_text(&raw_content)}"
                                }
                                div { class: "edit-source-actions",
                                    NzButton {
                                        kind: "submit".to_string(),
                                        accent: true,
                                        aria_label: "Save changes".to_string(),
                                        "save changes"
                                    }
                                    a { class: "edit-source-cancel", href: "{crate::router::route_for_space_resource(&decoded_space, &row.ref_str)}", "cancel" }
                                }
                            }
                        }
                    }
                }
            } else {
                // ---- Honest read-only state for non-documents. ----
                if !row.body_html.is_empty() {
                    div { class: "detail-body",
                        div { dangerous_inner_html: "{row.body_html}" }
                    }
                }
                NzCard {
                    padded: true,
                    div { class: "readonly-note",
                        p { "{row.kind} content cannot be edited from Notez." }
                        if is_attachment {
                            p { class: "lede dim",
                                "Open the raw file instead: "
                                a { href: "{route_for_space_preview(&decoded_space, &row.locator)}", "preview ↗" }
                            }
                        }
                    }
                }
            }

            details { class: "janet-panel",
                summary { "janet" }
                div { class: "janet-panel-body",
                    p { class: "janet-panel-hint", "run a Janet expression against the notez runtime." }
                    form {
                        action: "/api/janet/eval",
                        method: "post",
                        target: "_blank",
                        textarea {
                            name: "script",
                            class: "janet-input",
                            rows: "4",
                            placeholder: "(+ 1 2)",
                            "(+ 1 2)"
                        }
                        div { class: "janet-actions",
                            button { class: "edit-source-save", r#type: "submit", "run janet" }
                        }
                    }
                }
            }

            details { class: "meta-drawer",
                summary { "metadata" }
                div { class: "meta-drawer-body",
                    dl { class: "meta-list",
                        dt { "kind" }
                        dd {
                            KindIcon { kind: row.kind.clone() }
                            span { class: "mono-sm", "{row.kind}" }
                        }
                        dt { "source" }
                        dd { "{row.source_id}" }
                        dt { "locator" }
                        dd { class: "muted", "{row.locator}" }
                        dt { "revision" }
                        dd { class: "muted", "{row.revision}" }
                        dt { "object_id" }
                        dd { class: "muted", "{row.object_id}" }
                    }
                }
            }

            h2 { "Properties" }
            if row.properties.is_empty() {
                p { class: "props-empty", "no properties attached to this resource." }
            } else {
                NzCard {
                    padded: true,
                    div { class: "props",
                        for (k, v) in row.properties.iter() {
                            div { class: "row",
                                span { class: "k", "{k}" }
                                span { class: "v", "{v}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
