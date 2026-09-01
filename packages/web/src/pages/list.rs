//! Resource list page.

use dioxus::prelude::*;
use ui::notez::NzBadge;
use ui::notez::NzInput;

use crate::model::ResourceRow;
use crate::pages::ui::KindIcon;
use crate::pages::use_space_layout;
use crate::router::{encode_space, route_for_space_files_with_query, ListQuery};
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

/// Presentation mode for the query result set. Every mode renders the
/// same typed `ResourceRow` data — the choice only changes layout.
/// Persisted in the URL (`?mode=…`) so a view survives reloads and
/// deep links. Unknown values fall back to the default list mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListMode {
    List,
    Table,
    Cards,
    Stream,
}

impl ListMode {
    fn parse(s: &str) -> ListMode {
        match s {
            "table" => ListMode::Table,
            "cards" => ListMode::Cards,
            "stream" => ListMode::Stream,
            _ => ListMode::List,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            ListMode::List => "list",
            ListMode::Table => "table",
            ListMode::Cards => "cards",
            ListMode::Stream => "stream",
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
    // Clear selection after mount — never write signals during render
    // (a render-time set causes rerender storms under hydration).
    use_effect(move || resource_ref_ctx.set(None));
    let active_path = space().map(|s| s.path.clone());
    let active_path_for_forms = active_path.clone();
    let active_path_for_view = active_path.clone();

    let resources = use_server_future(move || {
        let p = active_path.clone();
        async move {
            match p {
                Some(path) => list_resources(path).await,
                None => Ok(Vec::new()),
            }
        }
    })?;

    // Snapshot the URL query for the "this view lives in the URL"
    // deep link before the seed closures below move the fields out
    // of `query`.
    let view_query = query.clone();

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
    // Source scope: filter the view to rows from one source. Seeded
    // from the URL query (`?source=…`) so deep links and the form GET
    // preserve the choice; the dropdown is populated from the sources
    // actually present.
    let mut source_filter = use_signal(move || {
        if query.source.is_empty() {
            "all".to_string()
        } else {
            query.source.clone()
        }
    });
    // Mode is URL-persisted (no signal): the segmented control
    // navigates with the full current view state, so reloads and
    // deep links reproduce the same presentation.
    let mode = ListMode::parse(&view_query.mode);

    let rows: Vec<ResourceRow> = match resources() {
        Some(Ok(r)) => r,
        _ => Vec::new(),
    };
    let mut sources: Vec<String> = rows.iter().map(|r| r.source_id.clone()).collect();
    sources.sort();
    sources.dedup();

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
        .filter(|r| {
            let sf = source_filter();
            sf == "all" || r.source_id == sf
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
    // Mode-switch links carry the LIVE filter state (what the user is
    // looking at right now) plus the new mode, so the URL always
    // round-trips the full view. The view-note deep link, by contrast,
    // points at the canonical URL of the current page.
    let space_path_for_view = active_path_for_view.clone().unwrap_or_default();
    let mode_href = |m: ListMode| -> String {
        route_for_space_files_with_query(&space_path_for_view, &ListQuery {
            q: q_filter(),
            kind: kind_filter(),
            sort: sort_key(),
            source: source_filter(),
            mode: m.as_str().to_string(),
        })
    };
    let current_url = route_for_space_files_with_query(&space_path_for_view, &view_query);
    // Source coverage of the visible results — how many rows come from
    // each source, so a filtered view always shows where the data lives.
    let mut source_counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for r in visible.iter() {
        *source_counts.entry(r.source_id.clone()).or_insert(0) += 1;
    }
 
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
                    action: format!("/source/{}/files", encoded),
                    style: "flex:1; display:flex; gap:0.6rem; align-items:center;",
                    div { class: "control",
                        span { class: "control-label", "find" }
                        NzInput {
                            placeholder: "title, ref, or locator…",
                            value: q_filter(),
                            name: "q".to_string(),
                            input_type: "search".to_string(),
                            aria_label: "Filter by title, ref, or locator".to_string(),
                            oninput: move |e: dioxus::prelude::FormEvent| q_filter.set(e.value()),
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
                    div { class: "control",
                        span { class: "control-label", "source" }
                        select {
                            name: "source",
                            value: "{source_filter}",
                            onchange: move |e| source_filter.set(e.value()),
                            option { value: "all", selected: source_filter() == "all", "all" }
                            for s in sources.iter() {
                                option { value: "{s}", selected: source_filter() == *s, "{s}" }
                            }
                        }
                    }
                    input {
                        r#type: "hidden",
                        name: "mode",
                        value: "{mode.as_str()}",
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
                        action: "/api/sources/scan",
                        method: "post",
                        input {
                            r#type: "hidden",
                            name: "source_root",
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
                        action: "/api/sources/watch/start",
                        method: "post",
                        input {
                            r#type: "hidden",
                            name: "source_root",
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

            div { class: "results-toolbar",
                div { class: "mode-tabs", role: "group", "aria-label": "Result view mode",
                    ModeTab { mode, current: ListMode::List, href: mode_href(ListMode::List), label: "list" }
                    ModeTab { mode, current: ListMode::Table, href: mode_href(ListMode::Table), label: "table" }
                    ModeTab { mode, current: ListMode::Cards, href: mode_href(ListMode::Cards), label: "cards" }
                    ModeTab { mode, current: ListMode::Stream, href: mode_href(ListMode::Stream), label: "stream" }
                }
                div { class: "view-note",
                    span { class: "view-note-label", "saved view" }
                    span { class: "view-note-state", "unsaved" }
                    a { class: "view-note-link mono-sm", href: "{current_url}", "this view lives in the URL — share the link" }
                }
            }

            if mode == ListMode::List {
                div { class: "results-head",
                    span { class: "col-ref", "ref" }
                    span { class: "col-title", "title" }
                    span { class: "col-loc", "locator" }
                }
            }
            div { class: "source-coverage",
                 span { class: "source-coverage-label", "sources" }
                 for (sid, n) in source_counts.iter() {
                     span { class: "source-coverage-item",
                         "{sid}: {n}"
                     }
                 }
                 if source_counts.is_empty() {
                     span { class: "source-coverage-item dim", "none" }
                 }
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
                    Some(Ok(_)) => match mode {
                        ListMode::List => rsx! {
                            for r in visible.iter() {
                                ResultRow {
                                    row: (*r).clone(),
                                    current: current_encoded.clone(),
                                }
                            }
                        },
                        ListMode::Table => {
                            let rows: Vec<ResourceRow> = visible.iter().map(|r| (*r).clone()).collect();
                            rsx! {
                                ModeTable { rows, current: current_encoded.clone() }
                            }
                        }
                        ListMode::Cards => {
                            let rows: Vec<ResourceRow> = visible.iter().map(|r| (*r).clone()).collect();
                            rsx! {
                                ModeCards { rows, current: current_encoded.clone() }
                            }
                        }
                        ListMode::Stream => {
                            let rows: Vec<ResourceRow> = visible.iter().map(|r| (*r).clone()).collect();
                            rsx! {
                                ModeStream { rows, current: current_encoded.clone() }
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
                    action: "/api/sources/watch/stop",
                    method: "post",
                    input { r#type: "hidden", name: "source_root", value: "{path_str}" }
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
        "/source/{}/note/{}",
        current,
        encode_space(&row.ref_str)
    );
    let editable = row.kind == "document";
    rsx! {
        div { class: "result",
            span { class: "col-ref",
                KindIcon { kind: row.kind.clone() }
                a { href: "{ref_link}", "{row.ref_str}" }
            }
            span { class: "col-title",
                a { href: "{ref_link}", "{row.title}" }
                if editable {
                    NzBadge { text: format!("✎ {}", row.source_id.clone()), tone: "ok".to_string() }
                } else {
                    NzBadge { text: format!("· {}", row.source_id.clone()), tone: "warn".to_string() }
                }
            }
            span { class: "col-loc", "{row.locator}" }
        }
    }
}

/// One segment of the mode switcher. Renders as a link so the choice
/// is URL-persisted; the active mode is marked with `aria-current`
/// and the `is-current` class.
#[component]
fn ModeTab(mode: ListMode, current: ListMode, href: String, label: &'static str) -> Element {
    let cls = if mode == current {
        "doc-mode-tab mode-tab is-current"
    } else {
        "doc-mode-tab mode-tab"
    };
    rsx! {
        a {
            class: "{cls}",
            href: "{href}",
            "aria-current": if mode == current { "true" } else { "false" },
            "{label}"
        }
    }
}

/// Table mode: the same typed rows in a `<table>` with explicit
/// columns. Row elements keep the `.result` class and the
/// `.col-ref` / `.col-title` / `.col-loc` hooks so the existing
/// SSR-only fallback JS (live `q` filtering + palette corpus) keeps
/// working across every mode.
#[component]
fn ModeTable(rows: Vec<ResourceRow>, current: String) -> Element {
    rsx! {
        table { class: "results-table",
            thead {
                tr {
                    th { scope: "col", "kind" }
                    th { scope: "col", "ref" }
                    th { scope: "col", "title" }
                    th { scope: "col", "locator" }
                    th { scope: "col", "source" }
                }
            }
            tbody {
                for row in rows.iter() {
                    TableRow { row: row.clone(), current: current.clone() }
                }
            }
        }
    }
}

#[component]
fn TableRow(row: ResourceRow, current: String) -> Element {
    let ref_link = format!(
        "/source/{}/note/{}",
        current,
        encode_space(&row.ref_str)
    );
    rsx! {
        tr { class: "result result-table-row",
            td { class: "col-kind", KindIcon { kind: row.kind.clone() } }
            td { class: "col-ref", a { href: "{ref_link}", "{row.ref_str}" } }
            td { class: "col-title", a { href: "{ref_link}", "{row.title}" } }
            td { class: "col-loc", "{row.locator}" }
            td { class: "col-source mono-sm", "{row.source_id}" }
        }
    }
}

/// Cards mode: one card per resource, same fields as the list row.
#[component]
fn ModeCards(rows: Vec<ResourceRow>, current: String) -> Element {
    rsx! {
        div { class: "results-cards",
            for row in rows.iter() {
                CardRow { row: row.clone(), current: current.clone() }
            }
        }
    }
}

#[component]
fn CardRow(row: ResourceRow, current: String) -> Element {
    let ref_link = format!(
        "/source/{}/note/{}",
        current,
        encode_space(&row.ref_str)
    );
    rsx! {
        div { class: "result result-card",
            span { class: "col-title",
                a { href: "{ref_link}", "{row.title}" }
            }
            span { class: "col-ref",
                KindIcon { kind: row.kind.clone() }
                a { href: "{ref_link}", "{row.ref_str}" }
            }
            span { class: "col-loc", "{row.locator}" }
            span { class: "col-source mono-sm", "{row.source_id}" }
        }
    }
}

/// Stream mode: the most compact rendering — one line per resource.
#[component]
fn ModeStream(rows: Vec<ResourceRow>, current: String) -> Element {
    rsx! {
        div { class: "results-stream",
            for row in rows.iter() {
                StreamRow { row: row.clone(), current: current.clone() }
            }
        }
    }
}

#[component]
fn StreamRow(row: ResourceRow, current: String) -> Element {
    let ref_link = format!(
        "/source/{}/note/{}",
        current,
        encode_space(&row.ref_str)
    );
    rsx! {
        div { class: "result result-stream",
            span { class: "col-ref",
                KindIcon { kind: row.kind.clone() }
                a { href: "{ref_link}", "{row.ref_str}" }
            }
            span { class: "col-title", a { href: "{ref_link}", "{row.title}" } }
            span { class: "col-loc", "{row.locator}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_mode_parse_known_values() {
        assert_eq!(ListMode::parse("table"), ListMode::Table);
        assert_eq!(ListMode::parse("cards"), ListMode::Cards);
        assert_eq!(ListMode::parse("stream"), ListMode::Stream);
        assert_eq!(ListMode::parse(""), ListMode::List);
    }

    #[test]
    fn list_mode_parse_unknown_falls_back_to_list() {
        assert_eq!(ListMode::parse("bogus"), ListMode::List);
        assert_eq!(ListMode::parse("LIST"), ListMode::List);
    }

    #[test]
    fn list_mode_as_str_round_trips() {
        for mode in [ListMode::List, ListMode::Table, ListMode::Cards, ListMode::Stream] {
            assert_eq!(ListMode::parse(mode.as_str()), mode);
        }
    }
}
