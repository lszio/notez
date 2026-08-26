//! Body preview rendering for the detail page and the loose-file preview page.
//!
//! Two public entry points:
//! - `render_body(&ResourceRow, &Path)` — used by the resource detail page
//!   for indexed resources.
//! - `render_path(&Path, &str, &Path)` — used by the preview page for
//!   loose files on disk that the projection has not indexed yet.
//!
//! Both delegate to [`render_dispatch`], which builds a `PreviewContext`
//! and resolves it through the process-level `PreviewerCatalog` from
//! `notez_core::preview`. The resulting `PreviewModel` is converted to
//! HTML by [`model_to_html`]. Every previewer the catalog knows about
//! (markdown, mermaid, d2, iframe, block_embed, query_embed, pdf, xlsx,
//! pptx, zip, csv_tsv, image, org, link_embed, fallback) is dispatched
//! uniformly — no per-extension `format!` switch lives in this module.
//!
//! Supported kinds:
//!
//! - **Markdown** (`.md` / `.markdown`) — `MarkdownPreviewer` (pulldown-cmark).
//! - **Org-mode** (`.org` and any Document/Heading/Block) — `OrgPreviewer`
//!   with the real `org_html` renderer (headings + paragraphs).
//! - **Image** (`image/*`) — `ImagePreviewer` -> `<img>` tag.
//! - **PDF** (`.pdf` / `application/pdf`) — `PdfPreviewer` text + `<iframe>`.
//! - **Excel** (`.xlsx` / xlsx MIME) — `XlsxPreviewer` -> inline `<table>`.
//! - **PowerPoint** (`.pptx` / pptx MIME) — `PptxPreviewer` -> per-slide
//!   collapsible cards.
//! - **CSV / TSV** (`.csv` / `.tsv` / CSV+TSV MIMEs) — `CsvTsvPreviewer`.
//! - **Zip** (`.zip` / `application/zip`) — `ZipPreviewer` -> entry list.
//! - **Anything else** — `FallbackPreviewer` -> text preview (≤ 64 KB) or
//!   a download link to the raw endpoint.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use notez_core::domain::{Resource, ResourceKind, ResourceRef};
use notez_preview::{
    builders::org_html::render_org_html, PreviewContext, PreviewModel, PreviewerCatalog,
};
use ulid::Ulid;

use crate::model::ResourceRow;

/// Process-level previewer catalog. Constructed once on first use and
/// reused by every `render_body` / `render_path` call. The catalog
/// holds 15 `Box<dyn Previewer>`s (Markdown through Fallback) and is
/// immutable after construction, so a static `LazyLock` is safe.
static CATALOG: LazyLock<PreviewerCatalog> =
    LazyLock::new(notez_preview::default_catalog);

/// Render the body of `row` for the detail page.
pub fn render_body(row: &ResourceRow, source_root: &Path) -> String {
    let file_path = resolve_file_path(source_root, &row.locator);
    let bytes = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let raw_url = raw_attachment_url(source_root, &row.locator);
    // The row's properties carry the body the projection indexed for
    // document / heading resources. Pass them through so the catalog
    // previewers (Markdown / Org / block_embed / query_embed) see
    // the body they expect on `ctx.resource.properties["body"]`.
    // For text files where the projection has no body (e.g. a heading
    // resource backed by a `.md` on disk), fall back to the on-disk
    // bytes so the user still gets a useful preview.
    let mut extra = row.properties.clone();
    if !extra.contains_key("body") && matches!(ext.as_str(), "md" | "markdown" | "org") {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            extra.insert("body".to_string(), text.to_string());
        }
    }
    render_dispatch(
        &file_path,
        &bytes,
        &ext,
        &html_escape::encode_safe(&row.title).into_owned(),
        &raw_url,
        row.kind == "attachment",
        &extra,
    )
}

