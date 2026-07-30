//! `/s/:space/r/:ref` — show a single resource via the matched previewer.
//!
//! The handler resolves the `PreviewerCatalog`, builds a `PreviewContext`,
//! and emits a small HTML page that wraps the rendered preview. The
//! client-side hydration scripts (`/static/js/preview/mermaid.mjs` etc.)
//! run after `DOMContentLoaded` and upgrade any placeholder `<div>`s.

use std::path::PathBuf;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use domain::ResourceRef;
use preview::{Heading, PreviewContext, PreviewModel};

use crate::html_escape::{escape, escape_attr};
use crate::state::WebState;

#[derive(serde::Deserialize)]
pub struct ResourceParams {
    pub space: String,
    #[serde(rename = "ref")]
    pub r_ref: String,
}

pub async fn show(
    State(state): State<WebState>,
    Path(p): Path<ResourceParams>,
) -> Result<Html<String>, (StatusCode, String)> {
    let parsed = ResourceRef::parse(&p.r_ref)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad ref: {e}")))?;
    let resource = state
        .service
        .read(&parsed)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "resource not found".into()))?;
    let ctx = PreviewContext {
        resource: resource.clone(),
        bytes: None,
        mime: None,
        locator: PathBuf::from(&resource.locator),
        segments: Vec::new(),
        siblings: Vec::new(),
        catalog: &state.catalog,
        service: None,
    };
    let previewer = state
        .catalog
        .resolve(None, &ctx)
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "no previewer".into()))?;
    let model = previewer
        .render(&ctx)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("render: {e}")))?;
    let model_json = serde_json::to_string(&model)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let body = render_resource_page(
        &resource.title,
        &resource.r#ref.to_string(),
        &model,
        &model_json,
        &p.space,
    );
    Ok(Html(body))
}

pub async fn preview_json(
    State(state): State<WebState>,
    Path(p): Path<ResourceParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let parsed = ResourceRef::parse(&p.r_ref)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad ref: {e}")))?;
    let resource = state
        .service
        .read(&parsed)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "resource not found".into()))?;
    let ctx = PreviewContext {
        resource: resource.clone(),
        bytes: None,
        mime: None,
        locator: PathBuf::from(&resource.locator),
        segments: Vec::new(),
        siblings: Vec::new(),
        catalog: &state.catalog,
        service: None,
    };
    let previewer = state
        .catalog
        .resolve(None, &ctx)
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "no previewer".into()))?;
    let model = previewer
        .render(&ctx)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("render: {e}")))?;
    let json = serde_json::to_string(&model)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, [("content-type", "application/json")], json))
}

fn render_resource_page(
    title: &str,
    r_ref: &str,
    model: &PreviewModel,
    model_json: &str,
    space: &str,
) -> String {
    let body = render_model(model);
    format!(
        r#"<!doctype html>
<html>
<head>
<title>{title} — Notez</title>
<meta name="notez-space" content="{space}">
<meta name="notez-ref" content="{r_ref}">
</head>
<body>
<h1>{title}</h1>
<p class="ref"><code>{r_ref}</code></p>
<section class="preview">
{body}
</section>
<script id="notez-preview-model" type="application/json">{model_json}</script>
</body>
</html>"#
    )
}

