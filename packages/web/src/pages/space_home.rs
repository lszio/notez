//! `SpaceHome` — the per-space welcome at `/space/:encoded`.
//!
//! The page is a single round-trip dashboard:
//!
//! - **`load_index_document(space_root)`** returns both the index
//!   entry (`index.org` / `index.md` / `README.*`) and the rendered
//!   document body in one server call. Earlier code fired two
//!   independent `use_server_future`s, and the document future ran
//!   before the entry future had resolved — so the body never
//!   appeared when an index existed. The single-call API eliminates
//!   that race.
//! - **`list_filesystem(space_root)`** returns the loose-files
//!   listing for the dashboard "recently modified" section.
//!
//! Both round trips suspend on SSR; the page renders only after both
//! are ready so the dashboard never shows a half-empty state.

use dioxus::prelude::*;

use crate::pages::ui::KindIcon;
use crate::pages::use_space_layout;
use crate::router::{route_for_space_list, route_for_space_preview, route_for_space_resource};
use crate::server::{list_filesystem, load_index_document, IndexDocumentDto};
use crate::space_ctx::{SpaceState, SpaceStatus};

/// Cap on the dashboard file list. The full list is reachable via
/// the "open full list" link.
const DASHBOARD_FILES_LIMIT: usize = 8;

