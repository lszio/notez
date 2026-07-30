//! `/s/:space/agenda` — renders the space's agenda view (scheduled +
//! deadline tasks). Uses `SendService::agenda()` for the data; the
//! rendering is plain HTML.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
};

use crate::state::WebState;

pub async fn agenda(
    State(state): State<WebState>,
    Path(space): Path<String>,
) -> Result<Html<String>, (StatusCode, String)> {
    let view = state
        .service
        .agenda()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let rows: String = view
        .items
        .iter()
        .map(|item| {
            let r_ref = escape(&item.r_ref);
            let title = escape(&item.title);
            let todo = item
                .todo
                .as_deref()
                .map(|s| format!(r#"<span class="todo">{t}</span>"#, t = escape(s)))
                .unwrap_or_default();
            let scheduled = item
                .scheduled
                .as_deref()
                .map(|s| format!(r#"<span class="sched">{t}</span>"#, t = escape(s)))
                .unwrap_or_default();
            let deadline = item
                .deadline
                .as_deref()
                .map(|s| format!(r#"<span class="due">{t}</span>"#, t = escape(s)))
                .unwrap_or_default();
            format!(
                r#"<li><a href="/s/{space}/r/{r_ref}">{title}</a> {todo}{scheduled}{deadline}</li>"#
            )
        })
        .collect();

    let body = format!(
        r#"<h1>{space} — Agenda</h1>
<p><a href="/s/{space}">← Back to hub</a></p>
<ul class="agenda">{rows}</ul>"#
    );
    Ok(Html(format!(
        "<!doctype html><html><head><title>{space} — Agenda</title></head><body>{body}</body></html>"
    )))
}

fn escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            other => other.to_string(),
        })
        .collect()
}