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
        div { class: "page",
            section { class: "onboard",
                h1 { "Notez" }
                p { class: "lede",
                    "a local-first, Org-mode-native knowledge federation engine."
                }
                p { class: "lede dim",
                    "Files on disk are authoritative: every doc, heading, attachment and block lives in the original .org / .md files; the SQLite projection is rebuilt with `notez scan`."
                }
            }

            div { class: "home-grid",
                // ---- Source health (real data) ----
                section { class: "home-card", "aria-labelledby": "health-heading",
                    div { class: "home-card-head",
                        h2 { id: "health-heading", "Source health" }
                        span { class: "home-card-count mono-sm", "{ready_count} ready · {broken_count} problem" }
                    }
                    if rows.is_empty() {
                        NzCard {
                            padded: true,
                            p { class: "home-empty",
                                "No sources registered yet."
                            }
                            p { class: "lede dim",
                                "Use the register form in the space dropdown (top-left) to point Notez at a local folder, or run "
                                span { class: "mono-sm", "notez source add" }
                                " from the CLI. Once registered, a space shows its health here."
                            }
                        }
                    } else {
                        ul { class: "health-list",
                            for row in rows.iter() {
                                {
                                    match row {
                                        SpaceHealth::Ready { dto, space_name, total } => rsx! {
                                            li { class: "health-row", key: "{dto.path}",
                                                NzCard {
                                                    padded: true,
                                                    div { class: "health-row-body",
                                                        div { class: "health-row-main",
                                                            a { class: "health-row-name", href: "{route_for_space_list(&dto.path)}", "{space_name}" }
                                                            if dto.source == "discovered" {
                                                                NzBadge { text: "found".to_string(), tone: "info".to_string() }
                                                            }
                                                            div { class: "health-row-path mono-sm", "{dto.path}" }
                                                        }
                                                        div { class: "health-row-side",
                                                            NzBadge { text: "ready".to_string(), tone: "ok".to_string() }
                                                            span { class: "health-row-total mono-sm", "{total} resources" }
                                                            a { class: "spine-action", href: "{route_for_space_list(&dto.path)}", "open →" }
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                        SpaceHealth::Broken { dto, message } => rsx! {
                                            li { class: "health-row", key: "{dto.path}",
                                                NzCard {
                                                    padded: true,
                                                    div { class: "health-row-body",
                                                        div { class: "health-row-main",
                                                            span { class: "health-row-name", "{dto.name}" }
                                                            div { class: "health-row-path mono-sm", "{dto.path}" }
                                                        }
                                                        div { class: "health-row-side",
                                                            NzBadge { text: "unavailable".to_string(), tone: "err".to_string() }
                                                            span { class: "health-row-err mono-sm", "{message}" }
                                                            a { class: "spine-action", href: "/", "re-register" }
                                                        }
                                                    }
                                                }
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }

                // ---- Activity (journal-backed) ----
                section { class: "home-card", "aria-labelledby": "activity-heading",
                    div { class: "home-card-head",
                        h2 { id: "activity-heading", "Activity" }
                        span { class: "home-card-count mono-sm",
                            if dashboard_loading {
                                "…"
                            } else {
                                "{recent_activity.len()} recent"
                            }
                        }
                    }
                    if dashboard_loading {
                        NzCard {
                            padded: true,
                            p { class: "home-empty", "loading activity…" }
                        }
                    } else if recent_activity.is_empty() {
                        NzCard {
                            padded: true,
                            p { class: "home-empty",
                                "No activity recorded yet — every space's journal is empty."
                            }
                            ul { class: "home-steps",
                                li { "Run " span { class: "mono-sm", "scan" } " or save a document; every journaled write appears here with its actor, source and revision." }
                                li { "Open a space and follow the " span { class: "mono-sm", "activity" } " link to see the full stream." }
                            }
                        }
                    } else {
                        NzCard {
                            padded: true,
                            ul { class: "activity-list activity-list-compact",
                                for entry in recent_activity.iter() {
                                    HomeActivityRow { entry: entry.clone(), now_ms }
                                }
                            }
                            div { class: "home-card-foot",
                                if let Some(href) = first_activity_href.as_ref() {
                                    a { class: "spine-action", href: "{href}", "open the activity stream →" }
                                }
                                span { class: "home-card-note mono-sm", "journal-backed · real recorded writes" }
                            }
                        }
                    }
                }

                // ---- Continue working ----
                section { class: "home-card", "aria-labelledby": "continue-heading",
                    div { class: "home-card-head",
                        h2 { id: "continue-heading", "Continue working" }
                    }
                    NzCard {
                        padded: true,
                        p { class: "home-empty",
                            "Recent documents appear in the activity card above. Saved views have no server surface in this build — the list page keeps its view state in the URL, clearly labelled as unsaved."
                        }
                        div { class: "onboard-actions",
                            a { class: "spine-action", href: "/", "search with ⌘K" }
                            if let Some(href) = first_ready_href.as_ref() {
                                a { class: "spine-action", href: "{href}", "open first healthy space" }
                            }
                        }
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