#[component]
pub fn SpaceHome(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space().map(|s| s.path.clone());
    let current_encoded = space().map(|s| s.encoded.clone()).unwrap_or_default();

    // Single combined call: entry + document together. See module
    // docs for why this is one future instead of two.
    let path_for_index = active_path.clone();
    let index_resource = use_server_future(move || {
        let p = path_for_index.clone();
        async move {
            match p {
                Some(p) => load_index_document(p).await,
                None => Ok(IndexDocumentDto {
                    entry: None,
                    document: None,
                }),
            }
        }
    })?;

    let path_for_files = active_path.clone();
    let files_resource = use_server_future(move || {
        let p = path_for_files.clone();
        async move {
            match p {
                Some(p) => list_filesystem(p).await,
                None => Ok(Vec::new()),
            }
        }
    })?;

    let index_doc: Option<IndexDocumentDto> = index_resource.cloned().and_then(|r| r.ok());
    let files = files_resource.cloned().and_then(|r| r.ok()).unwrap_or_default();

    let space_snapshot = space().clone();
    let space_name = match &space_snapshot {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => s.name.clone(),
        _ => "space".to_string(),
    };
    let space_resolving = matches!(
        space_snapshot.as_ref().map(|s| &s.status),
        Some(SpaceStatus::Resolving)
    );
    let space_error = match space_snapshot.as_ref().map(|s| &s.status) {
        Some(SpaceStatus::Error(e)) => Some(e.to_string()),
        _ => None,
    };
    let no_space = matches!(space_snapshot, None);

    let index_entry = index_doc.as_ref().and_then(|d| d.entry.clone());
    let rendered_doc = index_doc.and_then(|d| d.document);

    let dashboard_files: Vec<_> = files.iter().take(DASHBOARD_FILES_LIMIT).collect();
    let has_index = index_entry.is_some();
    let has_rendered_body = rendered_doc
    .as_ref()
    .map(|d| !d.body_html.is_empty())
    .unwrap_or(false);

    let decoded = crate::router::decode_space(&current_encoded);
    let active_path_for_list = active_path.clone().unwrap_or_default();
    let active_path_for_watch = active_path.clone().unwrap_or_default();
    let active_path_for_scan = active_path.clone().unwrap_or_default();

    let (eyebrow, h1, lede) = if space_resolving {
        (
            "resolving".to_string(),
            "Welcome".to_string(),
            "looking up the space…".to_string(),
        )
    } else if let Some(err) = space_error.as_ref() {
        (
            "error".to_string(),
            "Welcome".to_string(),
            format!("space error: {err}"),
        )
    } else if no_space {
        (
            "no space".to_string(),
            "Welcome".to_string(),
            "pick a space to begin.".to_string(),
        )
    } else if let Some(entry) = index_entry.as_ref() {
        (
            format!("space · {space_name}"),
            entry.title.clone(),
            format!(
                "{count} file{s} on disk · rendered from {loc}.",
                count = files.len(),
                s = if files.len() == 1 { "" } else { "s" },
                loc = entry.locator,
            ),
        )
    } else {
        (
            format!("space · {space_name}"),
            "No index document".to_string(),
            format!(
                "{count} file{s} on disk · create `index.org` or `README.md` to set a landing page.",
                count = files.len(),
                s = if files.len() == 1 { "" } else { "s" },
            ),
        )
    };

    rsx! {
        div { class: "page",
            div { class: "page-h",
                p { class: "eyebrow", "{eyebrow}" }
                h1 { "{h1}" }
                p { class: "lede", "{lede}" }
                // Dashboard actions: list / rescan / watch toggle.
                // These are plain HTML forms so they work without
                // WASM hydration (the rest of the site is SSR-only).
                div { class: "dashboard-actions",
                    a {
                        class: "spine-action",
                        href: "{route_for_space_list(&active_path_for_list)}",
                        "open full list →"
                    }
                    form {
                        class: "dashboard-form",
                        method: "post",
                        action: "/api/spaces/scan",
                        input { type: "hidden", name: "space_root", value: "{active_path_for_scan}" }
                        button { class: "spine-action", r#type: "submit", "force rescan" }
                    }
                    form {
                        class: "dashboard-form",
                        method: "post",
                        action: "/api/spaces/watch/start",
                        input { type: "hidden", name: "space_root", value: "{active_path_for_watch}" }
                        button { class: "spine-action", r#type: "submit", "watch this space" }
                    }
                }
            }

            if !has_index {
                // No-index CTA. The dashboard shows the user exactly
                // what to do next: drop an `index.org` at the space
                // root and rescan. The recently-modified list below
                // is already populated from `list_filesystem`, so
                // users can still navigate.
                div { class: "welcome-empty",
                    h2 { "Set a landing page" }
                    p { class: "lede dim",
                        "Notez renders `index.org` (or `index.md` / `README.*`) at the root of every space as the welcome page. Drop one in and run \"force rescan\" to pick it up."
                    }
                    ul { class: "welcome-empty-hints",
                        li { "Title becomes the page heading (the first `*` heading)." }
                        li { "Anything you link via `[[id:…]]` becomes a backlink." }
                        li { "Org-mode `#+BEGIN_SRC query` blocks render as query embeds." }
                    }
                }
            }

            if let (Some(_entry), Some(doc)) = (index_entry.as_ref(), rendered_doc.as_ref()) {
                div { class: "welcome-index",
                    div { class: "welcome-index-meta mono-sm",
                        KindIcon { kind: doc.kind.clone() }
                        a { href: "{route_for_space_resource(&decoded, &doc.ref_str)}", "{doc.ref_str}" }
                        span { class: "dim", " · {doc.locator}" }
                    }
                    if has_rendered_body {
                        div { class: "detail-body",
                            div { dangerous_inner_html: "{doc.body_html}" }
                        }
                    } else {
                        p { class: "props-empty",
                            "Index document is empty — add a `* heading` line and rescan."
                        }
                    }
                }
            }

            div { class: "welcome-files",
                div { class: "welcome-files-head",
                    span { class: "welcome-files-label", if has_index { "files on disk" } else { "recently modified" } }
                    a { class: "welcome-files-link mono-sm", href: "{route_for_space_list(&active_path_for_list)}", "open full list →" }
                }
                if dashboard_files.is_empty() {
                    p { class: "welcome-files-empty", "no files yet." }
                } else {
                    ul { class: "welcome-files-list",
                        for f in dashboard_files.iter() {
                            {
                                let is_indexed = !f.ref_str.is_empty();
                                let href = if is_indexed {
                                    route_for_space_resource(&decoded, &f.ref_str)
                                } else {
                                    route_for_space_preview(&decoded, &f.display_path)
                                };
                                let label = if f.ext.is_empty() { f.kind.to_uppercase() } else { f.ext.to_uppercase() };
                                let mtime_label = format_mtime(f.mtime_ms);
                                rsx! {
                                    li { class: "welcome-files-row", key: "{f.display_path}",
                                        a { class: "welcome-files-link2", href: "{href}",
                                            span { class: "welcome-files-ext kind-{f.kind}", "{label}" }
                                            span { class: "welcome-files-name", "{f.display_path}" }
                                            span { class: "welcome-files-title dim", "{f.title}" }
                                            if !mtime_label.is_empty() {
                                                span { class: "welcome-files-mtime mono-sm dim", "{mtime_label}" }
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

/// Format a mtime as a short relative label: "just now", "5m ago",
/// "2h ago", "3d ago", "Jan 2". Returns "" when mtime_ms is 0.
fn format_mtime(mtime_ms: u64) -> String {
    if mtime_ms == 0 {
        return String::new();
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if now_ms <= mtime_ms {
        return "just now".into();
    }
    let delta_ms = now_ms - mtime_ms;
    let sec = delta_ms / 1000;
    if sec < 60 {
        return format!("{sec}s ago");
    }
    let min = sec / 60;
    if min < 60 {
        return format!("{min}m ago");
    }
    let hr = min / 60;
    if hr < 24 {
        return format!("{hr}h ago");
    }
    let day = hr / 24;
    if day < 14 {
        return format!("{day}d ago");
    }
    // Older than 14 days: fall back to month-day.
    let secs = mtime_ms / 1000;
    let days = (secs / 86400) as i64;
    let (_y, m, d) = civil_from_days(days);
    format!("{} {}", month_short(m), d)
}

fn month_short(m: i64) -> &'static str {
    match m {
        1 => "Jan", 2 => "Feb", 3 => "Mar", 4 => "Apr", 5 => "May", 6 => "Jun",
        7 => "Jul", 8 => "Aug", 9 => "Sep", 10 => "Oct", 11 => "Nov", 12 => "Dec",
        _ => "?",
    }
}

/// Convert an absolute day count (days since 1970-01-01) to
/// (year, month, day). Howard Hinnant's algorithm from
/// <http://howardhinnant.github.io/date_algorithms.html>.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as i64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as i64;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}