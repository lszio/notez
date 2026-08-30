//! Landing page at `/` — Source health, Activity and Continue-working
//! affordances per `docs/ui-refactoring-v1.org` §4.1.
//!
//! Data honesty: Source health is real (every registered space is
//! resolved server-side via `selected_space` + `list_kind_counts`),
//! and the Activity card is backed by the durable journal (real
//! recorded writes, never fabricated rows; empty = genuinely no
//! journal entries yet). Saved views still have no server surface,
//! so the Continue card labels that state explicitly.

use dioxus::prelude::*;
use ui::notez::{NzBadge, NzCard};

use crate::pages::activity::{format_relative_time, space_leaf};
use crate::router::{route_for_space_activity, route_for_space_list, route_for_space_resource};
use crate::server::{
    list_kind_counts, list_recent_activity, list_registered_spaces, selected_space,
    ActivityEntryDto, RegisteredSpaceDto,
};
use crate::tree::KindCounts;

/// One row of the Source-health dashboard.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum SpaceHealth {
    Ready {
        dto: RegisteredSpaceDto,
        space_name: String,
        total: usize,
    },
    Broken { dto: RegisteredSpaceDto, message: String },
}

#[component]
pub fn HomePage() -> Element {
    // One future for the whole dashboard so SSR renders one
    // deterministic payload (no multi-future races): health rows +
    // the merged recent activity stream.
    let health_resource = use_server_future(|| async {
        let mut rows: Vec<SpaceHealth> = Vec::new();
        let spaces = list_registered_spaces().await.unwrap_or_default();
        for s in spaces {
            match selected_space(s.path.clone()).await {
                Ok(sel) => {
                    let counts: KindCounts =
                        list_kind_counts(s.path.clone()).await.unwrap_or_default();
                    rows.push(SpaceHealth::Ready {
                        dto: s,
                        space_name: sel.name,
                        total: counts.total(),
                    });
                }
                Err(e) => rows.push(SpaceHealth::Broken {
                    dto: s,
                    message: e.to_string(),
                }),
            }
        }
        let activity = list_recent_activity(12).await.unwrap_or_default();
        (rows, activity)
    })?;

    let (rows, recent_activity): (Vec<SpaceHealth>, Vec<ActivityEntryDto>) =
        health_resource.cloned().unwrap_or_default();
    let dashboard_loading = health_resource().is_none();
    let ready_count = rows
        .iter()
        .filter(|r| matches!(r, SpaceHealth::Ready { .. }))
        .count();
    let broken_count = rows.len() - ready_count;
    let first_ready_path = rows.iter().find_map(|r| match r {
        SpaceHealth::Ready { dto, .. } => Some(dto.path.clone()),
        _ => None,
    });
    let first_ready_href = first_ready_path.as_ref().map(|p| route_for_space_list(p));
    let first_activity_href = first_ready_path.as_ref().map(|p| route_for_space_activity(p));
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    rsx! {
        div { class: "page workspace-home",
            section { class: "workspace-hero",
                div { class: "workspace-hero-kicker", "WORKSPACE OVERVIEW" }
                h1 { "Good to see you." }
                p { class: "lede", "A quiet place for local documents, connected sources, and the work waiting next." }
                div { class: "workspace-hero-actions",
                    button { class: "hero-command", r#type: "button", "data-palette-open": "true", "⌘K", span { "Search or run a command" } }
                    if let Some(href) = first_ready_href.as_ref() { a { class: "hero-link", href: "{href}", "Open documents →" } }
                }
            }
            div { class: "workspace-columns",
                section { class: "workspace-section workspace-continue", "aria-labelledby": "continue-heading",
                    div { class: "workspace-section-head", span { class: "workspace-section-kicker", "CONTINUE" }, h2 { id: "continue-heading", "Pick up where you left off" } }
                    if dashboard_loading { div { class: "workspace-empty", "Loading recent work…" } }
                    else if recent_activity.is_empty() { div { class: "workspace-empty", p { "No recent documents yet." } p { class: "dim", "Open a source or search the workspace to begin." } } }
                    else { div { class: "continue-list", for entry in recent_activity.iter().take(5) { HomeActivityRow { entry: entry.clone(), now_ms } } } }
                }
                section { class: "workspace-section workspace-sources", "aria-labelledby": "sources-heading",
                    div { class: "workspace-section-head", span { class: "workspace-section-kicker", "SOURCES" }, h2 { id: "sources-heading", "Your connected spaces" } }
                    if rows.is_empty() { div { class: "workspace-empty", p { "No sources registered." } p { class: "dim", "Register a local workspace from the source menu above." } } }
                    else { div { class: "source-list", for row in rows.iter() { match row {
                        SpaceHealth::Ready { dto, space_name, total } => rsx! { a { class: "source-card", href: "{route_for_space_list(&dto.path)}", div { class: "source-card-top", span { class: "source-dot", "●" } strong { "{space_name}" } span { class: "source-state", "ready" } }, div { class: "source-card-meta mono-sm", "{total} resources · {dto.path}" } } },
                        SpaceHealth::Broken { dto, message } => rsx! { div { class: "source-card is-broken", div { class: "source-card-top", span { class: "source-dot", "!" } strong { "{dto.name}" } span { class: "source-state", "unavailable" } }, div { class: "source-card-meta mono-sm", "{message}" } } },
                    } } } }
                }
            }
            section { class: "workspace-section workspace-activity", "aria-labelledby": "activity-heading",
                div { class: "workspace-section-head", span { class: "workspace-section-kicker", "ACTIVITY" }, h2 { id: "activity-heading", "Recent changes" }, if let Some(href) = first_activity_href.as_ref() { a { class: "section-action", href: "{href}", "View all →" } } }
                if recent_activity.is_empty() { div { class: "workspace-empty", "Journal activity will appear here after a scan or document save." } }
                else { div { class: "activity-stream", for entry in recent_activity.iter() { HomeActivityRow { entry: entry.clone(), now_ms } } } }
            }
        }
    }
}

/// Compact activity row for the home dashboard. Rows can come from
/// different spaces, so the space leaf is shown alongside the action.
#[component]
fn HomeActivityRow(entry: ActivityEntryDto, now_ms: i64) -> Element {
    let when = format_relative_time(entry.at_unix_millis, now_ms);
    let leaf = space_leaf(&entry.source_root);
    let target_link = entry.target.as_ref().map(|t| {
        let href = route_for_space_resource(&entry.source_root, t);
        (t.clone(), href)
    });
    rsx! {
        li { class: "activity-row", key: "{entry.sequence}-{entry.source_root}",
            div { class: "activity-line",
                span { class: "activity-action mono-sm", "{entry.action}" }
                span { class: "activity-actor", "{entry.actor}" }
                span { class: "activity-when mono-sm", "{when}" }
                span { class: "activity-space mono-sm", "{leaf}" }
            }
            if let Some((t, href)) = target_link {
                div { class: "activity-target mono-sm",
                    span { class: "activity-target-mark", "→" }
                    a { href: "{href}", "{t}" }
                }
            }
        }
    }
}
