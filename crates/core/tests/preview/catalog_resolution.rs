//! Centralized catalog-resolution tests for the full phase-B previewer set.
//!
//! Every test exercises the canonical `preview::default_catalog()` (14
//! previewers, with `FallbackPreviewer` registered LAST). The tests cover
//! the catalog match-order, the override-path, and the default behavior for
//! each previewer family.

use bytes::Bytes;
use crate::domain::{Resource, ResourceKind, ResourceRef};
use crate::preview::{default_catalog, PreviewContext, PreviewerCatalog};
use std::collections::BTreeMap;
use std::path::PathBuf;
use ulid::Ulid;

fn doc_with_body(body: &str) -> Resource {
    let mut properties = BTreeMap::new();
    properties.insert("body".to_string(), body.to_string());
    Resource {
        r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: "doc".into(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: "doc.org".into(),
        properties,
        object_id: notez_core::domain::ObjectId::default(),
    }
}

fn markdown_resource(body: &str) -> Resource {
    let mut properties = BTreeMap::new();
    properties.insert("body".to_string(), body.to_string());
    Resource {
        r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: "note.md".into(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: "note.md".into(),
        properties,
        object_id: notez_core::domain::ObjectId::default(),
    }
}

fn attachment(locator: &str, mime: Option<&str>) -> Resource {
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
        object_id: notez_core::domain::ObjectId::default(),
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
        segments: vec![],
        siblings: vec![],
        catalog,
        service: None,
    }
}

// ---- catalog shape ----

#[test]
fn default_catalog_registers_all_fourteen_previewers() {
    let cat = default_catalog();
    let ids: Vec<&'static str> = cat.iter().map(|p| p.id()).collect();
    assert_eq!(ids.len(), 14, "expected 14 previewers, got {ids:?}");
    for expected in [
        "markdown",
        "org",
        "pdf",
        "xlsx",
        "pptx",
        "zip",
        "image",
        "mermaid",
        "d2",
        "iframe",
        "block_embed",
        "query_embed",
        "link_embed",
        "fallback",
    ] {
        assert!(
            ids.contains(&expected),
            "missing {expected} in catalog: {ids:?}"
        );
    }
}

#[test]
fn default_catalog_registers_fallback_last() {
    let cat = default_catalog();
    let last = cat.iter().last().expect("at least one previewer");
    assert_eq!(last.id(), "fallback", "fallback must be last");
}

// ---- match-order ----

#[test]
fn org_matches_document_with_body() {
    let cat = default_catalog();
    let r = doc_with_body("* Hi");
    let c = ctx_for(&cat, r, None, None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("org"));
}

#[test]
fn markdown_matches_md_locator() {
    let cat = default_catalog();
    let r = markdown_resource("# hi");
    let c = ctx_for(&cat, r, Some("text/markdown"), None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("markdown"));
}

#[test]
fn pdf_matches_pdf_attachment() {
    let cat = default_catalog();
    let r = attachment("report.pdf", Some("application/pdf"));
    let c = ctx_for(&cat, r, Some("application/pdf"), None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("pdf"));
}

#[test]
fn xlsx_matches_xlsx_attachment() {
    let cat = default_catalog();
    let r = attachment(
        "data.xlsx",
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
    );
    let c = ctx_for(
        &cat,
        r,
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        None,
    );
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("xlsx"));
}

#[test]
fn pptx_matches_pptx_attachment() {
    let cat = default_catalog();
    let pptx_mime = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
    let r = attachment("deck.pptx", Some(pptx_mime));
    let c = ctx_for(&cat, r, Some(pptx_mime), None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("pptx"));
}

#[test]
fn zip_matches_zip_attachment() {
    let cat = default_catalog();
    let r = attachment("bundle.zip", Some("application/zip"));
    let c = ctx_for(&cat, r, Some("application/zip"), None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("zip"));
}

#[test]
fn image_matches_image_mime() {
    let cat = default_catalog();
    let r = attachment("pixel.png", Some("image/png"));
    let c = ctx_for(&cat, r, Some("image/png"), None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("image"));
}

#[test]
fn mermaid_matches_mermaid_block() {
    let cat = default_catalog();
    let body = "#+BEGIN_SRC mermaid\ngraph LR; A-->B;\n#+END_SRC\n";
    let r = doc_with_body(body);
    let c = ctx_for(&cat, r, None, None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("mermaid"));
}

#[test]
fn d2_matches_d2_block() {
    let cat = default_catalog();
    let body = "#+BEGIN_SRC d2\nserver -> database\n#+END_SRC\n";
    let r = doc_with_body(body);
    let c = ctx_for(&cat, r, None, None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("d2"));
}

#[test]
fn iframe_matches_prefix() {
    let cat = default_catalog();
    let r = doc_with_body("[[iframe:https://example.com]]");
    let c = ctx_for(&cat, r, None, None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("iframe"));
}

#[test]
fn block_embed_matches_prefix() {
    let cat = default_catalog();
    let r = doc_with_body("[[block:01ABC]]");
    let c = ctx_for(&cat, r, None, None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("block_embed"));
}

#[test]
fn query_embed_matches_query_block() {
    let cat = default_catalog();
    let body = "#+BEGIN_SRC query\nSOURCE: notes\nLIMIT: 5\n#+END_SRC\n";
    let r = doc_with_body(body);
    let c = ctx_for(&cat, r, None, None);
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("query_embed"));
}

// ---- override path ----

#[test]
fn override_id_forces_choice() {
    let cat = default_catalog();
    let r = doc_with_body("* Hi\n");
    let c = ctx_for(&cat, r, None, None);
    // Org would normally resolve, but override pins fallback.
    assert_eq!(
        cat.resolve(Some("fallback"), &c).map(|p| p.id()),
        Some("fallback")
    );
    // Override can also pin a non-matching previewer (e.g. markdown on an org).
    let r2 = doc_with_body("* Hi\n");
    let c2 = ctx_for(&cat, r2, None, None);
    assert_eq!(
        cat.resolve(Some("markdown"), &c2).map(|p| p.id()),
        Some("markdown")
    );
}

#[test]
fn link_embed_only_via_override() {
    let cat = default_catalog();
    let r = doc_with_body("anything");
    let c = ctx_for(&cat, r, None, None);
    // Default match path picks a different previewer (org, since the body
    // is plain Org text).
    assert_eq!(cat.resolve(None, &c).map(|p| p.id()), Some("org"));
    // Override can pin link_embed specifically.
    let r2 = doc_with_body("anything");
    let c2 = ctx_for(&cat, r2, None, None);
    assert_eq!(
        cat.resolve(Some("link_embed"), &c2).map(|p| p.id()),
        Some("link_embed")
    );
}

// ---- fallback catches everything ----

#[test]
fn fallback_catches_unmatched() {
    let cat = default_catalog();
    // Pick an Attachment with an unsupported MIME so only fallback wins.
    let r = attachment("weird", Some("application/x-unknown"));
    let c = ctx_for(&cat, r, Some("application/x-unknown"), None);
    assert_eq!(
        cat.resolve(None, &c).map(|p| p.id()),
        Some("fallback"),
        "fallback should catch unmatched attachments"
    );
}
