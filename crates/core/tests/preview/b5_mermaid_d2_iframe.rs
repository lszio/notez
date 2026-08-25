//! Tests for the phase-B part-2 previewers: Mermaid / D2 / Iframe.
//!
//! Each test builds a resource whose body triggers the previewer's matcher
//! and verifies the rendered `PreviewModel` payload.

use crate::domain::{Resource, ResourceKind, ResourceRef};
use crate::preview::builders::d2::D2Previewer;
use crate::preview::builders::iframe::IframePreviewer;
use crate::preview::builders::mermaid::MermaidPreviewer;
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
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    }
}

fn ctx_for<'a>(
    catalog: &'a PreviewerCatalog,
    resource: Resource,
    mime: Option<&str>,
) -> PreviewContext<'a> {
    let locator = resource.locator.clone();
    PreviewContext {
        resource,
        bytes: None,
        mime: mime.map(|s| s.to_string()),
        locator: PathBuf::from(locator),
        segments: Vec::new(),
        siblings: Vec::new(),
        catalog,
        service: None,
    }
}

// ---- Mermaid ----

#[test]
fn mermaid_matches_mermaid_block() {
    let p = MermaidPreviewer;
    let body = "#+BEGIN_SRC mermaid\ngraph LR; A-->B;\n#+END_SRC\n";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), None);
    assert!(p.matches(&c));
}

#[test]
fn mermaid_does_not_match_plain_org() {
    let p = MermaidPreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("* Just a heading\n"), None);
    assert!(!p.matches(&c));
}

#[test]
fn mermaid_renders_source() {
    let p = MermaidPreviewer;
    let body = "#+BEGIN_SRC mermaid\nflowchart LR; A-->B;\n#+END_SRC\n";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), None);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Mermaid { source } = m else {
        panic!("expected PreviewModel::Mermaid");
    };
    assert!(source.contains("flowchart LR"));
    assert!(source.contains("A-->B"));
}

// ---- D2 ----

#[test]
fn d2_matches_d2_block() {
    let p = D2Previewer;
    let body = "#+BEGIN_SRC d2\nserver -> database\n#+END_SRC\n";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), None);
    assert!(p.matches(&c));
}

#[test]
fn d2_does_not_match_unmentioning_body() {
    let p = D2Previewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("plain text without any marker"), None);
    assert!(!p.matches(&c));
}

#[test]
fn d2_renders_source() {
    let p = D2Previewer;
    let body = "#+BEGIN_SRC d2\nserver -> database\n#+END_SRC\n";
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body(body), None);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::D2 { source } = m else {
        panic!("expected PreviewModel::D2");
    };
    assert!(source.contains("server -> database"));
}

// ---- Iframe ----

#[test]
fn iframe_matches_prefix() {
    let p = IframePreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("[[iframe:https://example.com]]"), None);
    assert!(p.matches(&c));
}

#[test]
fn iframe_does_not_match_plain_link() {
    let p = IframePreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(
        &catalog,
        doc_with_body("[[https://example.com]] not an iframe"),
        None,
    );
    assert!(!p.matches(&c));
}

#[test]
fn iframe_extracts_url() {
    let p = IframePreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("[[iframe:https://example.com/embed]]"), None);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Iframe { src, sandbox } = m else {
        panic!("expected PreviewModel::Iframe");
    };
    assert_eq!(src, "https://example.com/embed");
    assert_eq!(sandbox, "allow-scripts");
}

#[test]
fn iframe_extracts_custom_sandbox() {
    let p = IframePreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(
        &catalog,
        doc_with_body("[[iframe:https://example.com sandbox=allow-scripts allow-same-origin]]"),
        None,
    );
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Iframe { src, sandbox } = m else {
        panic!("expected PreviewModel::Iframe");
    };
    assert_eq!(src, "https://example.com");
    assert_eq!(sandbox, "allow-scripts allow-same-origin");
}

#[test]
fn iframe_tolerates_leading_whitespace() {
    let p = IframePreviewer;
    let catalog = PreviewerCatalog::new();
    let c = ctx_for(&catalog, doc_with_body("  [[iframe:https://example.com]]  "), None);
    assert!(p.matches(&c));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Iframe { src, .. } = m else {
        panic!("expected PreviewModel::Iframe");
    };
    assert_eq!(src, "https://example.com");
}
