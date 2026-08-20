//! Body preview rendering for the detail page.
//!
//! Two entry points:
//! - `render_body(&ResourceRow, &Path)` is what the resource detail
//!   page uses for indexed resources.
//! - `render_path(&Path, &str, &Path)` is the loose-file preview:
//!   it renders a file on disk that the projection has not indexed
//!   yet, so the user can still preview loose attachments (PDFs,
//!   images, archives, …) from the left-column files panel.
//!
//! Both go through `render_kind_inner`, which dispatches by
//! extension. Supported kinds:
//!
//! - **Markdown** (`.md`/`.markdown`) via `pulldown-cmark`.
//! - **Org-mode** (`.org`) as a `<pre>`-escaped block (the real
//!   org → HTML converter is in `notez_core::preview` but not
//!   wired in here yet).
//! - **Image** (`.png`/`.jpg`/`.jpeg`/`.gif`/`.svg`/`.webp`):
//!   rendered inline via `<img>`.
//! - **PDF** (`.pdf`): text extraction via the catalog previewer
//!   plus a 600px-tall `<iframe>` so the user can scroll through
//!   the document.
//! - **Excel** (`.xlsx`/`.xls`): inlined tables via the XLSX
//!   previewer.
//! - **Anything else**: text preview (≤ 64 KB) or a download link
//!   to the raw endpoint.

use std::path::{Path, PathBuf};

use notez_core::preview::Previewer;
use pulldown_cmark::{html, Options, Parser};

use crate::model::ResourceRow;

/// Render the body of `row` for the detail page.
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
    render_kind_inner(
        &file_path,
        &bytes,
        &ext,
        &html_escape::encode_safe(&row.title).into_owned(),
        &raw_url,
        row.kind == "attachment",
    )
}

/// Render an arbitrary on-disk file with the same logic the
/// resource detail page uses. `title` is shown in the fallback
/// link; `space_root` is the parent directory used to compute the
/// `locator` portion of the raw-attachment URL.
pub fn render_path(file_path: &Path, title: &str, space_root: &Path) -> String {
    let bytes = match std::fs::read(file_path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let rel_locator = file_path
        .strip_prefix(space_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| file_path.to_string_lossy().into_owned());
    let raw_url = format!(
        "/api/spaces/attachment/raw?space_root={}&locator={}",
        urlencoding::encode(&space_root.to_string_lossy()),
        urlencoding::encode(&rel_locator)
    );
    render_kind_inner(
        file_path,
        &bytes,
        &ext,
        &html_escape::encode_safe(title).into_owned(),
        &raw_url,
        true,
    )
}

fn render_kind_inner(
    file_path: &Path,
    bytes: &[u8],
    ext: &str,
    title: &str,
    raw_url: &str,
    is_attachment: bool,
) -> String {
    match ext {
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => {
            format!(
                "<div class=\"preview-img\"><img src=\"{raw_url}\" alt=\"{title}\" style=\"max-width:100%;height:auto;\" /></div>"
            )
        }
        "pdf" => render_pdf(file_path, bytes, title, raw_url),
        "xlsx" | "xls" => render_xlsx(file_path, bytes, raw_url),
        "pptx" | "ppt" => {
            format!("<div class=\"preview-download\"><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Download / Open PowerPoint ({title})</a></div>")
        }
        "zip" | "tar" | "tgz" => {
            format!("<div class=\"preview-download\"><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Download Archive ({title})</a></div>")
        }
        "md" | "markdown" => std::str::from_utf8(bytes)
            .map(render_markdown)
            .unwrap_or_default(),
        "org" => std::str::from_utf8(bytes)
            .map(render_org)
            .unwrap_or_default(),
        _ => render_fallback_dispatch(bytes, ext, raw_url, title, is_attachment),
    }
}

fn render_pdf(file_path: &Path, bytes: &[u8], title: &str, raw_url: &str) -> String {
    let previewer = notez_core::preview::builders::pdf::PdfPreviewer;
    let mut catalog = notez_core::preview::PreviewerCatalog::new();
    catalog.register(notez_core::preview::builders::pdf::PdfPreviewer);
    let placeholder = placeholder_row(file_path, title);
    let ctx = notez_core::preview::PreviewContext {
        resource: placeholder.to_domain_resource(),
        bytes: Some(bytes::Bytes::copy_from_slice(bytes)),
        mime: Some("application/pdf".into()),
        locator: file_path.to_path_buf(),
        segments: vec![],
        siblings: vec![],
        catalog: &catalog,
    };
    let text_preview = if let Ok(notez_core::preview::PreviewModel::Pdf { text, .. }) = previewer.render(&ctx) {
        if text.trim().is_empty() {
            String::new()
        } else {
            format!(
                "<details><summary class=\"mono-sm\">Extracted PDF Text</summary><pre class=\"raw\">{}</pre></details>",
                html_escape::encode_safe(&text)
            )
        }
    } else {
        String::new()
    };
    format!(
        "<div class=\"preview-pdf\"><p><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📄 Open PDF in new tab ({title})</a></p>{text_preview}<iframe src=\"{raw_url}\" width=\"100%\" height=\"600px\" style=\"border:1px solid var(--ink-rule);margin-top:0.5rem;\"></iframe></div>"
    )
}

fn render_xlsx(file_path: &Path, bytes: &[u8], raw_url: &str) -> String {
    let previewer = notez_core::preview::builders::xlsx::XlsxPreviewer;
    let mut catalog = notez_core::preview::PreviewerCatalog::new();
    catalog.register(notez_core::preview::builders::xlsx::XlsxPreviewer);
    let placeholder = placeholder_row(file_path, "");
    let ctx = notez_core::preview::PreviewContext {
        resource: placeholder.to_domain_resource(),
        bytes: Some(bytes::Bytes::copy_from_slice(bytes)),
        mime: Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into()),
        locator: file_path.to_path_buf(),
        segments: vec![],
        siblings: vec![],
        catalog: &catalog,
    };
    if let Ok(notez_core::preview::PreviewModel::Xlsx { sheets }) = previewer.render(&ctx) {
        let mut html = String::from("<div class=\"preview-xlsx\">");
        for sheet in sheets {
            html.push_str(&format!(
                "<h3>Sheet: {}</h3><table class=\"preview-table\" style=\"border-collapse:collapse;width:100%;margin-bottom:1rem;\">",
                html_escape::encode_safe(&sheet.name)
            ));
            for r in sheet.rows {
                html.push_str("<tr>");
                for cell in r {
                    html.push_str(&format!(
                        "<td style=\"border:1px solid var(--ink-rule);padding:0.2rem 0.5rem;\">{}</td>",
                        html_escape::encode_safe(&cell)
                    ));
                }
                html.push_str("</tr>");
            }
            html.push_str("</table>");
        }
        html.push_str("</div>");
        return html;
    }
    format!("<div class=\"preview-download\"><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Download Excel File</a></div>")
}

