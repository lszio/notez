//! `/` — space picker. Lists every space registered with the current
//! process; for the single-space demo this is just the loaded space.

use axum::{extract::State, response::Html};

use crate::state::WebState;

pub async fn root(State(state): State<WebState>) -> Html<String> {
    let name = state
        .space_root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("notez")
        .to_string();
    let href = html_escape(&name);
    let body = format!(
        r#"<h1>Notez</h1><ul><li><a href="/s/{href}">{name}</a></li></ul>"#
    );
    Html(format!(
        "<!doctype html><html><head><title>Notez</title></head><body>{body}</body></html>"
    ))
}

fn html_escape(s: &str) -> String {
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