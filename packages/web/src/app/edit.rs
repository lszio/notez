//! The editor page: textarea + formatting toolbar + live preview.
//!
//! Server-rendered like every other page; the behaviour comes from the
//! `app.js` island, which talks to `POST /api/render` (the same
//! renderer the read page uses) and `POST /save/{space}`.

use dioxus::prelude::*;

use crate::data::space::Space;
use crate::data::urls;

/// Toolbar buttons: `(action, label, title)`.
const TOOLS: &[(&str, &str, &str)] = &[
    ("h1", "H1", "Heading 1  (* )"),
    ("h2", "H2", "Heading 2  (** )"),
    ("h3", "H3", "Heading 3  (*** )"),
    ("bold", "B", "Bold  (*text*)  Ctrl-B"),
    ("italic", "I", "Italic  (/text/)  Ctrl-I"),
    ("code", "=", "Code  (=text=)  Ctrl-E"),
    ("strike", "+", "Strike  (+text+)"),
    ("list", "•", "Bullet list"),
    ("ordered", "1.", "Numbered list"),
    ("checkbox", "☐", "Checkbox"),
    ("quote", "❝", "Quote block"),
    ("table", "▦", "Table"),
    ("link", "🔗", "Link  (Ctrl-K)"),
];

#[component]
pub fn Editor(
    space: Space,
    locator: String,
    revision: String,
    content: String,
    notice: Element,
) -> Element {
    let encoded = space.encoded.clone();
    let save_action = urls::save_url(&encoded);
    let preview_html = crate::data::space::render_document(&space, &locator, &content)
        .unwrap_or_default();
    let is_org = locator.ends_with(".org");

    rsx! {
        {notice}
        div { class: "draft-bar", id: "draft-bar", hidden: true,
            span { "restored draft differs from disk:" }
            a { href: "#", id: "draft-restore", "restore" }
            a { href: "#", id: "draft-discard", "discard" }
        }
        form {
            id: "editor-form",
            method: "post",
            action: "{save_action}",
            "data-locator": "{locator}",
            "data-space": "{encoded}",
            input { r#type: "hidden", name: "locator", value: "{locator}" }
            input { r#type: "hidden", name: "revision", value: "{revision}" }
            div { class: "edit-bar",
                div { class: "toolbar",
                    for (action, label, title) in TOOLS {
                        button {
                            class: "tool",
                            r#type: "button",
                            "data-action": "{action}",
                            title: "{title}",
                            "{label}"
                        }
                    }
                }
                span { class: "spacer" }
                span { class: "edit-hint", id: "editor-state", "saved" }
                span { class: "edit-hint", "Ctrl/Cmd-S 保存 · Tab 缩进" }
                button { class: "btn primary", r#type: "submit", "Save" }
            }
            div { class: "edit-split",
                textarea {
                    class: "editor",
                    id: "editor",
                    name: "content",
                    spellcheck: "false",
                    autocomplete: "off",
                    // Dioxus SSR injects hydration comments around child
                    // text; a textarea is RCDATA, so those comments would
                    // become literal text in `.value`. Escape the body
                    // and inject it raw instead.
                    dangerous_inner_html: "{html_escape::encode_text(&content)}"
                }
                div { class: "preview-pane",
                    div { class: "preview-label", if is_org { "preview · org" } else { "preview · markdown" } }
                    div { class: "doc preview", id: "preview", dangerous_inner_html: "{preview_html}" }
                }
            }
        }
    }
}

/// "Saved" banner, built from the query string.
pub fn save_notice(query: &crate::app::route::DocQuery) -> Element {
    if let Some(revision) = &query.saved {
        let short = &revision[..revision.len().min(12)];
        return rsx! { div { class: "banner ok", "Saved · {short}" } };
    }
    rsx! {}
}

/// Banner for a restored failed save.
pub fn restore_notice(pending: &crate::data::pending::PendingSave) -> Element {
    let class = if pending.kind == "stale" { "banner warn" } else { "banner err" };
    let message = pending.message.clone();
    rsx! { div { class: "{class}", "{message}" } }
}
