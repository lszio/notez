//! Landing page at `/` — the configurable home dashboard.
//!
//! Widgets render in the order stored in `~/.config/notez/web.toml`
//! (`home_widgets` + `hidden_home_widgets`; see `crate::ui_config`).
//! The built-in widget set:
//!
//! - `journal`  — today's entry (open-or-create) + recent entries for
//!   the primary space
//! - `recent`   — recently modified files in the primary space
//! - `activity` — merged cross-space journal stream
//! - `spaces`   — registered sources
//! - `graph`    — mini graph of the primary space
//!
//! The "Customize" panel is plain HTML forms posting to
//! `/api/web/widgets` (`up` / `down` / `hide` / `show`) — no
//! hydration involved. All widget data is real: journal entries come
//! from the filesystem, activity from the durable journal, files from
//! the disk listing.

use dioxus::prelude::*;

use crate::pages::activity::{format_relative_time, space_leaf};
use crate::pages::journal::JournalWidget;
use crate::pages::panel_files::format_mtime;
use crate::server::{
    list_recent_activity, list_registered_spaces, list_source_files, ActivityEntryDto,
    RegisteredSpaceDto,
};
use crate::ui_config::WIDGET_IDS;

/// Primary space = the first registered source. Used by widgets that
/// need a concrete space when the user has not picked one.
fn primary_space(spaces: &[RegisteredSpaceDto]) -> Option<RegisteredSpaceDto> {
    spaces.first().cloned()
}

#[component]
pub fn HomePage() -> Element {
    let spaces_resource = use_server_future(|| async {
        list_registered_spaces().await.unwrap_or_default()
    })?;
    let spaces: Vec<RegisteredSpaceDto> = spaces_resource.cloned().unwrap_or_default();
    let widgets_resource = use_server_future(|| async {
        crate::server::get_home_widgets().await.unwrap_or_default()
    })?;
    let widgets: Vec<String> = widgets_resource.cloned().unwrap_or_default();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    rsx! {
        div { class: "page page-home",
            header { class: "page-head",
                p { class: "page-eyebrow", "notez" }
                h1 { class: "page-title", "Home" }
                p { class: "page-lede",
                    if spaces.is_empty() {
                        "Register a source to start writing notes."
                    } else {
                        "{spaces.len()} source(s) registered. Widgets below are configurable."
                    }
                }
            }

            CustomizePanel { widgets: widgets.clone() }

            if widgets.is_empty() {
                p { class: "empty-hint", "All widgets are hidden. Use Customize to bring some back." }
            } else {
                div { class: "widget-grid",
                    for id in widgets.iter() {
                        {match id.as_str() {
                            "journal" => rsx! { JournalSection { spaces: spaces.clone() } },
                            "recent" => rsx! { RecentSection { spaces: spaces.clone() } },
                            "activity" => rsx! { ActivitySection { now_ms } },
                            "spaces" => rsx! { SpacesSection { spaces: spaces.clone() } },
                            "graph" => rsx! { GraphSection { spaces: spaces.clone() } },
                            _ => rsx! {},
                        }}
                    }
                }
            }
            NotezCardsSection { spaces: spaces.clone() }
        }
    }
}

/// Document-driven cards: project every `notez` card defined in the
/// primary space's files, executed through the same executor the
/// body renderer uses. Bounded and source-policy-filtered upstream.
#[component]
fn NotezCardsSection(spaces: Vec<RegisteredSpaceDto>) -> Element {
    let cards_resource = use_server_future(|| async {
        crate::server::list_dashboard_cards().await.unwrap_or_default()
    })?;
    let cards: Vec<crate::server::NotezCardView> = cards_resource.cloned().unwrap_or_default();
    let primary = primary_space(&spaces);
    if cards.is_empty() {
        return rsx! {
            section { class: "widget widget-notez-cards",
                div { class: "widget-head",
                    h2 { class: "widget-title", "Cards" }
                }
                div { class: "widget-body",
                    if primary.is_some() {
                        p { class: "empty-hint",
                            "No `notez` cards in the primary space yet. Add one as a fenced Markdown block: "
                        }
                        pre { class: "card-hint", "`notez kind=card id=inbox output=list`\\n(notez/query ctx ...) `\"" }
                    } else {
                        p { class: "empty-hint", "Register a source to enable notez cards." }
                    }
                }
            }
        };
    }
    rsx! {
        section { class: "widget widget-notez-cards",
            div { class: "widget-head",
                h2 { class: "widget-title", "Cards" }
                span { class: "widget-sub", "{cards.len()} from this source" }
            }
            div { class: "notez-card-grid",
                for card in cards.iter() {
                    NotezCardView { card: card.clone() }
                }
            }
        }
    }
}

