//! `NotePage` — the note workbench at `/source/:encoded/note/:ref`.
//!
//! One page hosts three modes per note (pure-CSS radio tabs, no
//! hydration): **Read** (rendered body), **Edit** (raw editor), and
//! **Source** (same editor with on-disk chrome). A single
//! `<textarea>` is shared by Edit and Source so a draft is never
//! duplicated. `dangerous_inner_html` + `html_escape::encode_text`
//! keeps SSR textarea content clean.
//!
//! The right rail (outline / linked mentions / properties / local
//! graph) is driven by the `resource_ref` context signal this page
//! publishes. Save/conflict feedback arrives via `DetailQuery` params
//! redirected back from `/api/sources/document/edit`; the browser
//! keeps a `sessionStorage` draft keyed by ref so a failed save never
//! destroys edits.
//!
use dioxus::prelude::*;
use ui::notez::{NzBadge, NzButton};

use crate::model::ResourceRow;
use crate::pages::ui::KindIcon;
use crate::pages::use_space_layout;
use crate::pages::{
    BacklinksPanel, GraphPanel, OutlinePanel, PropertiesPanel,
};
use crate::router::{
    route_for_space_files, route_for_space_note, route_for_space_preview,
    DetailQuery,
};
use crate::server::{get_resource, get_space_ui, read_document_content};
use crate::space_ctx::{SpaceState, SpaceStatus};

/// Combined payload for the workbench: the indexed `ResourceRow` plus
/// the raw on-disk source text.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DocDetail {
    row: ResourceRow,
    raw_content: String,
}

/// Human format label derived from the locator extension.
fn format_label(locator: &str) -> &'static str {
    match locator.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "md" | "markdown" => "markdown",
        "org" => "org",
        "txt" => "text",
        _ => "file",
    }
}

/// Short tail of a revision hash for display.
fn short_rev(rev: &str) -> String {
    if rev.len() > 12 {
        format!("{}…", &rev[..12])
    } else {
        rev.to_string()
    }
}

