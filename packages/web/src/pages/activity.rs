//! ActivityPage — the per-space activity stream.
//!
//! Backed by the durable event journal (`event_journal` table):
//! every mutation that goes through the Projector records a Change
//! there, so this page shows real recorded writes (scans, document
//! saves, task transitions, link resolution, …) — never fabricated
//! rows. An empty journal renders an honest empty state, and a
//! readable journal is never disguised as "no activity".

use dioxus::prelude::*;
use ui::notez::NzBadge;

use crate::pages::ui::{Breadcrumb, BreadcrumbSegment};
use crate::pages::use_space_layout;
use crate::router::{route_for_space_files, route_for_space_note};
use crate::server::{list_space_activity, ActivityEntryDto};
use crate::space_ctx::{SpaceState, SpaceStatus};

const ACTIVITY_LIMIT: usize = 100;

/// Render a journal timestamp (`unix millis`) as a compact relative
/// string: "just now", "N min ago", "N hr ago", "N d ago", then a
/// short absolute date ("Aug 27") for older entries.
pub(crate) fn format_relative_time(at_unix_millis: i64, now_unix_millis: i64) -> String {
    let secs = (now_unix_millis - at_unix_millis) / 1000;
    if secs < 60 {
        return "just now".to_string();
    }
    if secs < 3600 {
        return format!("{} min ago", secs / 60);
    }
    if secs < 86_400 {
        return format!("{} hr ago", secs / 3600);
    }
    if secs < 7 * 86_400 {
        return format!("{} d ago", secs / 86_400);
    }
    format_date(at_unix_millis / 1000)
}

/// Civil date ("Aug 27") from a unix timestamp, using the standard
/// days-to-civil algorithm (Hinnant). Kept dependency-free.
fn format_date(unix_secs: i64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = unix_secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as i64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as i64;
    format!("{} {:02}", MONTHS[(m - 1) as usize], d)
}

fn now_unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Last path component of a space root, for compact cross-space rows.
pub(crate) fn space_leaf(source_root: &str) -> String {
    std::path::Path::new(source_root)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| source_root.to_string())
}

#[component]
pub fn ActivityPage(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();
    let mut resource_ref_ctx = use_context::<Signal<Option<String>>>();
    use_effect(move || resource_ref_ctx.set(None));

    let active_path = space().map(|s| s.path.clone());
    let active_encoded = space()
        .map(|s| s.encoded.clone())
        .unwrap_or_else(|| encoded.clone());

    let active_path_for_fetch = active_path.clone();
    let activity = use_server_future(move || {
        let p = active_path_for_fetch.clone();
        async move {
            match p {
                Some(path) => list_space_activity(path, ACTIVITY_LIMIT).await,
                None => Ok(Vec::new()),
            }
        }
    })?;

    let now_ms = now_unix_millis();

    let (eyebrow, h1, lede) = match space() {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => (
            "activity".to_string(),
            format!("{} · activity", s.name),
            format!("the last {ACTIVITY_LIMIT} journal events for this space, newest first."),
        ),
        Some(SpaceState { status: SpaceStatus::Resolving, .. }) => (
            "activity".to_string(),
            "activity · resolving…".to_string(),
            "validating the space root…".to_string(),
        ),
        Some(SpaceState { status: SpaceStatus::Error(e), .. }) => (
            "activity".to_string(),
            "activity".to_string(),
            format!("space error: {e}"),
        ),
        None => (
            "activity".to_string(),
            "activity".to_string(),
            "pick a space from the sidebar to begin.".to_string(),
        ),
    };

    let leaf_for_crumb = active_path
        .as_deref()
        .map(space_leaf)
        .unwrap_or_else(|| "space".to_string());

    rsx! {
        div { class: "page",
            Breadcrumb { segments: vec![
                BreadcrumbSegment::link("notez", "/"),
                BreadcrumbSegment::link(
                    leaf_for_crumb.clone(),
                    active_path
                        .as_ref()
                        .map(|p| route_for_space_files(p))
                        .unwrap_or_else(|| "/".to_string()),
                ),
                BreadcrumbSegment::here("activity".to_string()),
            ] }
            div { class: "page-h",
                p { class: "eyebrow", "{eyebrow}" }
                h1 { "{h1}" }
                p { class: "lede", "{lede}" }
            }

            match activity() {
                None => rsx! { p { class: "results-empty skel", "loading activity…" } },
                Some(Err(e)) => rsx! {
                    div { class: "activity-state",
                        p { class: "activity-state-title", "activity unavailable" }
                        p { class: "activity-state-body",
                            "could not read this space's journal: "
                            span { class: "mono-sm", "{e}" }
                        }
                        p { class: "hint",
                            "run "
                            span { class: "mono-sm", "scan" }
                            " to (re)create the projection, then reload."
                        }
                    }
                },
                Some(Ok(entries)) if entries.is_empty() => rsx! {
                    div { class: "activity-state",
                        p { class: "activity-state-title", "no activity recorded yet" }
                        p { class: "activity-state-body",
                            "This space's journal is empty. Writes that go through the project pipeline — a "
                            span { class: "mono-sm", "scan" }
                            ", a document save, a task transition — will appear here with their actor, source and revision."
                        }
                        p { class: "hint",
                            "run "
                            span { class: "mono-sm", "scan" }
                            " or edit a document to see the first entries."
                        }
                    }
                },
                Some(Ok(entries)) => rsx! {
                    ul { class: "activity-list",
                        for entry in entries.iter() {
                            ActivityRow {
                                entry: entry.clone(),
                                now_ms,
                                current: active_encoded.clone(),
                            }
                        }
                    }
                    div { class: "footer-rule",
                        span { "notez · activity" }
                        span { "·" }
                        span { "journal-backed · newest first" }
                    }
                },
            }
        }
    }
}

