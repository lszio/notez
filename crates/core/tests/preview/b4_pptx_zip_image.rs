//! Tests for the phase-B part-2 previewers: Pptx / Zip / Image.
//!
//! Each test builds a binary fixture in-memory (no external files) and
//! verifies that the previewer both matches the context and renders the
//! expected `PreviewModel` payload.

use bytes::Bytes;
use crate::domain::{Resource, ResourceKind, ResourceRef};
use crate::preview::builders::image::ImagePreviewer;
use crate::preview::builders::pptx::PptxPreviewer;
use crate::preview::builders::zip::ZipPreviewer;
use crate::preview::{PreviewContext, PreviewModel, Previewer, PreviewerCatalog};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use ulid::Ulid;

fn attachment_resource(locator: &str, mime: Option<&str>) -> Resource {
    let mut properties = BTreeMap::new();
    if let Some(m) = mime {
        properties.insert("mime".to_string(), m.to_string());
    }
    Resource {
        r#ref: ResourceRef::new(ResourceKind::Attachment, Ulid::new()),
        kind: ResourceKind::Attachment,
        title: locator.to_string(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: locator.to_string(),
        properties,
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    }
}

fn ctx_for<'a>(
    catalog: &'a PreviewerCatalog,
    resource: Resource,
    mime: Option<&str>,
    bytes: Option<Bytes>,
) -> PreviewContext<'a> {
    let locator = resource.locator.clone();
    PreviewContext {
        resource,
        bytes,
        mime: mime.map(|s| s.to_string()),
        locator: PathBuf::from(locator),
        segments: Vec::new(),
        siblings: Vec::new(),
        catalog,
        service: None,
    }
}

// ---- PPTX ----

/// Build a PPTX in memory: a ZIP archive containing two slides
/// (`ppt/slides/slide1.xml`, `slide2.xml`) with `<a:t>` text runs.
fn build_pptx_fixture() -> Bytes {
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts: zip::write::FileOptions = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("ppt/slides/slide1.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?>
<p:sld>
  <p:cSld><p:sp><p:txBody><a:p><a:r><a:t>First Slide Title</a:t></a:r><a:r><a:t>body line</a:t></a:r></a:p></p:txBody></p:sp></p:cSld>
</p:sld>"#,
        )
        .unwrap();
        zip.start_file("ppt/slides/slide2.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?>
<p:sld>
  <p:cSld><p:sp><p:txBody><a:p><a:r><a:t>Second Slide</a:t></a:r></a:p></p:txBody></p:sp></p:cSld>
</p:sld>"#,
        )
        .unwrap();
        zip.finish().unwrap();
    }
    Bytes::from(buf)
}

#[test]
fn pptx_matches_pptx_locator() {
    let p = PptxPreviewer;
    let r = attachment_resource(
        "deck.pptx",
        Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
    );
    let bytes = build_pptx_fixture();
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(
        &catalog,
        r,
        Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        Some(bytes),
    );
    assert!(p.matches(&c));
}

#[test]
fn pptx_renders_slide_text() {
    let p = PptxPreviewer;
    let bytes = build_pptx_fixture();
    let r = attachment_resource("deck.pptx", None);
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, None, Some(bytes));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Pptx { slides } = m else {
        panic!("expected PreviewModel::Pptx");
    };
    assert_eq!(slides.len(), 2, "expected two slides, got {slides:?}");
    assert_eq!(slides[0].index, 1);
    assert_eq!(slides[0].title.as_deref(), Some("First Slide Title"));
    // Slide 1 has two distinct <a:r> runs; the previewer must keep them as
    // separate entries rather than merging them.
    assert_eq!(
        slides[0].body,
        vec!["First Slide Title".to_string(), "body line".to_string()],
        "body entries should be split per <a:r> run"
    );
    // Slide 2 has a single run; the body should hold exactly that.
    assert_eq!(slides[1].body, vec!["Second Slide".to_string()]);
    assert_eq!(slides[1].title.as_deref(), Some("Second Slide"));
    assert!(slides[1].notes.is_none());
}

#[test]
fn pptx_does_not_match_non_pptx_locator() {
    let p = PptxPreviewer;
    let r = attachment_resource("readme.txt", Some("text/plain"));
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, Some("text/plain"), None);
    assert!(!p.matches(&c));
}
#[test]
fn pptx_renders_cdata_text_run() {
    // A slide whose <a:t> contains a CDATA section must yield the CDATA
    // payload as text; before the fix it was silently dropped.
    let p = PptxPreviewer;
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("ppt/slides/slide1.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?>
<p:sld>
  <p:cSld><p:sp><p:txBody><a:p><a:r><a:t><![CDATA[Hello&<World>]]></a:t></a:r></a:p></p:txBody></p:sp></p:cSld>
</p:sld>"#,
        )
        .unwrap();
        zip.finish().unwrap();
    }
    let bytes = Bytes::from(buf);
    let r = attachment_resource("deck.pptx", None);
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, None, Some(bytes));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Pptx { slides } = m else {
        panic!("expected PreviewModel::Pptx");
    };
    assert_eq!(slides.len(), 1, "expected one slide, got {slides:?}");
    assert_eq!(slides[0].body, vec!["Hello&<World>".to_string()]);
    assert_eq!(slides[0].title.as_deref(), Some("Hello&<World>"));
}

