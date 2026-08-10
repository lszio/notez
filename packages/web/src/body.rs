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
    // Attachments live outside the inline-body model.
    if row.kind == "attachment" {
        return String::new();
    }
    let file_path = resolve_file_path(space_root, &row.locator);
    let bytes = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => render_markdown(text),
        "org" => render_org(text),
        _ => render_fallback(text),
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
    fn attachments_have_no_body() {
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
