//! Centralized catalog-resolution tests for the four phase-B previewers.
//!
//! Each test builds a fresh `phase_b_catalog()` (Org + Markdown + Pdf + Xlsx
//! + Fallback) and then either asks the catalog to resolve a context, or
//! asks a specific previewer to render the context directly.

use bytes::Bytes;
use domain::{Resource, ResourceKind, ResourceRef};
use preview::{PreviewContext, PreviewerCatalog};
use std::collections::BTreeMap;
use std::path::PathBuf;
use ulid::Ulid;

mod previewers;
use previewers::phase_b_catalog;

// --- fixture builders kept local so this file compiles standalone ---

fn org_resource(body: &str) -> Resource {
    let mut properties = BTreeMap::new();
    properties.insert("body".into(), body.into());
    Resource {
        r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: "note.org".into(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: "note.org".into(),
        properties,
    }
}

fn markdown_resource(body: &str) -> Resource {
    let mut properties = BTreeMap::new();
    properties.insert("body".into(), body.into());
    Resource {
        r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: "note.md".into(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: "note.md".into(),
        properties,
    }
}

fn pdf_attachment(bytes: Bytes) -> Resource {
    Resource {
        r#ref: ResourceRef::new(ResourceKind::Attachment, Ulid::new()),
        kind: ResourceKind::Attachment,
        title: "report.pdf".into(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: "report.pdf".into(),
        properties: BTreeMap::new(),
    }
    .with_size(bytes.len())
}

trait ResourceExt {
    fn with_size(self, size: usize) -> Self;
}

impl ResourceExt for Resource {
    fn with_size(mut self, size: usize) -> Self {
        self.properties.insert("size".into(), size.to_string());
        self
    }
}

fn ctx_for<'a>(
    catalog: &'a PreviewerCatalog,
    resource: Resource,
    mime: Option<&str>,
    bytes: Option<Bytes>,
) -> PreviewContext<'a> {
    let locator = PathBuf::from(resource.locator.clone());
    PreviewContext {
        resource,
        bytes,
        mime: mime.map(|s| s.to_string()),
        locator,
        segments: vec![],
        siblings: vec![],
        catalog,
        service: None,
    }
}

#[test]
fn org_matches_document_with_body() {
    let cat = phase_b_catalog();
    let r = org_resource("* Hi\n");
    let c = ctx_for(&cat, r, None, None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("org"));
}

#[test]
fn markdown_matches_md_locator() {
    let cat = phase_b_catalog();
    let r = markdown_resource("# hi\n");
    let c = ctx_for(&cat, r, Some("text/markdown"), None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("markdown"));
}

#[test]
fn pdf_matches_pdf_attachment() {
    let cat = phase_b_catalog();
    let r = pdf_attachment(Bytes::from_static(b"%PDF-1.4\n%fake\n"));
    let c = ctx_for(&cat, r, Some("application/pdf"), None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("pdf"));
}

#[test]
fn override_id_forces_choice() {
    let cat = phase_b_catalog();
    let r = org_resource("* Hi\n");
    let c = ctx_for(&cat, r, None, None);
    // Org would normally resolve, but override pins fallback.
    assert_eq!(
        cat.resolve(Some("fallback"), &c).map(|p| p.id()),
        Some("fallback")
    );
    // Override can also pin a non-matching previewer (e.g. markdown on an org).
    let r2 = org_resource("* Hi\n");
    let c2 = ctx_for(&cat, r2, None, None);
    assert_eq!(
        cat.resolve(Some("markdown"), &c2).map(|p| p.id()),
        Some("markdown")
    );
}