#[component]
pub fn NotePage(encoded: String, encoded_ref: String, query: DetailQuery) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let decoded_ref = crate::router::decode_space(&encoded_ref);


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

    // Star state (web.toml) for the header toggle.
    let path_for_star = space_snapshot.as_ref().map(|s| s.path.clone());
    let ref_for_star = decoded_ref.clone();
    let star_resource = use_server_future(move || {
        let p = path_for_star.clone();
        let r = ref_for_star.clone();
        async move {
            match p {
                Some(p) => get_space_ui(p).await.map(|ui| ui.starred.iter().any(|s| s == &r)).unwrap_or(false),
                None => false,
            }
        }
    })?;
    let starred: bool = star_resource.cloned().unwrap_or(false);


    rsx! {
        div { class: "page page-note",
            match (space_snapshot.as_ref(), detail.cloned()) {
                (Some(s), _) if !matches!(s.status, SpaceStatus::Ready(_)) => rsx! {
                    header { class: "page-head",
                        p { class: "page-eyebrow", "space" }
                        h1 { class: "page-title", "{decoded_ref}" }
                        p { class: "page-lede",
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
                    header { class: "page-head",
                        p { class: "page-eyebrow", "error" }
                        h1 { class: "page-title", "load failed" }
                        p { class: "page-lede err-text", "{e}" }
                    }
                },
                (_, Some(Ok(None))) => rsx! {
                    header { class: "page-head",
                        p { class: "page-eyebrow", "not found" }
                        h1 { class: "page-title", "{decoded_ref}" }
                        p { class: "page-lede", "no resource with that ref in this space." }
                    }
                },
                (_, Some(Ok(Some(d)))) => rsx! {
                    div { class: "note-grid",
                        NoteBody {
                            row: d.row.clone(),
                            raw_content: d.raw_content.clone(),
                            current_encoded: current_encoded.clone(),
                            source_name: source_name.clone(),
                            starred,
                            query: query.clone(),
                        }
                        aside { class: "rail", "aria-label": "Note inspector",
                            OutlinePanel { active_encoded: current_encoded.clone(), active_ref: decoded_ref.clone() }
                            BacklinksPanel { active_encoded: current_encoded.clone(), active_ref: decoded_ref.clone() }
                            PropertiesPanel { active_encoded: current_encoded.clone(), active_ref: decoded_ref.clone() }
                            GraphPanel { active_encoded: current_encoded.clone(), active_ref: decoded_ref.clone() }
                        }
                    }
                },
                (_, None) => rsx! {
                    p { class: "skel", "loading…" }
                },
            }
        }
    }
}

#[component]
fn NoteBody(
    row: ResourceRow,
    raw_content: String,
    current_encoded: String,
    source_name: String,
    starred: bool,
    query: DetailQuery,
) -> Element {
    let decoded_space = crate::router::decode_space(&current_encoded);
    let files_href = route_for_space_files(&decoded_space);
    let editable = row.kind == "document";
    let format = format_label(&row.locator);
    let rev_short = short_rev(&row.revision);
    let is_attachment = row.kind == "attachment";

    rsx! {
        div { class: "note-main",

            // ---- Save / conflict banners (redirected back from the
            // edit route). Structured, with next actions. ----
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
                            a { class: "doc-banner-action", href: "{route_for_space_note(&decoded_space, &row.ref_str)}", "reload" }
                            span { class: "doc-banner-note", "your draft is kept in this browser and restored on reload." }
                        } else if query.edit_err == "read_only" || query.edit_err == "unsupported" {
                            a { class: "doc-banner-action", href: "{files_href}", "back to files" }
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

            // ---- Note header: title, star toggle, ref line, badges. ----
            header { class: "note-head",
                div { class: "note-head-top",
                    h1 { class: "note-title", "{row.title}" }
                    form { method: "post", action: "/api/web/star", class: "star-form",
                        input { r#type: "hidden", name: "source_root", value: "{decoded_space}" }
                        input { r#type: "hidden", name: "ref_str", value: "{row.ref_str}" }
                        button {
                            class: if starred { "star-btn is-starred" } else { "star-btn" },
                            r#type: "submit",
                            "aria-pressed": "{starred}",
                            "aria-label": if starred { "Remove star" } else { "Star this note" },
                            title: if starred { "starred" } else { "star" },
                            if starred { "★" } else { "☆" }
                        }
                    }
                }
                div { class: "note-refline",
                    span { class: "mono-sm note-ref", "{row.ref_str}" }
                    span { class: "dim", "·" }
                    a { href: "{files_href}", "all files" }
                }
                div { class: "note-status",
                    NzBadge { text: source_name.clone(), tone: "info".to_string() }
                    NzBadge { text: format.to_string(), tone: "info".to_string() }
                    if editable {
                        NzBadge { text: "editable".to_string(), tone: "ok".to_string() }
                    } else {
                        NzBadge { text: "read-only".to_string(), tone: "warn".to_string() }
                    }
                    NzBadge { text: format!("rev {}", rev_short.clone()), tone: "info".to_string() }
                }
            }

            if editable {
                // ---- Read / Edit / Source mode tabs (pure CSS). ----
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
                                article { class: "note-body",
                                    div { dangerous_inner_html: "{row.body_html}" }
                                }
                            } else {
                                p { class: "empty-hint",
                                    "No rendered body yet — add a `# heading` (or `* heading` in Org) and rescan."
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
                                    a { class: "edit-source-cancel", href: "{route_for_space_note(&decoded_space, &row.ref_str)}", "cancel" }
                                }
                            }
                        }
                    }
                }
            } else {
                // ---- Honest read-only state for non-documents. ----
                if !row.body_html.is_empty() {
                    article { class: "note-body",
                        div { dangerous_inner_html: "{row.body_html}" }
                    }
                }
                div { class: "readonly-note",
                    p { "{row.kind} content cannot be edited from Notez." }
                    if is_attachment {
                        p { class: "dim",
                            "Open the raw file: "
                            a { href: "{route_for_space_preview(&decoded_space, &row.locator)}", "preview ↗" }
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
        }
    }
}