#[test]
fn pptx_returns_parse_error_for_malformed_slide_xml() {
    // A truncated slide XML must surface as PreviewError::Extraction,
    // not be silently swallowed.
    let p = PptxPreviewer;
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("ppt/slides/slide1.xml", opts).unwrap();
        // Unterminated tag with a malformed attribute value — quick-xml
        // surfaces this as a parse error from `read_event`.
        zip.write_all(b"<a:t attr=\"<broken").unwrap();
        zip.finish().unwrap();
    }
    let bytes = Bytes::from(buf);
    let r = attachment_resource("deck.pptx", None);
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, None, Some(bytes));
    match p.render(&c) {
        Err(preview::PreviewError::Extraction(msg)) => {
            assert!(
                msg.contains("pptx xml parse error"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("expected Extraction error, got {other:?}"),
    }
}

// ---- ZIP ----

fn build_zip_fixture() -> Bytes {
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts: zip::write::FileOptions = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("alpha.txt", opts).unwrap();
        zip.write_all(b"alpha").unwrap();
        zip.start_file("nested/beta.txt", opts).unwrap();
        zip.write_all(b"beta-bytes").unwrap();
        zip.add_directory("empty-dir", opts).unwrap();
        zip.finish().unwrap();
    }
    Bytes::from(buf)
}

#[test]
fn zip_matches_zip_locator() {
    let p = ZipPreviewer;
    let r = attachment_resource("bundle.zip", Some("application/zip"));
    let bytes = build_zip_fixture();
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, Some("application/zip"), Some(bytes));
    assert!(p.matches(&c));
}

#[test]
fn zip_lists_entries() {
    let p = ZipPreviewer;
    let bytes = build_zip_fixture();
    let r = attachment_resource("bundle.zip", None);
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, None, Some(bytes));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Zip { entries } = m else {
        panic!("expected PreviewModel::Zip");
    };
    let names: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    assert!(names.contains(&"alpha.txt"), "missing alpha.txt: {names:?}");
    assert!(names.contains(&"nested/beta.txt"), "missing nested: {names:?}");
    assert!(names.contains(&"empty-dir/"), "missing empty-dir: {names:?}");
    let alpha = entries.iter().find(|e| e.path == "alpha.txt").unwrap();
    assert_eq!(alpha.size, 5);
    assert!(!alpha.is_dir);
    let empty = entries.iter().find(|e| e.path == "empty-dir/").unwrap();
    assert!(empty.is_dir);
}

#[test]
fn zip_missing_bytes_errors() {
    let p = ZipPreviewer;
    let r = attachment_resource("bundle.zip", None);
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, None, None);
    let err = p.render(&c).expect_err("expected missing bytes");
    assert!(
        matches!(err, preview::PreviewError::MissingBytes(_)),
        "got {err:?}"
    );
}

// ---- Image ----

/// Build a 1×1 PNG: 8-byte signature, then an IHDR chunk with width=1, height=1.
/// (CRC is left as a placeholder; the previewer only reads the IHDR header.)
fn build_png_1x1() -> Bytes {
    let mut b = Vec::new();
    b.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&13u32.to_be_bytes());
    ihdr.extend_from_slice(b"IHDR");
    ihdr.extend_from_slice(&1u32.to_be_bytes()); // width
    ihdr.extend_from_slice(&1u32.to_be_bytes()); // height
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit depth, RGB, no filter, no interlace
    ihdr.extend_from_slice(&[0u8; 4]); // trailing CRC placeholder
    ihdr.extend_from_slice(&[0u8; 4]);
    b.extend_from_slice(&ihdr);
    Bytes::from(b)
}

#[test]
fn image_matches_image_mime() {
    let p = ImagePreviewer;
    let r = attachment_resource("pixel.png", Some("image/png"));
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, Some("image/png"), Some(build_png_1x1()));
    assert!(p.matches(&c));
}

#[test]
fn image_does_not_match_text_mime() {
    let p = ImagePreviewer;
    let r = attachment_resource("readme.txt", Some("text/plain"));
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, Some("text/plain"), None);
    assert!(!p.matches(&c));
}

#[test]
fn image_extracts_png_dimensions() {
    let p = ImagePreviewer;
    let bytes = build_png_1x1();
    let r = attachment_resource("pixel.png", None);
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, Some("image/png"), Some(bytes));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Image {
        src,
        width,
        height,
        mime,
    } = m
    else {
        panic!("expected PreviewModel::Image");
    };
    assert_eq!(width, 1);
    assert_eq!(height, 1);
    assert_eq!(mime, "image/png");
    assert!(
        src.starts_with("/s/src-1/a/"),
        "src should be a URL path: {src}"
    );
}

#[test]
fn image_unknown_dimensions_when_bytes_absent() {
    let p = ImagePreviewer;
    let r = attachment_resource("pixel.png", None);
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, r, Some("image/png"), None);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Image { width, height, .. } = m else {
        panic!("expected PreviewModel::Image");
    };
    assert_eq!(width, 0);
    assert_eq!(height, 0);
}
