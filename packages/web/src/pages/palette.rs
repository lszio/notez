//! `CommandPalette` and `SearchTrigger` — global search overlay.
//!
//! Architecture:
//!
//! - Open state lives in a `Signal<bool>` provided by `Layout` via
//!   `use_context_provider`. Both `SearchTrigger` and
//!   `CommandPalette` read it via `use_context`.
//! - `SearchTrigger` is a `<button>` (not the previous
//!   `<details>`/`<summary>`) that toggles the signal to `true`.
//! - `CommandPalette` renders a `position: fixed` overlay only when
//!   the signal is `true`. Click on the backdrop closes; click on
//!   the modal panel does not.
//! - All keyboard handling lives on the search input's
//!   `onkeydown` so we don't need to register a document-level
//!   key listener. When the modal is open, the input is focused
//!   (`autofocus: true`), so it catches Arrow / Enter / Esc directly.
//! - Results are grouped by kind (`documents` / `headings` /
//!   `attachments`) with a section header per group, mirroring the
//!   file panel ordering.
//! - Attachment hits route to the preview route (their `locator`
//!   is the relative path on disk); documents and headings route
//!   to the resource reader.

use dioxus::prelude::*;


use crate::router::{route_for_space_preview, route_for_space_resource};
use crate::server::search_palette;
use crate::space_ctx::{SpaceState, SpaceStatus};
use crate::tree::SearchHit;

/// Context key for the palette open/closed signal. Layout installs
/// the signal via `use_context_provider`; this module re-exports the
/// key for callers that need to set the state from outside.
pub type PaletteOpen = Signal<bool>;