fn render_model(model: &PreviewModel) -> String {
    match model {
        PreviewModel::Org { html, outline } => {
            let nav = render_outline(outline);
            format!(r#"<nav class="outline">{nav}</nav><article>{html}</article>"#)
        }
        PreviewModel::Markdown { html } => format!(r#"<article>{html}</article>"#),
        PreviewModel::Pdf { pages, .. } => {
            let chunks: String = pages
                .iter()
                .map(|p| {
                    format!(
                        r#"<section><h3>Page {}</h3><pre>{}</pre></section>"#,
                        p.index,
                        escape(&p.text)
                    )
                })
                .collect();
            format!("<article>{chunks}</article>")
        }
        PreviewModel::Xlsx { sheets } => {
            let chunks: String = sheets
                .iter()
                .map(|s| {
                    let rows: String = s
                        .rows
                        .iter()
                        .map(|r| {
                            let cells: String =
                                r.iter().map(|c| format!(r#"<td>{}</td>"#, escape(c))).collect();
                            format!("<tr>{cells}</tr>")
                        })
                        .collect();
                    format!(
                        r#"<section><h3>{}</h3><table>{}</table></section>"#,
                        escape(&s.name),
                        rows
                    )
                })
                .collect();
            format!("<article>{chunks}</article>")
        }
        PreviewModel::Pptx { slides } => {
            let chunks: String = slides
                .iter()
                .map(|s| {
                    let body: String = s
                        .body
                        .iter()
                        .map(|t| format!("<p>{}</p>", escape(t)))
                        .collect();
                    format!(r#"<section><h3>Slide {}</h3>{}</section>"#, s.index, body)
                })
                .collect();
            format!("<article>{chunks}</article>")
        }
        PreviewModel::Zip { entries } => {
            let rows: String = entries
                .iter()
                .map(|e| format!(r#"<li>{} ({})</li>"#, escape(&e.path), e.size))
                .collect();
            format!("<article><ul>{rows}</ul></article>")
        }
        PreviewModel::Image {
            src,
            width,
            height,
            mime,
        } => {
            // `src` is rendered into a URL attribute and the locale-derived
            // path; `mime` is user input from the resource body. Both must
            // be attribute-escaped to defuse `" onload="…` style payloads.
            format!(
                r#"<article><img src="{src}" width="{width}" height="{height}" alt="{alt}"></article>"#,
                src = escape_attr(src),
                alt = escape_attr(mime),
            )
        }
        PreviewModel::Mermaid { source } => {
            format!(
                r#"<article><div class="mermaid-block" data-source="{escaped}"></div></article>
<script type="module" src="/static/js/preview/mermaid.mjs"></script>"#,
                escaped = escape_attr(source)
            )
        }
        PreviewModel::D2 { source } => {
            format!(
                r#"<article><div class="d2-block" data-source="{escaped}"></div></article>
<script type="module" src="/static/js/preview/d2.mjs"></script>"#,
                escaped = escape_attr(source)
            )
        }
        PreviewModel::Iframe { src, sandbox } => {
            // Both fields come from user-authored body content
            // (`[[iframe:<src> <sandbox>]]`) and must be attribute-escaped
            // to defuse attribute-breakout payloads such as
            // `" onload="alert(1)" sandbox=`.
            format!(
                r#"<article><iframe src="{src}" sandbox="{sandbox}" style="width:100%;height:60vh;border:0"></iframe></article>"#,
                src = escape_attr(src),
                sandbox = escape_attr(sandbox),
            )
        }
        PreviewModel::LinkEmbed { target, child } => {
            let inner = render_model(child);
            format!(
                r#"<article><p>↪ <code>{}</code></p>{}</article>"#,
                escape(&target.r#ref.to_string()),
                inner
            )
        }
        PreviewModel::QueryEmbed { query, snapshot } => {
            let rows: String = snapshot
                .iter()
                .map(|r| {
                    format!(
                        r#"<li>{} — <code>{}</code></li>"#,
                        escape(&r.title),
                        escape(&r.r#ref.to_string())
                    )
                })
                .collect();
            format!(
                r#"<article><details><summary>Query: {}</summary><ul>{}</ul></details></article>"#,
                escape(&query.source),
                rows
            )
        }
        PreviewModel::BlockEmbed { source, html } => {
            // The embedded `html` is the raw body of a (currently
            // proxy) source resource, not a sanitised fragment — until the
            // block target is resolved through the application service
            // it may contain markup. Drop it inside a `<pre>` after
            // HTML-escaping so any `<script>` / attribute-breakout
            // payload becomes inert text.
            //
            // TODO(P3): when block resolution is wired through
            // `PreviewContext::service`, replace this with the rendered
            // target's preview HTML.
            format!(
                r#"<article><p class="embed-source"><code>{}</code></p><pre class="block-embed">{}</pre></article>"#,
                escape(&source.to_string()),
                escape(&html)
            )
        }
        PreviewModel::Fallback { message } => {
            format!(r#"<article><p class="fallback">{}</p></article>"#, escape(message))
        }
    }
}

fn render_outline(outline: &[Heading]) -> String {
    if outline.is_empty() {
        return String::new();
    }
    let rows: String = outline
        .iter()
        .map(|h| {
            format!(
                r##"<li><a href="#{}">{}</a></li>"##,
                escape(&h.anchor),
                escape(&h.title)
            )
        })
        .collect();
    format!("<ul>{rows}</ul>")
}