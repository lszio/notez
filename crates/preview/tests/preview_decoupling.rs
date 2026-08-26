//! Preview decoupling contracts.
//!
//! Pin the rule that `PreviewContext` no longer carries a concrete
//! `ApplicationService<SqliteProjection>`; preview must render strictly
//! from data passed in by the caller.

use notez_core::domain::{Resource, ResourceKind, ResourceRef, ProjectionReader, ProjectionWrite};
use notez_preview::{PreviewContext, PreviewModel, PreviewerCatalog, default_catalog};

fn make_ctx<'a>(
    bytes: &'static [u8],
    mime: &'static str,
    locator: &str,
    catalog: &'a PreviewerCatalog,
) -> PreviewContext<'a> {
    PreviewContext {
        resource: Resource {
            r#ref: ResourceRef::parse("document:01J000000000000000000000D1").unwrap(),
            kind: ResourceKind::Document,
            title: "T".to_string(),
            revision: "r1".to_string(),
            source_id: "native".to_string(),
            locator: locator.to_string(),
            properties: Default::default(),
            object_id: notez_core::domain::ObjectIdentity::default(),
            primary_source_id: String::new(),
        },
        bytes: Some(bytes::Bytes::from_static(bytes)),
        mime: Some(mime.to_string()),
        locator: std::path::PathBuf::from(locator),
        segments: vec![],
        siblings: vec![],
        catalog,
    }
}

#[test]
fn preview_context_type_does_not_carry_application_service_handle() {
    let catalog = default_catalog();
    let ctx = make_ctx(b"# Hello\n", "text/markdown", "/x.md", &catalog);
    let previewer = ctx.catalog.resolve(Some("fallback"), &ctx);
    assert!(previewer.is_some());
}

#[test]
fn render_with_default_catalog_needs_no_application_service() {
    let catalog = default_catalog();
    let ctx = make_ctx(b"* heading\n", "text/org", "/y.org", &catalog);
    let previewer = ctx
        .catalog
        .resolve(None, &ctx)
        .expect("at least one previewer must match a default context");
    let _model = previewer.render(&ctx).expect("render must succeed");
}
