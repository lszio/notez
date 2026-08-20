//! `CommandPalette` and `SearchTrigger` — global ⌘K search overlay.

use dioxus::prelude::*;

use crate::router::route_for_space_resource;
use crate::server::search_palette;
use crate::space_ctx::{SpaceState, SpaceStatus};
use crate::tree::SearchHit;

#[component]
pub fn SearchTrigger() -> Element {
    rsx! {
        details { class: "search-trigger", id: "cmd-palette-trigger",
            summary { class: "search-summary",
                span { class: "search-icon", "⌕" }
                span { class: "search-label", "search…" }
                span { class: "search-kbd mono-sm", "⌘K" }
            }
        }
    }
}

#[component]
pub fn CommandPalette() -> Element {
    rsx! {
        div { class: "palette-overlay", id: "cmd-palette",
            div { class: "palette-modal",
                PaletteBody {}
            }
        }
    }
}

#[component]
fn PaletteBody() -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());
    let active_encoded = space().map(|s| s.encoded.clone());

    let mut query = use_signal(String::new);

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
                rsx! {
                    ul { class: "palette-list",
                        for h in hits.iter() {
                            {
                                let href = route_for_space_resource(&decoded_space, &h.ref_str);
                                let label = match_field_label(&h.match_field);
                                rsx! {
                                    li { class: "palette-hit", key: "{h.ref_str}",
                                        a { class: "palette-hit-link", href: "{href}",
                                            div { class: "palette-hit-head",
                                                span { class: "palette-hit-kind kind-{h.kind}", "{label}" }
                                                span { class: "palette-hit-title", "{h.title}" }
                                            }
                                            div { class: "palette-hit-loc mono-sm", "{h.locator}" }
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
                oninput: move |e| query.set(e.value()),
            }
            span { class: "palette-close mono-sm", "esc" }
        }
        div { class: "palette-results",
            {palette_content}
        }
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