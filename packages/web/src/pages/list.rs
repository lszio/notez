//! Resource list page.
//!
//! Layout: page heading (eyebrow + h1 + lede), controls row
//! (search / kind filter / sort), then a column-headed list of
//! rows. Each row carries a kind badge, the ref (monospace, dim),
//! the title (serif, primary), and the source-id + locator
//! (monospace, dim). All filtering and sorting happens on the
//! client; the server only delivers the full list once.

use dioxus::prelude::*;

use crate::model::ResourceRow;
use crate::pages::{use_space_layout, PageHeader};
use crate::pages::ui::KindIcon;
use crate::router::encode_space;
use crate::server::list_resources;
use crate::space_ctx::{SpaceState, SpaceStatus};

#[derive(Debug, Clone, PartialEq)]
enum SortKey {
    Title,
    Kind,
    Locator,
}

impl SortKey {
    fn parse(s: &str) -> SortKey {
        match s {
            "kind" => SortKey::Kind,
            "locator" => SortKey::Locator,
            _ => SortKey::Title,
        }
    }
}

#[component]
pub fn ListPage(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    // Server fetch.
    let active_path = space().map(|s| s.path.clone());
    let active_path_for_forms = active_path.clone();
    let resources = use_server_future(move || {
        let p = active_path.clone();
        async move {
            match p {
                Some(path) => list_resources(path).await,
                None => Ok(Vec::new()),
            }
        }
    })?;

    // Client-side UI state.
    let mut query = use_signal(String::new);
    let mut kind_filter = use_signal(|| "all".to_string());
    let mut sort_key = use_signal(|| "title".to_string());

    let rows: Vec<ResourceRow> = match resources() {
        Some(Ok(r)) => r,
        _ => Vec::new(),
    };

    let total = rows.len();
    let q = query().trim().to_lowercase();
    let kf = kind_filter();
    let sk = SortKey::parse(&sort_key());

    let mut visible: Vec<&ResourceRow> = rows
        .iter()
        .filter(|r| kf == "all" || r.kind == kf)
        .filter(|r| {
            if q.is_empty() {
                return true;
            }
            r.title.to_lowercase().contains(&q)
                || r.ref_str.to_lowercase().contains(&q)
                || r.locator.to_lowercase().contains(&q)
        })
        .collect();
    match sk {
        SortKey::Title => visible.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        SortKey::Kind => visible.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.title.cmp(&b.title))),
        SortKey::Locator => visible.sort_by(|a, b| a.locator.cmp(&b.locator)),
    }

    let current_encoded = space().map(|s| s.encoded.clone()).unwrap_or_default();

    // Page heading text depends on space state.
    let (eyebrow, h1, lede) = match space() {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => (
            format!("space · {}", s.name),
            format!("Index"),
            format!("{} resources across all kinds in this space.", total),
        ),
        Some(SpaceState { status: SpaceStatus::Resolving, path, .. }) => (
            "resolving".to_string(),
            "Index".to_string(),
            format!("looking up {path}…"),
        ),
        Some(SpaceState { status: SpaceStatus::Error(e), path, .. }) => (
            "error".to_string(),
            "Index".to_string(),
            format!("could not load {path}: {e}"),
        ),
        None => (
            "no space".to_string(),
            "Index".to_string(),
            "pick a space from the picker above to begin.".to_string(),
        ),
    };

    rsx! {
        PageHeader {}
        div { class: "page",
            div { class: "page-h",
                p { class: "eyebrow", "{eyebrow}" }
                h1 { "{h1}" }
                p { class: "lede", "{lede}" }
            }

            // ----- Controls row -----
            div { class: "controls",
                div { class: "control",
                    span { class: "control-label", "find" }
                    input {
                        class: "grow",
                        r#type: "search",
                        placeholder: "title, ref, or locator…",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                }
                div { class: "control",
                    span { class: "control-label", "kind" }
                    select {
                        value: "{kind_filter}",
                        onchange: move |e| kind_filter.set(e.value()),
                        option { value: "all", "all" }
                        option { value: "document", "document" }
                        option { value: "heading", "heading" }
                        option { value: "attachment", "attachment" }
                        option { value: "block", "block" }
                    }
                }
                div { class: "control",
                    span { class: "control-label", "sort" }
                    select {
                        value: "{sort_key}",
                        onchange: move |e| sort_key.set(e.value()),
                        option { value: "title", "title" }
                        option { value: "kind", "kind" }
                        option { value: "locator", "locator" }
                    }
                }
                div { class: "control-spacer" }
                div { class: "control",
                    span { class: "control-label", "showing" }
                    span { class: "mono-sm",
                        "{visible.len()} / {total}"
                    }
                }
                div { class: "control-spacer" }
                div { class: "control",
                    form {
                        class: "control-form",
                        action: "/api/spaces/scan",
                        method: "post",
                        input {
                            r#type: "hidden",
                            value: "{active_path_for_forms.clone().unwrap_or_default()}",
                        }
                        button {
                            class: "control-action",
                            r#type: "submit",
                            "scan"
                        }
                    }
                }
                div { class: "control",
                    form {
                        class: "control-form",
                        action: "/api/spaces/watch/start",
                        method: "post",
                        input {
                            r#type: "hidden",
                            name: "space_root",
                            value: "{active_path_for_forms.clone().unwrap_or_default()}",
                        }
                        button {
                            class: "control-action",
                            r#type: "submit",
                            "watch"
                        }
                    }
                }
             }

            // ----- Results -----
            div { class: "results-head",
                span { class: "col-ref", "ref" }
                span { class: "col-title", "title" }
                span { class: "col-loc", "locator" }
            }
            div { class: "results-list",
                match resources() {
                    None => rsx! { p { class: "results-empty skel", "loading…" } },
                    Some(Err(e)) => rsx! {
                        p { class: "results-empty err-text", "load failed: {e}" }
                    },
                    Some(Ok(_)) if visible.is_empty() => rsx! {
                        div { class: "results-empty",
                            if total == 0 {
                                "this space has not been scanned yet."
                                p { class: "hint", "run `notez scan` to populate the index." }
                            } else {
                                "no rows match the current filter."
                                p { class: "hint", "clear the search box or switch the kind filter." }
                            }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        for r in visible.iter() {
                            ResultRow {
                                row: (*r).clone(),
                                current: current_encoded.clone(),
                            }
                        }
                    },
                }
            }

            // ----- Watch panel -----
            WatchPanel { active_path: active_path_for_forms.clone() }

            div { class: "footer-rule",
                span { "notez · reader" }
                span { "·" }
                span { "all kinds in one view" }
            }
        }
    }
}
#[component]
fn WatchPanel(active_path: Option<String>) -> Element {
    if active_path.is_none() {
        return rsx! { Fragment {} };
    }
    let path_str = active_path.unwrap_or_default();
    rsx! {
        section { class: "watch-panel",
            div { class: "watch-head",
                span { class: "watch-label", "watch" }
                form {
                    action: "/api/spaces/watch/stop",
                    method: "post",
                    input { r#type: "hidden", name: "space_root", value: "{path_str}" }
                    button { class: "watch-btn", r#type: "submit", "stop" }
                }
            }
            p { class: "watch-hint",
                "Visit /api/spaces/watch/state?path=" 
                code { "{path_str}" }
                " for the live event log."
            }
        }
    }
}

#[component]
fn ResultRow(row: ResourceRow, current: String) -> Element {
    let ref_link = format!(
        "/space/{}/resource/{}",
        current,
        crate::router::encode_space(&row.ref_str)
    );
    rsx! {
        div { class: "result",
            span { class: "col-ref",
                KindIcon { kind: row.kind.clone() }
                a { href: "{ref_link}", "{row.ref_str}" }
             }
            span { class: "col-title",
                a { href: "{ref_link}", "{row.title}" }
                span { class: "meta", "— {row.source_id}" }
            }
            span { class: "col-loc", "{row.locator}" }
        }
    }
}
