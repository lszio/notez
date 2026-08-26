//! Previewer for Microsoft Word (DOCX) attachments.
//!
//! Matches on a `.docx` locator suffix or the
//! `application/vnd.openxmlformats-officedocument.wordprocessingml.document`
//! MIME. The full byte payload must be present in `ctx.bytes`; if it is
//! missing the render step returns `PreviewError::MissingBytes`.
//!
//! A DOCX file is a ZIP archive whose `word/document.xml` member holds
//! the body content as a sequence of `<w:p>` paragraphs. For every
//! paragraph we:
//!
//! 1. Look at the first `<w:pStyle w:val="…"/>` child to detect a
//!    heading (`Heading1`..`Heading9` map to level 1..6; `Title` maps
//!    to level 1; the absent or default style maps to level 0 = body).
//! 2. Concatenate every `<w:t>` text run inside the paragraph into
//!    `text`. `<w:tab/>` becomes a single space and `<w:br/>` becomes
//!    a newline, regardless of whether they appear inside or between
//!    `<w:t>` runs.
//!
//! The limit of `MAX_PARAGRAPHS` truncates very large documents so a
//! pathological DOCX cannot exhaust memory. The cap is generous
//! (10 000) — most documents are far shorter.
//!
//! XML parse errors are surfaced to the caller rather than swallowed,
//! because a truncated / malformed document is a real bug, not an
//! empty result. ZIP errors (the bytes are not a DOCX container or
//! `word/document.xml` is missing) are also surfaced.

use crate::{DocxParagraph, PreviewContext, PreviewError, PreviewModel, Previewer};
use bytes::Bytes;
use quick_xml::Reader;
use quick_xml::events::Event;
use quick_xml::name::QName;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Maximum number of paragraphs rendered before truncation.
const MAX_PARAGRAPHS: usize = 10_000;

/// Previewer for DOCX attachments.
pub struct DocxPreviewer;

impl Previewer for DocxPreviewer {
    fn id(&self) -> &'static str {
        "docx"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".docx")
            || ctx.mime.as_deref()
                == Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes: Bytes = ctx
            .bytes
            .clone()
            .ok_or_else(|| PreviewError::MissingBytes(ctx.resource.r#ref.to_string()))?;

        let xml = read_document_xml(&bytes)?;
        let paragraphs = parse_paragraphs(&xml)?;

        Ok(PreviewModel::Docx { paragraphs })
    }
}

/// Open the DOCX zip and read `word/document.xml` into a UTF-8 string.
fn read_document_xml(bytes: &[u8]) -> Result<String, PreviewError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| PreviewError::Extraction(format!("docx zip: {e}")))?;

    let mut entry = archive
        .by_name("word/document.xml")
        .map_err(|e| PreviewError::Extraction(format!("docx entry word/document.xml: {e}")))?;

    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .map_err(|e| PreviewError::Extraction(format!("docx read: {e}")))?;
    Ok(xml)
}

