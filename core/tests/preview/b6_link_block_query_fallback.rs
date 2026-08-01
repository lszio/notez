//! Tests for the phase-B part-2 previewers: LinkEmbed / BlockEmbed /
//! QueryEmbed / Fallback.

use bytes::Bytes;
use crate::domain::{Resource, ResourceKind, ResourceRef};
use crate::preview::builders::block_embed::BlockEmbedPreviewer;
use crate::preview::builders::fallback::FallbackPreviewer;
use crate::preview::builders::link_embed::LinkEmbedPreviewer;
use crate::preview::builders::query_embed::QueryEmbedPreviewer;
use crate::preview::{PreviewContext, PreviewModel, Previewer, PreviewerCatalog};
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
    }
}

fn doc_with_props(props: BTreeMap<String, String>) -> Resource {
    Resource {
        r#ref: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: "doc".into(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: "doc.org".into(),
        properties: props,
    }
}

fn ctx_for<'a>(
    catalog: &'a PreviewerCatalog,
    resource: Resource,
    siblings: Vec<Resource>,
) -> PreviewContext<'a> {
    let locator = resource.locator.clone();
    PreviewContext {
        resource,
        bytes: None,
        mime: None,
        locator: PathBuf::from(locator),
        segments: Vec::new(),
        siblings,
        catalog,
        service: None,
    }
}

fn build_default_catalog() -> PreviewerCatalog {
    let mut c = PreviewerCatalog::new();
    c.register(FallbackPreviewer);
    c
}

// ---- LinkEmbed ----

#[test]
fn link_embed_does_not_match_by_default() {
    let p = LinkEmbedPreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("anything"), vec![]);
    // Override-only: a direct match() must never pick this previewer.
    assert!(!p.matches(&c));
}

#[test]
fn link_embed_via_override_resolves_sibling() {
    let p = LinkEmbedPreviewer;
    // Build a target resource ref to embed.
    let target_ref = ResourceRef::new(ResourceKind::Document, Ulid::new());
    let target = Resource {
        r#ref: target_ref,
        kind: ResourceKind::Document,
        title: "linked".into(),
        revision: "rev-1".into(),
        source_id: "src-1".into(),
        locator: "linked.org".into(),
        properties: BTreeMap::new(),
    };

    // Source resource carries the link_target property.
    let mut props = BTreeMap::new();
    props.insert("link_target".into(), serde_json::to_string(&target_ref).unwrap());
    let source = doc_with_props(props);

    let catalog = build_default_catalog();
    let c = ctx_for(&catalog, source, vec![target.clone()]);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::LinkEmbed { target: got, child } = m else {
        panic!("expected PreviewModel::LinkEmbed");
    };
    assert_eq!(got.r#ref, target_ref);
    assert!(matches!(*child, PreviewModel::Fallback { .. }));
}

#[test]
fn link_embed_errors_without_link_target() {
    let p = LinkEmbedPreviewer;
    let catalog = build_default_catalog();
    let c = ctx_for(&catalog, doc_with_body(""), vec![]);
    let err = p.render(&c).expect_err("expected extraction error");
    assert!(
        matches!(err, preview::PreviewError::Extraction(_)),
        "got {err:?}"
    );
}

// ---- BlockEmbed ----

#[test]
fn block_embed_matches_prefix() {
    let p = BlockEmbedPreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("[[block:01ABC]]"), vec![]);
    assert!(p.matches(&c));
}

#[test]
fn block_embed_does_not_match_plain_text() {
    let p = BlockEmbedPreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("plain text without block marker"), vec![]);
    assert!(!p.matches(&c));
}

#[test]
fn block_embed_renders_source() {
    let p = BlockEmbedPreviewer;
    let body = "[[block:01ABC]]";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), vec![]);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::BlockEmbed { source, html } = m else {
        panic!("expected PreviewModel::BlockEmbed");
    };
    assert_eq!(source.kind(), ResourceKind::Block);
    assert!(html.contains("[[block:01ABC]]"));
}

// ---- QueryEmbed ----

#[test]
fn query_embed_matches_query_block() {
    let p = QueryEmbedPreviewer;
    let body = "#+BEGIN_SRC query\nSOURCE: notes\nLIMIT: 5\n#+END_SRC\n";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), vec![]);
    assert!(p.matches(&c));
}

#[test]
fn query_embed_does_not_match_non_query_block() {
    let p = QueryEmbedPreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("#+BEGIN_SRC sh\nls\n#+END_SRC\n"), vec![]);
    assert!(!p.matches(&c));
}

#[test]
fn query_embed_parses_request() {
    let p = QueryEmbedPreviewer;
    let body =
        "#+BEGIN_SRC query\nSOURCE: notes\nKIND: heading\nTITLE: weekly\nLIMIT: 3\n#+END_SRC\n";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), vec![]);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::QueryEmbed { query, snapshot } = m else {
        panic!("expected PreviewModel::QueryEmbed");
    };
    assert_eq!(query.source, "notes");
    assert_eq!(query.kind_hint.as_deref(), Some("heading"));
    assert_eq!(query.title_contains.as_deref(), Some("weekly"));
    assert_eq!(query.limit, 3);
    assert!(
        snapshot.is_empty(),
        "snapshot is empty until dynamic queries land"
    );
}

#[test]
fn query_embed_uses_default_limit_when_unset() {
    let p = QueryEmbedPreviewer;
    let body = "#+BEGIN_SRC query\nSOURCE: notes\n#+END_SRC\n";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), vec![]);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::QueryEmbed { query, .. } = m else {
        panic!("expected PreviewModel::QueryEmbed");
    };
    assert_eq!(query.limit, 50, "default limit must be 50");
}

// ---- Fallback ----

#[test]
fn fallback_matches_anything() {
    let p = FallbackPreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("anything"), vec![]);
    assert!(p.matches(&c));
}

#[test]
fn fallback_catches_unmatched() {
    let p = FallbackPreviewer;
    let catalog = build_default_catalog();
    let c = ctx_for(&catalog, doc_with_body("xyz"), vec![]);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Fallback { message } = m else {
        panic!("expected PreviewModel::Fallback");
    };
    assert!(message.contains("Resource"), "message: {message}");
    assert!(message.contains("0 bytes"), "message: {message}");
}

#[test]
fn fallback_messages_byte_count() {
    let p = FallbackPreviewer;
    let catalog = PreviewerCatalog::new();
    let mut c = ctx_for(&catalog, doc_with_body(""), vec![]);
    c.bytes = Some(Bytes::from_static(b"1234567890"));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Fallback { message } = m else {
        panic!("expected PreviewModel::Fallback");
    };
    assert!(message.contains("10 bytes"), "message: {message}");
}
