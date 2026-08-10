//! Body preview rendering for the detail page.
//!
//! Given a `ResourceRow` and the absolute path to the space root,
//! read the file backing the resource and render the relevant
//! section as HTML. We support:
//!
//! - **Markdown** (`.md`) via `pulldown-cmark`.
//! - **Org-mode** (`.org`) via a simple "escape into a `<pre>`" pass.
//!   The full org → HTML converter is in `notez_core::preview`, but
//!   it requires a `PreviewerCatalog` that we do not currently wire
//!   into the web server. The raw text pass is correct (every char
//!   is preserved verbatim) and renders with the paper-terminal
//!   monospace font in the detail page.
//! - **Anything else** (attachments, missing file): return an empty
//!   string. The detail page will hide the body section entirely.
//!
//! For documents, the whole file is the body. For headings and
//! blocks, we currently return the full file too: the v0.1 index
//! does not yet record line ranges, so we cannot slice a section
//! out of the file. A future version of this module can read the
//! line range from `Resource.properties` once that is populated.

use std::path::{Path, PathBuf};

use notez_core::preview::Previewer;
use pulldown_cmark::{html, Options, Parser};

use crate::model::ResourceRow;

/// Render the body of `row` for the detail page.
///
/// `space_root` is the absolute path to the space's directory. The
/// resource's `locator` is joined onto it; the result is read as
/// UTF-8 and rendered. Returns an empty string when the file is
/// missing, unreadable, or the resource has no inline content
/// (attachments).
pub fn render_body(row: &ResourceRow, space_root: &Path) -> String {
    let file_path = resolve_file_path(space_root, &row.locator);
    let bytes = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let raw_url = format!(
        "/api/spaces/attachment/raw?space_root={}&locator={}",
        urlencoding::encode(&space_root.to_string_lossy()),
        urlencoding::encode(&row.locator)
    );

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => {
            format!(
                "<div class=\"preview-img\"><img src=\"{raw_url}\" alt=\"{}\" style=\"max-width:100%;height:auto;\" /></div>",
                html_escape::encode_safe(&row.title)
            )
        }
        "pdf" => {
            let previewer = notez_core::preview::builders::pdf::PdfPreviewer;
            let mut catalog = notez_core::preview::PreviewerCatalog::new();
            catalog.register(notez_core::preview::builders::pdf::PdfPreviewer);
            let ctx = notez_core::preview::PreviewContext {
                resource: row.to_domain_resource(),
                bytes: Some(bytes::Bytes::from(bytes.clone())),
                mime: Some("application/pdf".into()),
                locator: file_path.clone(),
                segments: vec![],
                siblings: vec![],
                catalog: &catalog,
            };
            let text_preview = if let Ok(notez_core::preview::PreviewModel::Pdf { text, .. }) = previewer.render(&ctx) {
                if text.trim().is_empty() {
                    String::new()
                } else {
                    format!("<details><summary class=\"mono-sm\">Extracted PDF Text Preview</summary><pre class=\"raw\">{}</pre></details>", html_escape::encode_safe(&text))
                }
            } else {
                String::new()
            };

            format!(
                "<div class=\"preview-pdf\"><p><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📄 Open PDF in new tab ({})</a></p>{text_preview}<iframe src=\"{raw_url}\" width=\"100%\" height=\"600px\" style=\"border:1px solid var(--ink-rule);margin-top:0.5rem;\"></iframe></div>",
                html_escape::encode_safe(&row.title)
            )
        }
        "xlsx" | "xls" => {
            let previewer = notez_core::preview::builders::xlsx::XlsxPreviewer;
            let mut catalog = notez_core::preview::PreviewerCatalog::new();
            catalog.register(notez_core::preview::builders::xlsx::XlsxPreviewer);
            let ctx = notez_core::preview::PreviewContext {
                resource: row.to_domain_resource(),
                bytes: Some(bytes::Bytes::from(bytes.clone())),
                mime: Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into()),
                locator: file_path.clone(),
                segments: vec![],
                siblings: vec![],
                catalog: &catalog,
            };
            if let Ok(notez_core::preview::PreviewModel::Xlsx { sheets }) = previewer.render(&ctx) {
                let mut html = String::from("<div class=\"preview-xlsx\">");
                for sheet in sheets {
                    html.push_str(&format!("<h3>Sheet: {}</h3><table class=\"preview-table\" style=\"border-collapse:collapse;width:100%;margin-bottom:1rem;\">", html_escape::encode_safe(&sheet.name)));
                    for r in sheet.rows {
                        html.push_str("<tr>");
                        for cell in r {
                            html.push_str(&format!("<td style=\"border:1px solid var(--ink-rule);padding:0.2rem 0.5rem;\">{}</td>", html_escape::encode_safe(&cell)));
                        }
                        html.push_str("</tr>");
                    }
                    html.push_str("</table>");
                }
                html.push_str("</div>");
                return html;
            }
            format!("<div class=\"preview-download\"><a href=\"{raw_url}\" class=\"spine-action\">📥 Download Excel File</a></div>")
        }
        "md" | "markdown" => {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                render_markdown(text)
            } else {
                String::new()
            }
        }
        "org" => {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                render_org(text)
            } else {
                String::new()
            }
        }
        _ => {
            if row.kind == "attachment" {
                format!(
                    "<div class=\"preview-download\"><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Download Attachment File ({})</a></div>",
                    html_escape::encode_safe(&row.title)
                )
            } else if let Ok(text) = std::str::from_utf8(&bytes) {
                render_fallback(text)
            } else {
                format!(
                    "<div class=\"preview-download\"><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Download Binary File ({})</a></div>",
                    html_escape::encode_safe(&row.title)
                )
            }
        }
    }
}