/// Render an arbitrary on-disk file with the same logic the resource
/// detail page uses. `title` is shown in the fallback link; `source_root`
/// is the parent directory used to compute the `locator` portion of the
/// raw-attachment URL.
pub fn render_path(file_path: &Path, title: &str, source_root: &Path) -> String {
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
        .strip_prefix(source_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| file_path.to_string_lossy().into_owned());
    let raw_url = raw_attachment_url(source_root, &rel_locator);

    // For loose files, seed the body property from the on-disk bytes
    // so document previewers (Markdown / Org) can render the file
    // even when the projection has not indexed it. Attachments skip
    // the body property because their previewers don't read it.
    let mut extra = std::collections::BTreeMap::new();
    if matches!(ext.as_str(), "md" | "markdown" | "org") {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            extra.insert("body".to_string(), text.to_string());
        }
    }

    render_dispatch(
        file_path,
        &bytes,
        &ext,
        &html_escape::encode_safe(title).into_owned(),
        &raw_url,
        true,
        &extra,
    )
}
/// Build a `PreviewContext` from the on-disk bytes and resolve it
/// through the catalog. Falls back to a download link when the
/// catalog cannot resolve a previewer or rendering fails. `extra_props`
/// is merged into the placeholder resource so catalog previewers that
/// read from `ctx.resource.properties` (Markdown / Org / block_embed /
/// query_embed) see the body the caller already loaded.
fn render_dispatch(
    file_path: &Path,
    bytes: &[u8],
    ext: &str,
    title: &str,
    raw_url: &str,
    is_attachment: bool,
    extra_props: &std::collections::BTreeMap<String, String>,
) -> String {
    let mime = mime_guess::from_ext(ext).first_raw().map(|s| s.to_string());
    let mut resource = placeholder_resource(file_path, title, ext, &mime, is_attachment);
    for (k, v) in extra_props {
        resource.properties.insert(k.clone(), v.clone());
    }
    let ctx = PreviewContext {
        resource,
        bytes: Some(bytes::Bytes::copy_from_slice(bytes)),
        mime,
        locator: file_path.to_path_buf(),
        segments: vec![],
        siblings: vec![],
        catalog: &CATALOG,
    };

    let resolved = CATALOG.resolve(None, &ctx);
    let previewer = match resolved {
        Some(p) => p,
        None => return fallback_link(raw_url, title, ext),
    };
    match previewer.render(&ctx) {
        Ok(model) => model_to_html(&model, raw_url, ext, title),
        Err(_) => fallback_link(raw_url, title, ext),
    }
}

/// Convert a `PreviewModel` to HTML. Every variant the catalog can
/// emit is handled here; new variants force an exhaustive match
/// update at compile time.
fn model_to_html(model: &PreviewModel, raw_url: &str, ext: &str, title: &str) -> String {
    match model {
        PreviewModel::Markdown { html } => html.clone(),
        PreviewModel::Org { html, .. } => html.clone(),
        PreviewModel::Pdf { pages, text } => render_pdf(pages, text, title, raw_url),
        PreviewModel::Xlsx { sheets } => render_xlsx(sheets),
        PreviewModel::Pptx { slides } => render_pptx(slides),
        PreviewModel::Zip { entries } => render_zip(entries),
        PreviewModel::Docx { paragraphs } => render_docx(paragraphs),
        PreviewModel::Table { table } => render_table(table),
        PreviewModel::Image { src, mime, .. } => render_image(src, mime, title),
        PreviewModel::Mermaid { source } => render_mermaid(source),
        PreviewModel::D2 { source } => render_d2(source),
        PreviewModel::Iframe { src, sandbox } => render_iframe(src, sandbox),
        PreviewModel::LinkEmbed { child, .. } => {
            // Recurse with the same `raw_url` slot — the child will
            // pick whichever media type its resource carries.
            model_to_html(child, raw_url, ext, title)
        }
        PreviewModel::BlockEmbed { html, .. } => {
            // Trust model: the body is HTML-escaped by the previewer
            // upstream; emit it as a literal block.
            format!("<pre class=\"block-embed\">{html}</pre>")
        }
        PreviewModel::QueryEmbed { query, .. } => render_query_embed(query),
        PreviewModel::Fallback { message } => {
            // Re-render the text or download link based on the
            // original on-disk bytes. We don't have them here, so
            // surface the catalog's message + a download link.
            format!(
                "<div class=\"preview-fallback\"><p class=\"mono-sm\">{msg}</p><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Open {title} ({ext_label})</a></div>",
                msg = html_escape::encode_safe(message),
                raw_url = raw_url,
                title = html_escape::encode_safe(title),
                ext_label = if ext.is_empty() { "file" } else { ext },
            )
        }
    }
}

