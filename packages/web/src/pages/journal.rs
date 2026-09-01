//! `JournalPage` — the daily journal (`journal/YYYY-MM-DD.md`).
//!
//! Logseq-style daily notes: the page shows today's entry with
//! open-or-create semantics plus the recent entry list. Creation goes
//! through the plain-HTML POST route `/api/sources/document/create`
//! (progressive enhancement — no hydration involved), which writes
//! the file, indexes it, and redirects to the note page.

use dioxus::prelude::*;
use crate::router::{route_for_space_journal, route_for_space_note, route_for_space_preview};
use crate::pages::use_space_layout;
use crate::server::{list_journal, today_iso_date, today_journal_locator, JournalEntryDto};
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn JournalPage(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space().map(|s| s.path.clone());
    let current_encoded = space().map(|s| s.encoded.clone()).unwrap_or_default();

    let path_for_entries = active_path.clone();
    let entries_resource = use_server_future(move || {
        let p = path_for_entries.clone();
        async move {
            match p {
                Some(p) => list_journal(p).await.unwrap_or_default(),
                None => Vec::new(),
            }
        }
    })?;

    let entries: Vec<JournalEntryDto> = entries_resource.cloned().unwrap_or_default();
    let today = today_iso_date();
    let today_entry = entries.iter().find(|e| e.date == today).cloned();
    let recent: Vec<JournalEntryDto> = entries.into_iter().take(30).collect();

    let space_snapshot = space().clone();
    let source_name = match &space_snapshot {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => s.name.clone(),
        _ => "space".to_string(),
    };
    let space_error = match space_snapshot.as_ref().map(|s| &s.status) {
        Some(SpaceStatus::Error(e)) => Some(e.to_string()),
        _ => None,
    };

    let decoded_space = crate::router::decode_space(&current_encoded);
    let today_locator = today_journal_locator();

    rsx! {
        div { class: "page page-journal",
            header { class: "page-head",
                p { class: "page-eyebrow", "{source_name} · journal" }
                h1 { class: "page-title", "Journal" }
                if let Some(err) = space_error.as_ref() {
                    p { class: "page-lede err-text", "{err}" }
                }
            }

            section { class: "journal-today",
                div { class: "journal-today-head",
                    span { class: "journal-kicker", "today" }
                    span { class: "journal-date mono-sm", "{today}" }
                }
                match today_entry.as_ref() {
                    Some(entry) => rsx! {
                        div { class: "journal-today-body",
                            p { class: "journal-today-title", "{entry.title}" }
                            a { class: "btn btn-accent", href: "{note_href(&decoded_space, entry)}", "Open today's note" }
                        }
                    },
                    None => rsx! {
                        div { class: "journal-today-body",
                            p { class: "journal-today-hint", "No entry for today yet." }
                            form { method: "post", action: "/api/sources/document/create",
                                input { r#type: "hidden", name: "source_root", value: "{decoded_space}" }
                                input { r#type: "hidden", name: "locator", value: "{today_locator}" }
                                button { class: "btn btn-accent", r#type: "submit", "Create today's note" }
                            }
                        }
                    },
                }
            }

            section { class: "journal-list-section",
                h2 { class: "section-title", "Recent entries" }
                if recent.is_empty() {
                    p { class: "empty-hint", "The journal directory is empty. Created entries appear here." }
                } else {
                    ul { class: "journal-list",
                        for entry in recent.iter() {
                            li { class: "journal-row",
                                span { class: "journal-row-date mono-sm", "{entry.date}" }
                                a { class: "journal-row-link", href: "{note_href(&decoded_space, entry)}",
                                    title: "{entry.locator}",
                                    "{entry.title}"
                                }
                            }
                        }
                    }
                }
            }

            details { class: "journal-custom",
                summary { "New entry for another day" }
                form { class: "journal-custom-form", method: "post", action: "/api/sources/document/create",
                    input { r#type: "hidden", name: "source_root", value: "{decoded_space}" }
                    label { class: "field",
                        span { class: "field-label", "Date" }
                        input { r#type: "date", name: "journal_date", required: true }
                    }
                    button { class: "btn", r#type: "submit", "Create" }
                }
            }
        }
    }
}

/// Journal entries link to the note page when indexed; loose files
/// fall back to the preview route.
fn note_href(decoded_space: &str, entry: &JournalEntryDto) -> String {
    if entry.ref_str.is_empty() {
        route_for_space_preview(decoded_space, &entry.locator)
    } else {
        route_for_space_note(decoded_space, &entry.ref_str)
    }
}

/// Today card + a few recent entries — the compact widget reused on
/// the configurable home dashboard.
#[component]
pub fn JournalWidget(decoded_space: String) -> Element {
    let ds = decoded_space.clone();
    let entries_resource = use_server_future(move || {
        let p = ds.clone();
        async move { list_journal(p).await.unwrap_or_default() }
    })?;
    let entries: Vec<JournalEntryDto> = entries_resource.cloned().unwrap_or_default();
    let today = today_iso_date();
    let today_entry = entries.iter().find(|e| e.date == today).cloned();
    let recent: Vec<JournalEntryDto> = entries.into_iter().take(5).collect();

    rsx! {
        section { class: "widget widget-journal",
            div { class: "widget-head",
                h2 { class: "widget-title", "Journal" }
                a { class: "widget-more", href: "{route_for_space_journal(&decoded_space)}", "all →" }
            }
            div { class: "widget-body",
                match today_entry.as_ref() {
                    Some(entry) => rsx! {
                        div { class: "journal-today-body",
                            p { class: "journal-today-title",
                                span { class: "journal-kicker", "today · {today}" }
                                " — {entry.title}"
                            }
                            a { class: "btn btn-accent", href: "{note_href(&decoded_space, entry)}", "Open today's note" }
                        }
                    },
                    None => rsx! {
                        div { class: "journal-today-body",
                            p { class: "journal-today-hint", "No entry for today ({today}) yet." }
                            form { method: "post", action: "/api/sources/document/create",
                                input { r#type: "hidden", name: "source_root", value: "{decoded_space}" }
                                input { r#type: "hidden", name: "journal_date", value: "{today}" }
                                button { class: "btn btn-accent", r#type: "submit", "Create today's note" }
                            }
                        }
                    },
                }
                if !recent.is_empty() {
                    ul { class: "journal-list journal-list-compact",
                        for entry in recent.iter() {
                            li { class: "journal-row",
                                span { class: "journal-row-date mono-sm", "{entry.date}" }
                                a { class: "journal-row-link", href: "{note_href(&decoded_space, entry)}", "{entry.title}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
