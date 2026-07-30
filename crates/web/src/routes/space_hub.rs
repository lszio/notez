//! `/s/:space` — the per-space hub page. Lists the most recent resources
//! in the loaded space; falls back to a simple message when the projection
//! is empty.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
};
use domain::Selector;

use crate::state::WebState;

pub async fn hub(
    State(state): State<WebState>,
    Path(space): Path<String>,
) -> Result<Html<String>, (StatusCode, String)> {
    let recent = state
        .service
        .query(&Selector::new())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let items: String = recent
        .items
        .iter()
        .take(50)
        .map(|r| {
            let title = escape(&r.title);
            let r_ref = escape(&r.r#ref.to_string());
            let kind = escape(&format!("{:?}", r.kind).to_lowercase());
            format!(
                r#"<li><span class="kind">{kind}</span> <a href="/s/{space}/r/{r_ref}">{title}</a></li>"#
            )
        })
        .collect();

    let body = format!(
        r#"<h1>{space}</h1>
<p><a href="/s/{space}/agenda">Agenda</a></p>
<h2>Resources</h2>
<ul>{items}</ul>"#
    );
    Ok(Html(format!(
        "<!doctype html><html><head><title>{space} — Notez</title></head><body>{body}</body></html>"
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