fn render_fallback_dispatch(
    bytes: &[u8],
    ext: &str,
    raw_url: &str,
    title: &str,
    is_attachment: bool,
) -> String {
    let label = if ext.is_empty() { "file" } else { ext };
    if let Ok(text) = std::str::from_utf8(bytes) {
        if text.len() > 64 * 1024 {
            return format!(
                "<div class=\"preview-download\"><p class=\"mono-sm\">{} preview — {} KB, too long to inline.</p><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Open {} ({title})</a></div>",
                label.to_uppercase(),
                text.len() / 1024,
                label,
            );
        }
        format!(
            "<div class=\"preview-text\"><p class=\"mono-sm\">{} preview — <a href=\"{raw_url}\" target=\"_blank\">open raw</a></p><pre class=\"raw\">{}</pre></div>",
            label.to_uppercase(),
            html_escape::encode_safe(text),
        )
    } else {
        let target = if is_attachment { "target=\"_blank\"" } else { "" };
        format!(
            "<div class=\"preview-download\"><a href=\"{raw_url}\" {target} class=\"spine-action\">📥 Download {label} ({title})</a></div>"
        )
    }
}

fn placeholder_row(file_path: &Path, title: &str) -> ResourceRow {
    ResourceRow::new(
        notez_core::domain::ResourceRef::new(
            notez_core::domain::ResourceKind::Attachment,
            ulid::Ulid::new(),
        ),
        notez_core::domain::ResourceKind::Attachment,
        title.to_string(),
        String::new(),
        file_path.to_string_lossy().into_owned(),
        String::new(),
        String::new(),
        Default::default(),
        String::new(),
    )
}

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

fn render_org(text: &str) -> String {
    let fenced = format!("```org\n{text}\n```");
    let parser = Parser::new(&fenced);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
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
    use std::collections::BTreeMap;

    fn row_with_kind_locator(kind: &str, locator: &str) -> ResourceRow {
        let mut properties = BTreeMap::new();
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
        assert!(body.contains("TODO") || body.contains("&lt;"));
    }

    #[test]
    fn render_path_picks_up_png() {
        let dir = tempfile::tempdir().unwrap();
        // Tiny valid PNG (1x1 transparent).
        let bytes: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
            0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
            0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
            0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
            0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let file = dir.path().join("dot.png");
        std::fs::write(&file, &bytes).unwrap();
        let html = render_path(&file, "dot.png", dir.path());
        assert!(html.contains("<img"), "expected img tag: {html}");
    }
}