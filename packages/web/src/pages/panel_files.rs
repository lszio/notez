//! `FilesPanel` — the bottom half of the left column.
//!
//! Lists every file on disk under the active space (not just the
//! resources the projection has indexed). Clicking a row navigates
//! to the in-app preview page (`/space/:enc/preview/:locator`) so
//! PDFs, images, and other loose attachments get a proper
//! inline preview the moment they drop in, even before the
//! projection has scanned the space.

use dioxus::prelude::*;

use crate::router::{route_for_space_preview, route_for_space_resource, ListQuery};
use crate::server::list_filesystem;
use crate::space_ctx::{SpaceState, SpaceStatus};
use crate::tree::SourceFileRow;

/// Ordered list of kind pills shown at the top of the panel. The
/// order is editorial: document formats first, then rich-attachment
/// formats, then archives + code. The "all" pill clears the filter.
const KIND_PILLS: &[(&str, &str)] = &[
    ("all", "all"),
    ("md", "md"),
    ("org", "org"),
    ("pdf", "pdf"),
    ("docx", "docx"),
    ("xlsx", "xlsx"),
    ("pptx", "pptx"),
    ("csv", "csv"),
    ("tsv", "tsv"),
    ("zip", "zip"),
    ("other", "other"),
];

#[component]
pub fn FilesPanel(active_encoded: Option<String>) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());
    let active_path_for_pills = active_path.clone();

    let files_resource = use_server_future(move || {
        let p = active_path.clone();
        async move {
            match p {
                Some(p) => list_filesystem(p).await.unwrap_or_default(),
                None => Vec::new(),
            }
        }
    })?;

    let files: Vec<SourceFileRow> = files_resource.cloned().unwrap_or_default();

    let encoded = active_encoded.clone().unwrap_or_default();
    let decoded_space = crate::router::decode_space(&encoded);
    // Aggregate counts per kind so the pills show "n" next to each
    // label. "all" gets the total. "other" gets anything that
    // doesn't match a pill key.
    let mut counts: std::collections::BTreeMap<&str, usize> =
        KIND_PILLS.iter().map(|(k, _)| (*k, 0usize)).collect();
    for f in files.iter() {
        let key: &str = if KIND_PILLS.iter().any(|(k, _)| *k != "all" && *k == f.ext) {
            f.ext.as_str()
        } else if KIND_PILLS.iter().any(|(k, _)| *k != "all" && *k == f.kind) {
            f.kind.as_str()
        } else {
            "other"
        };
        counts.entry(key).and_modify(|c| *c += 1).or_insert(1);
    }
    counts.insert("all", files.len());

    rsx! {
        div { class: "files-panel",
            div { class: "files-head",
                span { class: "files-label", "files" }
                span { class: "files-count", "({files.len()})" }
            }
            // Kind pill row — each pill links to /list?kind=X so
            // the user lands on a filtered, sortable view of the
            // whole resource graph (not just the loose files).
            div { class: "files-pills",
                for (key, label) in KIND_PILLS.iter() {
                    {
                        let active_path_pill = active_path_for_pills.clone().unwrap_or_default();
                        let count = counts.get(key).copied().unwrap_or(0);
                        let query = ListQuery { q: String::new(), kind: if *key == "all" { String::new() } else { key.to_string() }, sort: String::new(), source: String::new(), mode: String::new() };
                        let href = crate::router::route_for_space_list_with_query(&active_path_pill, &query);
                        rsx! {
                            a {
                                class: "files-pill files-pill-{key}",
                                href: "{href}",
                                title: "filter list by {label}",
                                span { class: "files-pill-label", "{label}" }
                                span { class: "files-pill-count mono-sm", "{count}" }
                            }
                        }
                    }
                }
            }
            div { class: "files-body",
                if files.is_empty() {
                    {
                        match space().map(|s| s.status.clone()) {
                            Some(SpaceStatus::Resolving) => rsx! { p { class: "files-empty", "loading…" } },
                            Some(SpaceStatus::Error(e)) => rsx! { p { class: "files-empty err-text", "error: {e}" } },
                            _ => rsx! { p { class: "files-empty", "no files in this space yet." } },
                        }
                    }
                } else {
                    ul { class: "files-list",
                        for f in files.iter() {
                            {
                                let is_indexed = !f.ref_str.is_empty();
                                let href = if is_indexed {
                                    route_for_space_resource(&decoded_space, &f.ref_str)
                                } else {
                                    route_for_space_preview(&decoded_space, &f.display_path)
                                };
                                let ext_label = ext_label(&f.ext);
                                let mtime_label = format_mtime(f.mtime_ms);
                                rsx! {
                                    li { class: "files-row", key: "{f.display_path}",
                                        a {
                                            class: "files-link",
                                            href: "{href}",
                                            // PR8: surface the full path as a
                                            // native tooltip — `files-name`
                                            // truncates inside a 320px rail.
                                            title: "{f.display_path}",
                                            span { class: "files-ext files-ext-{f.kind}", "{ext_label}" }
                                            span { class: "files-name", "{f.display_path}" }
                                            if f.size > 0 {
                                                span { class: "files-size mono-sm", "{format_size(f.size)}" }
                                            }
                                            if !mtime_label.is_empty() {
                                                span { class: "files-mtime mono-sm dim", "{mtime_label}" }
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

fn ext_label(ext: &str) -> String {
    if ext.is_empty() { "FILE".into() } else { ext.to_uppercase() }
}

fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
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