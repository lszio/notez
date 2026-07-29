//! Shared helpers for the phase-B previewer integration tests.
//!
//! The `phase_b_catalog()` constructor builds a `PreviewerCatalog` containing
//! the four previewers introduced in Phase B (Org, Markdown, Pdf, Xlsx) plus
//! a `Fallback` previewer that always matches. Future previewers (mermaid,
//! d2, iframe, link_embed, query_embed, block_embed, pptx, zip, image) will be
//! added by the dispatch that lands them.

#![allow(dead_code)]

pub mod org;
pub mod markdown;
pub mod pdf;
pub mod xlsx;

use bytes::Bytes;
use domain::{Resource, ResourceKind, ResourceRef, SegmentRecord};
use preview::builders::markdown::MarkdownPreviewer;
use preview::builders::org::OrgPreviewer;
use preview::builders::pdf::PdfPreviewer;
use preview::builders::xlsx::XlsxPreviewer;
use preview::{Heading, PreviewContext, PreviewModel, Previewer, PreviewerCatalog};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use ulid::Ulid;

/// Fallback previewer used only by tests. Always matches, returns a small
/// placeholder `PreviewModel::Fallback`.
pub struct FallbackPreviewer;

impl Previewer for FallbackPreviewer {
    fn id(&self) -> &'static str {
        "fallback"
    }
    fn matches(&self, _ctx: &PreviewContext) -> bool {
        true
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, preview::PreviewError> {
        Ok(PreviewModel::Fallback {
            message: format!("no previewer for {:?}", ctx.resource.kind),
        })
    }
}

/// Build a catalog with Markdown, Org, Pdf, Xlsx + Fallback registered.
///
/// Markdown is registered before Org so that `.md` documents resolve to
/// the Markdown previewer (which matches on the locator suffix or the
/// declared MIME) rather than to the more permissive Org previewer (which
/// matches on `ResourceKind::Document | Heading | Block`).
pub fn phase_b_catalog() -> PreviewerCatalog {
    let mut c = PreviewerCatalog::new();
    c.register(MarkdownPreviewer);
    c.register(OrgPreviewer);
    PdfPreviewer::register(&mut c);
    XlsxPreviewer::register(&mut c);
    c.register(FallbackPreviewer);
    c
}

/// Build a `PreviewContext` for an Org document resource whose body is `body`.
pub fn org_ctx<'a>(catalog: &'a PreviewerCatalog, body: &str) -> PreviewContext<'a> {
    let locator = "note.org".to_string();
    let mut properties = BTreeMap::new();
    properties.insert("body".to_string(), body.to_string());
    let resource = Resource {
        r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: locator.clone(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: locator.clone(),
        properties,
    };
    PreviewContext {
        resource,
        bytes: None,
        mime: None,
        locator: PathBuf::from(locator),
        segments: Vec::<SegmentRecord>::new(),
        siblings: Vec::<Resource>::new(),
        catalog,
        service: None,
    }
}

/// Build a `PreviewContext` for a Markdown resource whose body is `body`.
pub fn markdown_ctx<'a>(catalog: &'a PreviewerCatalog, body: &str) -> PreviewContext<'a> {
    let locator = "note.md".to_string();
    let mut properties = BTreeMap::new();
    properties.insert("body".to_string(), body.to_string());
    let resource = Resource {
        r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: locator.clone(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: locator.clone(),
        properties,
    };
    PreviewContext {
        resource,
        bytes: None,
        mime: Some("text/markdown".to_string()),
        locator: PathBuf::from(locator),
        segments: Vec::<SegmentRecord>::new(),
        siblings: Vec::<Resource>::new(),
        catalog,
        service: None,
    }
}

/// Build a `PreviewContext` for a PDF attachment, including bytes.
pub fn pdf_ctx<'a>(catalog: &'a PreviewerCatalog, bytes: Bytes) -> PreviewContext<'a> {
    let locator = Path::new("report.pdf");
    let resource = Resource {
        r#ref: ResourceRef::new(ResourceKind::Attachment, Ulid::new()),
        kind: ResourceKind::Attachment,
        title: locator.to_string_lossy().to_string(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: locator.to_string_lossy().to_string(),
        properties: BTreeMap::new(),
    };
    PreviewContext {
        resource,
        bytes: Some(bytes),
        mime: Some("application/pdf".to_string()),
        locator: locator.to_path_buf(),
        segments: Vec::<SegmentRecord>::new(),
        siblings: Vec::<Resource>::new(),
        catalog,
        service: None,
    }
}

/// Build a `PreviewContext` for an XLSX attachment, including bytes.
pub fn xlsx_ctx<'a>(catalog: &'a PreviewerCatalog, bytes: Bytes) -> PreviewContext<'a> {
    let locator = Path::new("data.xlsx");
    let resource = Resource {
        r#ref: ResourceRef::new(ResourceKind::Attachment, Ulid::new()),
        kind: ResourceKind::Attachment,
        title: locator.to_string_lossy().to_string(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: locator.to_string_lossy().to_string(),
        properties: BTreeMap::new(),
    };
    PreviewContext {
        resource,
        bytes: Some(bytes),
        mime: Some(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
        ),
        locator: locator.to_path_buf(),
        segments: Vec::<SegmentRecord>::new(),
        siblings: Vec::<Resource>::new(),
        catalog,
        service: None,
    }
}

/// Convenience: take the first heading from a `PreviewModel::Org` payload.
pub fn first_heading(m: &PreviewModel) -> &Heading {
    match m {
        PreviewModel::Org { outline, .. } => outline.first().expect("outline not empty"),
        other => panic!("expected Org, got {other:?}"),
    }
}