#[component]
pub fn SearchTrigger() -> Element {
    let mut open = use_context::<PaletteOpen>();
    rsx! {
        button {
            class: "search-trigger",
            r#type: "button",
            title: "Open search (Ctrl+K)",
            "aria-label": "Open search",
            onclick: move |_| open.set(true),
            span { class: "search-icon", "⌕" }
            span { class: "search-label", "search…" }
            span { class: "search-kbd mono-sm", "⌘K" }
        }
    }
}
#[component]
pub fn CommandPalette() -> Element {
    // PR9: always render the overlay div, even when "closed". The
    // overlay's CSS `display: none` keeps it hidden, and the inline
    // script in `<Layout>` toggles `.is-open` to show / hide. This
    // is the SSR-only compatible way to drive the modal — Dioxus
    // signals (and therefore the previous `if *open.read()` branch)
    // would never fire on the client.
    rsx! {
        div {
            class: "palette-overlay",
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "Search this space",
            div {
                class: "palette-modal",
                // The inline script handles click-to-close on the
                // backdrop via the document-level `click` listener
                // (matched on .search-trigger / .palette-close), so
                // no inline onclick is needed here.
                PaletteBody {}
            }
        }
    }
}
#[allow(non_snake_case)]
fn PaletteBody() -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_encoded = space().map(|s| s.encoded.clone());
    let mut open = use_context::<PaletteOpen>();
    // PR9: do NOT use use_navigator here — CommandPalette is
    // mounted from <Layout>, which is OUTSIDE the <AppRouter>
    // tree. use_navigator panics with "Must be called in a
    // descendant of a Router component". Keyboard Enter on a hit
    // is handled by the PR9 inline script (delegated click on
    // the focused <a>).
    let mut query = use_signal(String::new);
    // Selected row index for arrow-key navigation. Reset to 0 on
    // every new query.
    let mut selected = use_signal(|| 0usize);

    let active_path = space().map(|s| s.path.clone());
    let q_for_fetch = query();
    let path_for_fetch = active_path.clone();
    let hits_resource = use_server_future(move || {
        let p = path_for_fetch.clone();
        let q = q_for_fetch.clone();
        async move {
            match p {
                Some(p) if !q.trim().is_empty() => search_palette(p, q).await.unwrap_or_default(),
                _ => Vec::new(),
            }
        }
    })?;

    let hits: Vec<SearchHit> = hits_resource.cloned().unwrap_or_default();
    let encoded = active_encoded.clone().unwrap_or_default();
    let decoded_space = crate::router::decode_space(&encoded);

    // Group hits by kind. Each hit is cloned into an owned
    // `SearchHit` so the keyboard closure and the rsx! builder
    // can each hold their own copy without fighting over a
    // borrow of `hits`.
    let mut docs: Vec<SearchHit> = Vec::new();
    let mut heads: Vec<SearchHit> = Vec::new();
    let mut attaches: Vec<SearchHit> = Vec::new();
    for h in hits.iter() {
        match h.kind.as_str() {
            "document" => docs.push(h.clone()),
            "heading" => heads.push(h.clone()),
            "attachment" => attaches.push(h.clone()),
            _ => docs.push(h.clone()),
        }
    }
    let groups: Vec<(&'static str, &Vec<SearchHit>)> = vec![
        ("documents", &docs),
        ("headings", &heads),
        ("attachments", &attaches),
    ];
    let groups: Vec<(&'static str, Vec<SearchHit>)> = groups
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(label, v)| (label, v.clone()))
        .collect();

    let flat_hits: Vec<SearchHit> = groups
        .iter()
        .flat_map(|(_, v)| v.iter().cloned())
        .collect();
    let total_hits = flat_hits.len();

    // Build the row body once; the onkeydown closure captures it
    // Clone everything the onkeydown closure captures so the
    // rsx! builder below can keep using the originals.
    let onkeydown = {
        let decoded_space = decoded_space.clone();
        let flat_hits = flat_hits.clone();
        let mut open = open.clone();
        let mut selected = selected.clone();
        move |e: KeyboardEvent| {
        match e.key() {
            Key::ArrowDown => {
                e.prevent_default();
                let cur = selected.cloned();
                let max = total_hits.saturating_sub(1);
                let next = if cur >= max { 0 } else { cur + 1 };
                selected.set(next);
            }
            Key::ArrowUp => {
                e.prevent_default();
                let cur = selected.cloned();
                let next = if cur == 0 {
                    total_hits.saturating_sub(1)
                } else {
                    cur - 1
                };
                selected.set(next);
            }
            Key::Enter => {
                e.prevent_default();
                let cur = selected.cloned();
                if let Some(hit) = flat_hits.get(cur) {
                    open.set(false);
                    // No navigator available; rely on the inline
                    // script's `keydown` listener to focus the
                    // <a class="palette-hit-link"> of the selected
                    // row + dispatch Enter via DOM (TODO PR9.5).
                }
                // Also clear the open signal so the inline script's
                // CSS class toggle (added by PR9) stays in sync.
                // The browser-side JS will see is-open removed.
                // (The PR9 script also flips the class on its own.)
                open.set(false);
            }
            _ => {}
         }
     }
     };
    let palette_content: Element = match space().map(|s| s.status.clone()) {
        Some(SpaceStatus::Resolving) => rsx! { p { class: "palette-empty", "loading space…" } },
        Some(SpaceStatus::Error(e)) => rsx! { p { class: "palette-empty err-text", "error: {e}" } },
        _ => {
            if active_path.is_none() {
                rsx! { p { class: "palette-empty", "select a space to search." } }
            } else if query().trim().is_empty() {
                rsx! { p { class: "palette-empty", "type to search this space." } }
            } else if hits.is_empty() {
                rsx! { p { class: "palette-empty", "no matches." } }
            } else {
                let mut flat_index: usize = 0;
                rsx! {
                    div { class: "palette-list-wrap",
                        for (group_label, group_hits) in groups.iter() {
                            div { class: "palette-group", key: "{group_label}",
                                div { class: "palette-group-head mono-sm", "{group_label}" }
                                ul { class: "palette-list",
                                    for h in group_hits.iter() {
                                        {
                                            let this_index = flat_index;
                                            flat_index += 1;
                                            let is_selected = selected.cloned() == this_index;
                                            let href = hit_href(&decoded_space, h);
                                            let kind_label = match_field_label(&h.match_field);
                                            let id = format!("palette-hit-{this_index}");
                                            rsx! {
                                                li {
                                                    class: if is_selected {
                                                        "palette-hit palette-hit-selected"
                                                    } else {
                                                        "palette-hit"
                                                    },
                                                    key: "{h.ref_str}",
                                                    a {
                                                        class: "palette-hit-link",
                                                        id: "{id}",
                                                        href: "{href}",
                                                         div { class: "palette-hit-head",
                                                             span { class: "palette-hit-kind kind-{h.kind}", "{kind_label}" }
                                                             span { class: "palette-hit-title", "{h.title}" }
                                                         }
                                                        if !h.snippet.is_empty() {
                                                            div { class: "palette-hit-snippet mono-sm", "{h.snippet}" }
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
        }
    };

    rsx! {
        div { class: "palette-prompt",
            span { class: "palette-mark", "⌕" }
            input {
                class: "palette-input",
                r#type: "search",
                autofocus: true,
                placeholder: "search titles, headings, bodies, filenames…",
                value: "{query}",
                oninput: move |e| {
                    query.set(e.value());
                    selected.set(0);
                },
                onkeydown: onkeydown,
            }
            button {
                class: "palette-close mono-sm",
                r#type: "button",
                title: "Close (Esc)",
                onclick: move |_| open.set(false),
                "esc"
            }
        }
        div { class: "palette-results",
            {palette_content}
        }
    }
}

/// Build the navigation URL for a hit. Documents and headings go to
/// the resource reader; attachments go to the preview route (their
/// `locator` is the relative path on disk).
fn hit_href(encoded_space: &str, hit: &SearchHit) -> String {
    if hit.kind == "attachment" {
        route_for_space_preview(encoded_space, &hit.locator)
    } else {
        route_for_space_resource(encoded_space, &hit.ref_str)
    }
}

fn match_field_label(field: &str) -> String {
    match field {
        "title" => "title".into(),
        "body" => "body".into(),
        "filename" => "file".into(),
        "heading" => "hd".into(),
        other => other.into(),
    }
}

