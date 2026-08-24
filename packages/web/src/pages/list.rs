//! Resource list page.

use dioxus::prelude::*;

use crate::model::ResourceRow;
use crate::pages::ui::KindIcon;
use crate::pages::use_space_layout;
use crate::router::{encode_space, ListQuery};
use crate::server::list_resources;
use crate::space_ctx::{SpaceState, SpaceStatus};

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

/// Extract the locator extension lowercased. Empty string when the
/// locator has no extension.
fn locator_ext(locator: &str) -> String {
    std::path::Path::new(locator)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[component]
pub fn ListPage(encoded: String, query: ListQuery) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();
    let mut resource_ref_ctx = use_context::<Signal<Option<String>>>();
    resource_ref_ctx.set(None);
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

    // Seed the search/filter signals from the URL query so deep
    // links like `/list?kind=attachment&sort=mtime` produce the
    // expected view on first paint.
    let mut q_filter = use_signal(move || query.q.clone());
    let mut kind_filter = use_signal(move || {
        if query.kind.is_empty() {
            "all".to_string()
        } else {
            query.kind.clone()
        }
    });
    let mut sort_key = use_signal(move || {
        if query.sort.is_empty() {
            "title".to_string()
        } else {
            query.sort.clone()
        }
    });

    let rows: Vec<ResourceRow> = match resources() {
        Some(Ok(r)) => r,
        _ => Vec::new(),
    };

    let total = rows.len();
    let q = q_filter().trim().to_lowercase();
    let kf = kind_filter();
    let sk = SortKey::parse(&sort_key());

    let mut visible: Vec<&ResourceRow> = rows
        .iter()
        .filter(|r| match kf.as_str() {
            "all" => true,
            "document" | "heading" | "attachment" | "block" => r.kind == kf,
            // Extension-based filters (md / org / pdf / docx / xlsx /
            // pptx / csv / tsv / zip / other): match the locator's
            // file extension; `other` matches anything not in the
            // known list.
            "other" => {
                let ext = locator_ext(&r.locator);
                !matches!(
                    ext.as_str(),
                    "md" | "markdown"
                        | "org"
                        | "pdf"
                        | "docx"
                        | "xlsx"
                        | "pptx"
                        | "csv"
                        | "tsv"
                        | "zip"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "txt"
                )
            }
            ext => locator_ext(&r.locator).eq_ignore_ascii_case(ext),
        })
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
        SortKey::Title => {
            visible.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }
        SortKey::Kind => visible.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.title.cmp(&b.title))),
        SortKey::Locator => visible.sort_by(|a, b| a.locator.cmp(&b.locator)),
    }

    let current_encoded = space().map(|s| s.encoded.clone()).unwrap_or_default();

    let (eyebrow, h1, lede) = match space() {
        Some(SpaceState { status: SpaceStatus::Ready(s), .. }) => (
            format!("space · {}", s.name),
            "Index".to_string(),
            format!("{total} resources across all kinds in this space."),
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
            "pick a space from the sidebar to begin.".to_string(),
        ),
    };

    rsx! {
        div { class: "page",
            div { class: "page-h",
                p { class: "eyebrow", "{eyebrow}" }
                h1 { "{h1}" }
                p { class: "lede", "{lede}" }
            }

            div { class: "controls-wrapper", style: "display:flex; gap:1rem; align-items:center; width:100%; flex-wrap:wrap;",
                form {
                    class: "controls",
                    action: format!("/space/{}/list", encoded),
                    style: "flex:1; display:flex; gap:0.6rem; align-items:center;",
                    div { class: "control",
                        span { class: "control-label", "find" }
                        input {
                            class: "grow",
                            r#type: "search",
                            name: "q",
                            placeholder: "title, ref, or locator…",
                            value: "{q_filter}",
                            oninput: move |e| q_filter.set(e.value()),
                        }
                    }
                    div { class: "control",
                        span { class: "control-label", "kind" }
                        select {
                            name: "kind",
                            value: "{kind_filter}",
                            onchange: move |e| kind_filter.set(e.value()),
                            option { value: "all", selected: kind_filter() == "all", "all" }
                            option { value: "document", selected: kind_filter() == "document", "document" }
                            option { value: "heading", selected: kind_filter() == "heading", "heading" }
                            option { value: "attachment", selected: kind_filter() == "attachment", "attachment" }
                            option { value: "block", selected: kind_filter() == "block", "block" }
                            option { value: "-----", disabled: true, "--- extensions ---" }
                            option { value: "md", selected: kind_filter() == "md", "md" }
                            option { value: "org", selected: kind_filter() == "org", "org" }
                            option { value: "pdf", selected: kind_filter() == "pdf", "pdf" }
                            option { value: "docx", selected: kind_filter() == "docx", "docx" }
                            option { value: "xlsx", selected: kind_filter() == "xlsx", "xlsx" }
                            option { value: "pptx", selected: kind_filter() == "pptx", "pptx" }
                            option { value: "csv", selected: kind_filter() == "csv", "csv" }
                            option { value: "tsv", selected: kind_filter() == "tsv", "tsv" }
                            option { value: "zip", selected: kind_filter() == "zip", "zip" }
                            option { value: "other", selected: kind_filter() == "other", "other" }
                        }
                    }
                    div { class: "control",
                        span { class: "control-label", "sort" }
                        select {
                            name: "sort",
                            value: "{sort_key}",
                            onchange: move |e| sort_key.set(e.value()),
                            option { value: "title", selected: sort_key() == "title", "title" }
                            option { value: "kind", selected: sort_key() == "kind", "kind" }
                            option { value: "locator", selected: sort_key() == "locator", "locator" }
                        }
                    }
                    button {
                        class: "control-action",
                        r#type: "submit",
                        "filter"
                    }
                    div { class: "control-spacer" }
                    div { class: "control",
                        span { class: "control-label", "showing" }
                        span { class: "mono-sm",
                            "{visible.len()} / {total}"
                        }
                    }
                }
                div { class: "control",
                    form {
                        class: "control-form",
                        action: "/api/spaces/scan",
                        method: "post",
                        input {
                            r#type: "hidden",
                            name: "space_root",
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
                    input { r#type: "hidden", name: "path", value: "{path_str}" }
                    button { r#type: "submit", "stop watching" }
                }
            }
            p { class: "watch-note", "watching {path_str}; new files appear in the left column." }
        }
    }
}

#[component]
fn ResultRow(row: ResourceRow, current: String) -> Element {
    let ref_link = format!(
        "/space/{}/resource/{}",
        current,
        encode_space(&row.ref_str)
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