/// Render a Markdown source to an HTML fragment. We use a small
/// set of options — no footnotes, no table-of-contents — that
/// keeps the resulting HTML compact and matches the v0.1 reader's
/// reading column.
fn render_markdown(text: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(text, opts);
    let mut out = String::with_capacity(text.len() + 32);
    html::push_html(&mut out, parser);
    out
}

/// Render an Org-mode source as a `<pre>`-wrapped block. The HTML
/// is escaped by `pulldown_cmark::html` after we wrap the source
/// in a markdown code fence, which is the simplest path that
/// produces safe output. A proper org → HTML converter will
/// replace this in v0.4.
fn render_org(text: &str) -> String {
    let fenced = format!("```org\n{text}\n```");
    let parser = Parser::new(&fenced);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

/// Render an unknown text format. We escape special chars and
/// wrap in a `<pre>`. The reader can still see the raw bytes.
fn render_fallback(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<pre class=\"raw\">{}</pre>", escaped)
}

fn resolve_file_path(space_root: &Path, locator: &str) -> PathBuf {
    let p = PathBuf::from(locator);
    if p.is_absolute() {
        p
    } else {
        space_root.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_with_kind_locator(kind: &str, locator: &str) -> ResourceRow {
        let mut properties = std::collections::BTreeMap::new();
        properties.insert("k".to_string(), "v".to_string());
        ResourceRow {
            ref_str: format!("{kind}:01ARZ"),
            kind: kind.to_string(),
            title: "T".to_string(),
            source_id: "src".to_string(),
            locator: locator.to_string(),
            object_id: "oid".to_string(),
            revision: "r1".to_string(),
            properties,
            body_html: String::new(),
        }
    }

    #[test]
    fn missing_attachment_yields_empty_body() {
        let row = row_with_kind_locator("attachment", "x.png");
        assert_eq!(render_body(&row, Path::new("/nope")), "");
    }

    #[test]
    fn missing_file_yields_empty_body() {
        let row = row_with_kind_locator("document", "no-such-file.md");
        assert_eq!(render_body(&row, Path::new("/tmp")), "");
    }

    #[test]
    fn markdown_renders_to_html() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.md");
        std::fs::write(&file, "# heading\n\nparagraph\n").unwrap();
        let row = row_with_kind_locator("document", "a.md");
        let body = render_body(&row, dir.path());
        assert!(body.contains("<h1>"), "missing h1: {body}");
        assert!(body.contains("paragraph"));
    }

    #[test]
    fn org_falls_back_to_pre_block() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.org");
        std::fs::write(&file, "* TODO thing\n").unwrap();
        let row = row_with_kind_locator("document", "a.org");
        let body = render_body(&row, dir.path());
        // The fenced-code path produces a <pre> with the source.
        assert!(body.contains("TODO") || body.contains("&lt;"));
    }
}