#[component]
fn NotezCardView(card: crate::server::NotezCardView) -> Element {
    let state_class = format!("notez-card notez-card--{}", card.state);
    let kind_class = format!("notez-card-error--{}", card.error_kind.clone().unwrap_or_default());
    let tip = format!("card-id={}; state={}; output={}", card.id, card.state, card.output_type);
    rsx! {
        article { class: "{state_class}", aria_label: "{tip}",
            header { class: "notez-card-head",
                h3 { class: "notez-card-title", "{card.title}" }
                small { class: "notez-card-source", "{card.locator}" }
            }
            div { class: "notez-card-body",
                if card.state == "failed" {
                    pre { class: "{kind_class}",
                        "{card.error.clone().unwrap_or_default()}" }
                } else if card.output_type == "json" {
                    pre { class: "notez-card-json", "{card.value.clone().map(|v|v.to_string()).unwrap_or_default()}" }
                } else if card.output_type == "list" {
                    ul { class: "notez-card-list",
                        for item in card.items.iter() {
                            li { "{item.to_string()}" }
                        }
                    }
                } else if card.output_type == "object" {
                    code { class: "notez-card-ref", "{card.object_ref.clone().unwrap_or_default()}" }
                }
            }
        }
    }
}

/// Widget visibility editor. `widgets` = visible ids in display
/// order; the hidden rest of `WIDGET_IDS` renders show buttons.
#[component]
fn CustomizePanel(widgets: Vec<String>) -> Element {
    let hidden_ids: Vec<&str> = WIDGET_IDS
        .iter()
        .copied()
        .filter(|id| !widgets.iter().any(|w| w == id))
        .collect();

    rsx! {
        details { class: "customize",
            summary { "Customize home" }
            div { class: "customize-body",
                div { class: "customize-col",
                    h3 { class: "customize-title", "Visible" }
                    if widgets.is_empty() {
                        p { class: "empty-hint", "none" }
                    }
                    ul { class: "customize-list",
                        for (idx, w) in widgets.iter().enumerate() {
                            li { class: "customize-row",
                                span { class: "customize-name", "{widget_label(w)}" }
                                div { class: "customize-actions",
                                    form { method: "post", action: "/api/web/widgets",
                                        input { r#type: "hidden", name: "widget", value: "{w}" }
                                        input { r#type: "hidden", name: "op", value: "up" }
                                        button { class: "mini-btn", r#type: "submit", disabled: idx == 0, "aria-label": "Move up", "↑" }
                                    }
                                    form { method: "post", action: "/api/web/widgets",
                                        input { r#type: "hidden", name: "widget", value: "{w}" }
                                        input { r#type: "hidden", name: "op", value: "down" }
                                        button { class: "mini-btn", r#type: "submit", disabled: idx == widgets.len() - 1, "aria-label": "Move down", "↓" }
                                    }
                                    form { method: "post", action: "/api/web/widgets",
                                        input { r#type: "hidden", name: "widget", value: "{w}" }
                                        input { r#type: "hidden", name: "op", value: "hide" }
                                        button { class: "mini-btn mini-btn-warn", r#type: "submit", "aria-label": "Hide widget", "hide" }
                                    }
                                }
                            }
                        }
                    }
                }
                if !hidden_ids.is_empty() {
                    div { class: "customize-col",
                        h3 { class: "customize-title", "Hidden" }
                        ul { class: "customize-list",
                            for w in hidden_ids.iter() {
                                li { class: "customize-row",
                                    span { class: "customize-name dim", "{widget_label(w)}" }
                                    form { method: "post", action: "/api/web/widgets",
                                        input { r#type: "hidden", name: "widget", value: "{w}" }
                                        input { r#type: "hidden", name: "op", value: "show" }
                                        button { class: "mini-btn", r#type: "submit", "show" }
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

fn widget_label(id: &str) -> &'static str {
    match id {
        "journal" => "Journal",
        "recent" => "Recent files",
        "activity" => "Activity",
        "spaces" => "Sources",
        "graph" => "Graph",
        _ => "Widget",
    }
}

#[component]
fn JournalSection(spaces: Vec<RegisteredSpaceDto>) -> Element {
    match primary_space(&spaces) {
        Some(s) => rsx! { JournalWidget { decoded_space: s.path.clone() } },
        None => rsx! { WidgetEmpty { title: "Journal", hint: "Register a source first." } },
    }
}

#[component]
fn RecentSection(spaces: Vec<RegisteredSpaceDto>) -> Element {
    match primary_space(&spaces) {
        Some(s) => rsx! { RecentWidget { decoded_space: s.path } },
        None => rsx! { WidgetEmpty { title: "Recent files", hint: "Register a source first." } },
    }
}

#[component]
fn GraphSection(spaces: Vec<RegisteredSpaceDto>) -> Element {
    match primary_space(&spaces) {
        Some(s) => rsx! { GraphWidget { decoded_space: s.path } },
        None => rsx! { WidgetEmpty { title: "Graph", hint: "Register a source first." } },
    }
}

#[component]
fn WidgetEmpty(title: String, hint: String) -> Element {
    rsx! {
        section { class: "widget",
            div { class: "widget-head", h2 { class: "widget-title", "{title}" } }
            div { class: "widget-body", p { class: "empty-hint", "{hint}" } }
        }
    }
}

/// Recently modified files in the primary space (mtime desc).
#[component]
fn RecentWidget(decoded_space: String) -> Element {
    let ds = decoded_space.clone();
    let files_resource = use_server_future(move || {
        let p = ds.clone();
        async move {
            let mut files = list_source_files(p).await.unwrap_or_default();
            files.sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms));
            files.truncate(10);
            files
        }
    })?;
    let files = files_resource.cloned().unwrap_or_default();

    rsx! {
        section { class: "widget widget-recent",
            div { class: "widget-head",
                h2 { class: "widget-title", "Recent files" }
                a { class: "widget-more", href: "{crate::router::route_for_space_files(&decoded_space)}", "all →" }
            }
            div { class: "widget-body",
                if files.is_empty() {
                    p { class: "empty-hint", "No files indexed yet." }
                } else {
                    ul { class: "recent-list",
                        for f in files.iter() {
                            li { class: "recent-row",
                                a {
                                    class: "recent-link",
                                    href: if f.ref_str.is_empty() {
                                        crate::router::route_for_space_preview(&decoded_space, &f.display_path)
                                    } else {
                                        crate::router::route_for_space_note(&decoded_space, &f.ref_str)
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
    }
}

/// Merged cross-space activity stream.
#[component]
fn ActivitySection(now_ms: i64) -> Element {
    let activity_resource = use_server_future(|| async {
        list_recent_activity(10).await.unwrap_or_default()
    })?;
    let entries: Vec<ActivityEntryDto> = activity_resource.cloned().unwrap_or_default();

    rsx! {
        section { class: "widget widget-activity",
            div { class: "widget-head", h2 { class: "widget-title", "Activity" } }
            div { class: "widget-body",
                if entries.is_empty() {
                    p { class: "empty-hint", "No writes recorded yet — edits land here." }
                } else {
                    ul { class: "activity-list",
                        for entry in entries.iter() {
                            li { class: "activity-row", HomeActivityRow { entry: entry.clone(), now_ms } }
                        }
                    }
                }
            }
        }
    }
}

/// Registered sources.
#[component]
fn SpacesSection(spaces: Vec<RegisteredSpaceDto>) -> Element {
    rsx! {
        section { class: "widget widget-spaces",
            div { class: "widget-head", h2 { class: "widget-title", "Sources" } }
            div { class: "widget-body",
                if spaces.is_empty() {
                    p { class: "empty-hint", "No sources registered yet." }
                } else {
                    ul { class: "space-list",
                        for s in spaces.iter() {
                            li { class: "space-row",
                                a { class: "space-link", href: "{crate::router::route_for_space_home(&s.path)}",
                                    span { class: "space-name", "{s.name}" }
                                    span { class: "space-path dim mono-sm", "{s.path}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Mini force-directed graph of the primary space.
#[component]
fn GraphWidget(decoded_space: String) -> Element {
    let ds = decoded_space.clone();
    let graph_resource = use_server_future(move || {
        let p = ds.clone();
        async move { crate::server::list_graph(p).await.ok() }
    })?;
    let graph = graph_resource
        .cloned()
        .flatten()
        .unwrap_or_else(|| notez_core::application::Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            total_nodes: 0,
            truncated: false,
        });
    rsx! {
        section { class: "widget widget-graph",
            div { class: "widget-head",
                h2 { class: "widget-title", "Graph" }
                a { class: "widget-more", href: "{crate::router::route_for_space_graph(&decoded_space)}", "full →" }
            }
            div { class: "widget-body widget-body-graph",
                if graph.nodes.is_empty() {
                    p { class: "empty-hint", "No linked notes yet." }
                } else {
                    crate::pages::graph::GraphSvg {
                        graph,
                        space_decoded: decoded_space.clone(),
                        width: 640.0,
                        height: 320.0,
                        iterations: 80,
                    }
                }
            }
        }
    }
}

/// Compact activity row for the home dashboard. Rows can come from
/// different spaces, so the space leaf is shown alongside the action.
#[component]
fn HomeActivityRow(entry: ActivityEntryDto, now_ms: i64) -> Element {
    let href = if entry.target.is_some() {
        crate::router::route_for_space_note(&entry.source_root, entry.target.as_deref().unwrap_or(""))
    } else {
        crate::router::route_for_space_activity(&entry.source_root)
    };
    rsx! {
        div { class: "activity-inner",
            span { class: "activity-action", "{entry.action}" }
            if let Some(t) = entry.target.as_ref() {
                span { class: "activity-target mono-sm", "{space_leaf(t)}" }
            }
            span { class: "activity-space dim", "{space_leaf(&entry.source_root)}" }
            span { class: "activity-when dim", "{format_relative_time(entry.at_unix_millis, now_ms)}" }
            a { class: "activity-link", href: "{href}", "view" }
        }
    }
}