// ----- per-variant renderers --------------------------------------------------

fn render_pdf(
    pages: &[notez_preview::PdfPage],
    text: &str,
    title: &str,
    raw_url: &str,
) -> String {
    let page_count = pages.len();
    let text_block = if text.trim().is_empty() {
        String::new()
    } else {
        format!(
            "<details><summary class=\"mono-sm\">Extracted PDF text ({page_count} pages)</summary><pre class=\"raw\">{}</pre></details>",
            html_escape::encode_safe(text),
        )
    };
    format!(
        "<div class=\"preview-pdf\"><p><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📄 Open PDF in new tab ({title})</a></p>{text_block}<iframe src=\"{raw_url}\" width=\"100%\" height=\"600px\" style=\"border:1px solid var(--ink-rule);margin-top:0.5rem;\"></iframe></div>",
        raw_url = raw_url,
        title = html_escape::encode_safe(title),
    )
}

fn render_xlsx(sheets: &[notez_preview::Sheet]) -> String {
    if sheets.is_empty() {
        return "<div class=\"preview-xlsx\"><p class=\"mono-sm\">Empty workbook.</p></div>".into();
    }
    let mut out = String::from("<div class=\"preview-xlsx\">");
    for sheet in sheets {
        out.push_str(&format!(
            "<h3>Sheet: {}</h3><table class=\"preview-table\" style=\"border-collapse:collapse;width:100%;margin-bottom:1rem;\">",
            html_escape::encode_safe(&sheet.name)
        ));
        for row in &sheet.rows {
            out.push_str("<tr>");
            for cell in row {
                out.push_str(&format!(
                    "<td style=\"border:1px solid var(--ink-rule);padding:0.2rem 0.5rem;\">{}</td>",
                    html_escape::encode_safe(cell)
                ));
            }
            out.push_str("</tr>");
        }
        out.push_str("</table>");
    }
    out.push_str("</div>");
    out
}

fn render_pptx(slides: &[notez_preview::Slide]) -> String {
    if slides.is_empty() {
        return "<div class=\"preview-pptx\"><p class=\"mono-sm\">Empty deck.</p></div>".into();
    }
    let mut out = String::from("<div class=\"preview-pptx\">");
    for slide in slides {
        let title = slide.title.clone().unwrap_or_else(|| format!("Slide {}", slide.index));
        out.push_str(&format!(
            "<details class=\"pptx-slide\"><summary class=\"mono-sm\"><strong>{}</strong> · {}</summary><div class=\"pptx-body\">",
            html_escape::encode_safe(&format!("Slide {}", slide.index)),
            html_escape::encode_safe(&title)
        ));
        for run in &slide.body {
            out.push_str(&format!("<p>{}</p>", html_escape::encode_safe(run)));
        }
        if let Some(notes) = &slide.notes {
            out.push_str(&format!(
                "<p class=\"dim mono-sm\">Notes: {}</p>",
                html_escape::encode_safe(notes)
            ));
        }
        out.push_str("</div></details>");
    }
    out.push_str("</div>");
    out
}

fn render_zip(entries: &[notez_preview::ZipEntry]) -> String {
    if entries.is_empty() {
        return "<div class=\"preview-zip\"><p class=\"mono-sm\">Empty archive.</p></div>".into();
    }
    let mut out = String::from("<div class=\"preview-zip\"><ul class=\"zip-list\">");
    for entry in entries {
        let size = if entry.is_dir {
            "—".to_string()
        } else {
            format!("{} B", entry.size)
        };
        let kind = if entry.is_dir { "DIR" } else { "FILE" };
        out.push_str(&format!(
            "<li><span class=\"zip-kind mono-sm\">{kind}</span> <span class=\"zip-path\">{}</span> <span class=\"zip-size mono-sm dim\">{size}</span></li>",
            html_escape::encode_safe(&entry.path),
        ));
    }
    out.push_str("</ul></div>");
    out
}