/// Walk the document XML and split it into a list of `DocxParagraph`.
/// Returns the first `MAX_PARAGRAPHS` paragraphs; the rest are
/// silently dropped.
fn parse_paragraphs(xml: &str) -> Result<Vec<DocxParagraph>, PreviewError> {
    let mut reader = Reader::from_str(xml);
    // trim_text(false) preserves internal whitespace — DOCX authors
    // rely on this for things like "term: 12 months" where the run
    // boundary sits inside a word. We trim leading/trailing whitespace
    // of the final paragraph instead.
    reader.config_mut().trim_text(false);

    let mut paragraphs: Vec<DocxParagraph> = Vec::new();
    let mut current: Option<ParagraphBuilder> = None;
    let mut buf = Vec::new();

    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| PreviewError::Extraction(format!("docx xml: {e}")))?
        {
            Event::Start(e) if is_tag(&e.name(), b"w:p") => {
                current = Some(ParagraphBuilder::new());
            }
            Event::Start(e) if is_tag(&e.name(), b"w:pStyle") => {
                style_to_level(&e, current.as_mut());
            }
            Event::Empty(e) if is_tag(&e.name(), b"w:pStyle") => {
                style_to_level(&e, current.as_mut());
            }
            Event::Start(e) if is_tag(&e.name(), b"w:t") => {
                if let Some(p) = current.as_mut() {
                    p.in_text = true;
                }
            }
            Event::Text(t) => {
                if let Some(p) = current.as_mut() {
                    if p.in_text {
                        let text = t
                            .unescape()
                            .map_err(|e| PreviewError::Extraction(format!("docx text: {e}")))?;
                        p.text.push_str(&text);
                    }
                }
            }
            Event::Empty(e) if is_tag(&e.name(), b"w:tab") => {
                if let Some(p) = current.as_mut() {
                    p.text.push(' ');
                }
            }
            Event::Empty(e) if is_tag(&e.name(), b"w:br") => {
                if let Some(p) = current.as_mut() {
                    p.text.push('\n');
                }
            }
            Event::End(e) if is_tag(&e.name(), b"w:t") => {
                if let Some(p) = current.as_mut() {
                    p.in_text = false;
                }
            }
            Event::End(e) if is_tag(&e.name(), b"w:p") => {
                if let Some(p) = current.take() {
                    if !p.text.is_empty() || p.level > 0 {
                        paragraphs.push(p.finish());
                        if paragraphs.len() >= MAX_PARAGRAPHS {
                            break;
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    // Flush a trailing unclosed paragraph (lenient: Word always closes
    // them, but we shouldn't refuse an otherwise valid document).
    if let Some(p) = current.take() {
        if !p.text.is_empty() || p.level > 0 {
            paragraphs.push(p.finish());
        }
    }

    Ok(paragraphs)
}

fn style_to_level(e: &quick_xml::events::BytesStart, current: Option<&mut ParagraphBuilder>) {
    if let Some(p) = current {
        if let Some(val) = attr(e, b"w:val") {
            p.level = parse_heading_level(&val);
        }
    }
}

/// Compare a `QName`'s bytes against an expected `w:`-prefixed tag.
fn is_tag(name: &QName, expected: &[u8]) -> bool {
    name.as_ref() == expected
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    for a in e.attributes() {
        let a = match a {
            Ok(a) => a,
            Err(_) => continue,
        };
        if a.key.as_ref() == key {
            let value = a.value.into_owned();
            return String::from_utf8(value).ok();
        }
    }
    None
}

/// Map a `w:pStyle` value to a heading level. Returns 0 for the
/// default style. Unknown styles collapse to level 0 (body); the
/// paragraph is still emitted but without a heading wrapper.
fn parse_heading_level(val: &str) -> u8 {
    let v = val.trim();
    if v.eq_ignore_ascii_case("Title") || v.eq_ignore_ascii_case("Subtitle") {
        return 1;
    }
    if let Some(rest) = v.strip_prefix("Heading") {
        if let Some(idx) = rest.find(|c: char| !c.is_ascii_digit()) {
            let n = &rest[..idx];
            if let Ok(n) = n.parse::<u8>() {
                return n.clamp(1, 6);
            }
        } else if let Ok(n) = rest.parse::<u8>() {
            return n.clamp(1, 6);
        }
    }
    0
}

/// In-progress state for a single `<w:p>` element.
struct ParagraphBuilder {
    level: u8,
    text: String,
    in_text: bool,
}

impl ParagraphBuilder {
    fn new() -> Self {
        Self {
            level: 0,
            text: String::new(),
            in_text: false,
        }
    }

    fn finish(self) -> DocxParagraph {
        DocxParagraph {
            level: self.level,
            text: self.text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::domain::{Resource, ResourceKind, ResourceRef};
    use crate::PreviewerCatalog;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn ctx<'a>(
        bytes: &'a [u8],
        locator: PathBuf,
        mime: &'a str,
        catalog: &'a PreviewerCatalog,
    ) -> PreviewContext<'a> {
        let resource = Resource {
            r#ref: ResourceRef::new(ResourceKind::Attachment, ulid::Ulid::new()),
            kind: ResourceKind::Attachment,
            title: String::new(),
            revision: String::new(),
            source_id: String::from("native"),
            locator: locator.to_string_lossy().into_owned(),
            properties: BTreeMap::new(),
            object_id: Default::default(),
            primary_source_id: String::new(),
        };
        PreviewContext {
            resource,
            bytes: Some(Bytes::copy_from_slice(bytes)),
            mime: Some(mime.to_string()),
            locator,
            segments: Vec::new(),
            siblings: Vec::new(),
            catalog,
        }
    }

    #[test]
    fn matches_docx_locator() {
        let cat = PreviewerCatalog::new();
        let c = ctx(
            b"",
            PathBuf::from("file.docx"),
            "application/octet-stream",
            &cat,
        );
        assert!(DocxPreviewer.matches(&c));
    }

    #[test]
    fn matches_docx_mime() {
        let cat = PreviewerCatalog::new();
        let c = ctx(
            b"",
            PathBuf::from("file.bin"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            &cat,
        );
        assert!(DocxPreviewer.matches(&c));
    }

    #[test]
    fn rejects_other_extensions() {
        let cat = PreviewerCatalog::new();
        let c = ctx(b"", PathBuf::from("file.doc"), "application/msword", &cat);
        assert!(!DocxPreviewer.matches(&c));
    }

    #[test]
    fn missing_bytes_surfaces_error() {
        let cat = PreviewerCatalog::new();
        let resource = Resource {
            r#ref: ResourceRef::new(ResourceKind::Attachment, ulid::Ulid::new()),
            kind: ResourceKind::Attachment,
            title: String::new(),
            revision: String::new(),
            source_id: String::from("native"),
            locator: String::from("file.docx"),
            properties: BTreeMap::new(),
            object_id: Default::default(),
            primary_source_id: String::new(),
        };
        let c = PreviewContext {
            resource,
            bytes: None,
            mime: Some(String::from(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            )),
            locator: PathBuf::from("file.docx"),
            segments: Vec::new(),
            siblings: Vec::new(),
            catalog: &cat,
        };
        let err = DocxPreviewer.render(&c).unwrap_err();
        assert!(matches!(err, PreviewError::MissingBytes(_)), "got {err:?}");
    }

    #[test]
    fn renders_paragraphs_and_headings() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>
    <w:p><w:r><w:t>body one</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Sub</w:t></w:r></w:p>
    <w:p><w:r><w:t>body </w:t><w:t>two</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        let paragraphs = parse_paragraphs(xml).expect("parse ok");
        assert_eq!(paragraphs.len(), 4);
        assert_eq!(paragraphs[0].level, 1);
        assert_eq!(paragraphs[0].text, "Title");
        assert_eq!(paragraphs[1].level, 0);
        assert_eq!(paragraphs[1].text, "body one");
        assert_eq!(paragraphs[2].level, 2);
        assert_eq!(paragraphs[2].text, "Sub");
        assert_eq!(paragraphs[3].level, 0);
        assert_eq!(paragraphs[3].text, "body two");
    }

    #[test]
    fn maps_title_style_to_heading_level_one() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>Cov</w:t></w:r></w:p></w:body>
</w:document>"#;
        let paragraphs = parse_paragraphs(xml).expect("parse ok");
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].level, 1);
        assert_eq!(paragraphs[0].text, "Cov");
    }

    #[test]
    fn unknown_style_collapses_to_level_zero() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:pStyle w:val="Quote"/></w:pPr><w:r><w:t>inspiration</w:t></w:r></w:p></w:body>
</w:document>"#;
        let paragraphs = parse_paragraphs(xml).expect("parse ok");
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].level, 0);
        assert_eq!(paragraphs[0].text, "inspiration");
    }

    #[test]
    fn heading9_clamps_to_level_6() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:pStyle w:val="Heading9"/></w:pPr><w:r><w:t>deep</w:t></w:r></w:p></w:body>