#[component]
fn ActivityRow(entry: ActivityEntryDto, now_ms: i64, current: String) -> Element {
    let when = format_relative_time(entry.at_unix_millis, now_ms);
    let change_short: String = entry.change_id.chars().take(8).collect();
    let target_link = entry.target.as_ref().map(|t| {
        let href = route_for_space_note(&entry.source_root, t);
        (t.clone(), href)
    });
    rsx! {
        li { class: "activity-row", key: "{entry.sequence}",
            div { class: "activity-line",
                span { class: "activity-action mono-sm", "{entry.action}" }
                span { class: "activity-actor", "{entry.actor}" }
                span { class: "activity-when mono-sm", "{when}" }
                if entry.audited {
                    NzBadge { text: "audited".to_string(), tone: "ok".to_string() }
                }
                if entry.target_count > 1 {
                    NzBadge { text: format!("{} refs", entry.target_count), tone: "info".to_string() }
                }
            }
            if let Some((t, href)) = target_link {
                div { class: "activity-target mono-sm",
                    span { class: "activity-target-mark", "→" }
                    a { href: "{href}", "{t}" }
                }
            }
            div { class: "activity-meta mono-sm",
                span { "change {change_short}" }
                span { "·" }
                span { "seq {entry.sequence}" }
                if let Some(rev) = entry.revision.as_ref() {
                    Fragment {
                        span { "·" }
                        span { "rev {rev}" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_relative_time_covers_all_bands() {
        let now = 1_700_000_000_000i64;
        assert_eq!(format_relative_time(now - 5_000, now), "just now");
        assert_eq!(format_relative_time(now - 30_000, now), "just now");
        assert_eq!(format_relative_time(now - 5 * 60_000, now), "5 min ago");
        assert_eq!(format_relative_time(now - 3 * 3_600_000, now), "3 hr ago");
        assert_eq!(format_relative_time(now - 2 * 86_400_000, now), "2 d ago");
    }

    #[test]
    fn format_relative_time_falls_back_to_date() {
        // 2023-11-14T22:13:20Z = 1700000000 → "Nov 14".
        let now = 1_700_000_000_000i64;
        assert_eq!(format_relative_time(0, now), "Jan 01");
        assert_eq!(format_relative_time(1_700_000_000_000, 1_800_000_000_000), "Nov 14");
    }

    #[test]
    fn format_relative_time_future_timestamps_are_just_now() {
        assert_eq!(format_relative_time(1_800_000_000_000, 1_700_000_000_000), "just now");
    }

    #[test]
    fn space_leaf_takes_last_component() {
        assert_eq!(space_leaf("/tmp/work space"), "work space");
        assert_eq!(space_leaf("relative/path"), "path");
    }
}