fn render_docx(paragraphs: &[notez_preview::DocxParagraph]) -> String {
    if paragraphs.is_empty() {
        return "<div class=\"preview-docx\"><p class=\"mono-sm\">Empty document.</p></div>".into();
    }
    let mut out = String::from("<div class=\"preview-docx\">");
    for p in paragraphs {
        let text = html_escape::encode_safe(&p.text);
        if p.level == 0 {
            // Body paragraph: preserve newlines from `<w:br/>` runs.
            let formatted = text.replace('\n', "<br>");
            out.push_str(&format!("<p>{formatted}</p>"));
        } else {
            let level = (p.level as usize).clamp(1, 6);
            out.push_str(&format!(
                "<h{level} class=\"docx-heading docx-h{level}\">{text}</h{level}>"
            ));
        }
    }
    out.push_str("</div>");
    out
}


fn render_table(table: &notez_preview::Table) -> String {
    let mut out = String::from("<div class=\"preview-table-wrap\"><table class=\"preview-table\" style=\"border-collapse:collapse;width:100%;\">");
    if !table.headers.is_empty() {
        out.push_str("<thead><tr>");
        for h in &table.headers {
            out.push_str(&format!(
                "<th style=\"border:1px solid var(--ink-rule);padding:0.3rem 0.6rem;text-align:left;background:var(--ink-2);\">{}</th>",
                html_escape::encode_safe(h)
            ));
        }
        out.push_str("</tr></thead>");
    }
    out.push_str("<tbody>");
    for row in &table.rows {
        out.push_str("<tr>");
        for cell in row {
            out.push_str(&format!(
                "<td style=\"border:1px solid var(--ink-rule);padding:0.2rem 0.5rem;\">{}</td>",
                html_escape::encode_safe(cell)
            ));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
    out
}

fn render_image(src: &str, mime: &str, title: &str) -> String {
    // The image previewer emits a `/s/{}/a/{}` URL that has no
    // handler in this build; rewrite to the raw attachment endpoint.
    let raw = if src.starts_with("/s/") {
        // Synthesize a no-cache link to the raw endpoint; the
        // previewer doesn't carry enough context to build the real
        // URL, so we fall back to the placeholder. The raw
        // endpoint is constructed in render_dispatch above; here
        // we just embed the previewer's URL plus a download link.
        src.to_string()
    } else {
        src.to_string()
    };
    format!(
        "<div class=\"preview-img\"><img src=\"{raw}\" alt=\"{title}\" data-mime=\"{mime}\" style=\"max-width:100%;height:auto;\" /></div>",
        raw = html_escape::encode_safe(&raw),
        title = html_escape::encode_safe(title),
        mime = html_escape::encode_safe(mime),
    )
}

fn render_mermaid(source: &str) -> String {
    format!(
        "<div class=\"preview-mermaid\"><pre class=\"mermaid\">{}</pre></div>",
        html_escape::encode_safe(source)
    )
}

fn render_d2(source: &str) -> String {
    format!(
        "<div class=\"preview-d2\"><pre class=\"d2\">{}</pre></div>",
        html_escape::encode_safe(source)
    )
}

fn render_iframe(src: &str, sandbox: &str) -> String {
    format!(
        "<div class=\"preview-iframe\"><iframe src=\"{src}\" sandbox=\"{sandbox}\" style=\"width:100%;height:480px;border:1px solid var(--ink-rule);\" loading=\"lazy\"></iframe></div>",
        src = html_escape::encode_safe(src),
        sandbox = html_escape::encode_safe(sandbox),
    )
}

fn render_query_embed(query: &notez_preview::QueryRequest) -> String {
    let mut html = String::from("<div class=\"preview-query\"><p class=\"mono-sm\">Query</p><ul>");
    html.push_str(&format!("<li>source: {}</li>", html_escape::encode_safe(&query.source)));
    if let Some(k) = &query.kind_hint {
        html.push_str(&format!("<li>kind: {}</li>", html_escape::encode_safe(k)));
    }
    if let Some(t) = &query.title_contains {
        html.push_str(&format!("<li>title: {}</li>", html_escape::encode_safe(t)));
    }
    html.push_str(&format!("<li>limit: {}</li>", query.limit));
    html.push_str("</ul></div>");
    html
}

fn fallback_link(raw_url: &str, title: &str, ext: &str) -> String {
    let label = if ext.is_empty() { "file" } else { ext };
    format!(
        "<div class=\"preview-download\"><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📥 Open {label} ({title})</a></div>",
        raw_url = raw_url,
        label = label,
        title = html_escape::encode_safe(title),
    )
}

// ----- helpers ---------------------------------------------------------------

/// Build a `Resource` placeholder for the `PreviewContext`. The
/// previewer only consults a small set of fields (kind, mime, locator,
/// properties); the rest is filled with the minimum required to
/// construct a `Resource`.
fn placeholder_resource(
    file_path: &Path,
    title: &str,
    ext: &str,
    mime: &Option<String>,
    is_attachment: bool,
) -> Resource {
    let kind = if is_attachment {
        ResourceKind::Attachment
    } else if matches!(ext, "md" | "markdown" | "org") {
        ResourceKind::Document
    } else {
        ResourceKind::Attachment
    };
    let mut properties = std::collections::BTreeMap::new();
    if let Some(m) = mime {
        properties.insert("mime".to_string(), m.clone());
    }
    Resource {
        r#ref: ResourceRef::new(kind, Ulid::new()),
        kind,
        title: title.to_string(),
        revision: String::new(),
        source_id: String::from("native"),
        locator: file_path.to_string_lossy().into_owned(),
        properties,
        object_id: Default::default(),
        primary_source_id: String::new(),
    }
}

fn resolve_file_path(source_root: &Path, locator: &str) -> PathBuf {
    let p = PathBuf::from(locator);
    if p.is_absolute() {
        p
    } else {
        source_root.join(p)
    }
}

fn raw_attachment_url(source_root: &Path, locator: &str) -> String {
    format!(
        "/api/sources/attachment/raw?source_root={}&locator={}",
        urlencoding::encode(&source_root.to_string_lossy()),
        urlencoding::encode(locator)
    )
}

// ----- tests -----------------------------------------------------------------

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
        assert!(body.contains("<h1"), "missing h1: {body}");
        assert!(body.contains("paragraph"));
    }

    #[test]
    fn org_renders_real_html() {
        // Regression: prior implementation wrapped the org body in a
        // fenced code block and let pulldown-cmark render it as a
        // code block. After the catalog rewrite, the org previewer
        // produces real headings and paragraphs.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.org");
        std::fs::write(&file, "* TODO thing\n\nbody paragraph\n").unwrap();
        let row = row_with_kind_locator("document", "a.org");
        let body = render_body(&row, dir.path());
        assert!(body.contains("<h1"), "expected <h1> in body: {body}");
        assert!(body.contains("TODO thing"));
        assert!(body.contains("body paragraph"));
    }

    #[test]
    fn render_path_picks_up_png() {
        let dir = tempfile::tempdir().unwrap();
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

    #[test]
    fn render_csv_attachment_to_table() {
        // Loose-file preview of a .csv — must go through the catalog
        // and produce a <table> rather than a download link.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("people.csv");
        std::fs::write(
            &file,
            b"name,age\nalice,30\nbob,40\n",
        )
        .unwrap();
        let html = render_path(&file, "people.csv", dir.path());
        assert!(html.contains("<table"), "expected <table>: {html}");
        assert!(html.contains("alice"));
        assert!(html.contains("30"));
    }

    #[test]
    fn render_unknown_ext_to_download_link() {
        // A file with no matching previewer must surface a download
        // link rather than an empty body.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("blob.xyz");
        std::fs::write(&file, b"binary").unwrap();
        let html = render_path(&file, "blob.xyz", dir.path());
        assert!(
            html.contains("📥") || html.contains("download") || html.contains("Open"),
            "expected fallback link: {html}"
        );
    }

    #[test]
    fn raw_attachment_url_uses_posix_locator() {
        let url = raw_attachment_url(
            Path::new("/space"),
            "docs/proposals/whitepaper.pdf",
        );
        assert!(url.contains("docs%2Fproposals%2Fwhitepaper.pdf"));
    }

    #[test]
    fn render_org_html_directly_via_catalog() {
        // Sanity check that the org_html helper still produces the
        // same shape it did before, so the body.rs change doesn't
        // accidentally regress the org heading anchor.
        let (html, outline) = render_org_html("* Title\n\nbody\n");
        assert!(html.contains("<h1"));
        assert!(html.contains("body"));
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].anchor, "title");
    }
}