</w:document>"#;
        let paragraphs = parse_paragraphs(xml).expect("parse ok");
        assert_eq!(paragraphs[0].level, 6);
    }

    #[test]
    fn tab_and_break_become_space_and_newline() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t>c</w:t></w:r></w:p></w:body>
</w:document>"#;
        let paragraphs = parse_paragraphs(xml).expect("parse ok");
        assert_eq!(paragraphs[0].text, "a b\nc");
    }

    #[test]
    fn empty_paragraphs_skipped() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p/>
    <w:p><w:r><w:t>kept</w:t></w:r></w:p>
    <w:p/>
  </w:body>
</w:document>"#;
        let paragraphs = parse_paragraphs(xml).expect("parse ok");
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].text, "kept");
    }

    #[test]
    fn heading_only_paragraph_is_kept() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr></w:p></w:body>
</w:document>"#;
        let paragraphs = parse_paragraphs(xml).expect("parse ok");
        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].level, 1);
        assert_eq!(paragraphs[0].text, "");
    }

    #[test]
    fn invalid_zip_bytes_surfaces_error() {
        let cat = PreviewerCatalog::new();
        let c = ctx(
            b"this is not a zip",
            PathBuf::from("file.docx"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            &cat,
        );
        let err = DocxPreviewer.render(&c).unwrap_err();
        assert!(matches!(err, PreviewError::Extraction(_)), "got {err:?}");
    }

    #[test]
    fn missing_document_xml_surfaces_error() {
        let cat = PreviewerCatalog::new();
        // Build a valid ZIP that doesn't contain word/document.xml.
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts: zip::write::FileOptions =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("other.txt", opts).unwrap();
        use std::io::Write;
        zip.write_all(b"hi").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let c = ctx(
            &bytes,
            PathBuf::from("file.docx"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            &cat,
        );
        let err = DocxPreviewer.render(&c).unwrap_err();
        assert!(matches!(err, PreviewError::Extraction(_)), "got {err:?}");
    }
}
