use super::org_html::render_org_html;
use notez_core::domain::ResourceKind;
use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};

/// Previewer for Org-mode documents, headings, and blocks.
///
/// The Org source text is read from `ctx.resource.properties["body"]` and
/// passed through `render_org_html` to produce HTML and a heading outline.
pub struct OrgPreviewer;

impl Previewer for OrgPreviewer {
    fn id(&self) -> &'static str {
        "org"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        matches!(
            ctx.resource.kind,
            ResourceKind::Document | ResourceKind::Heading | ResourceKind::Block
        )
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let body = ctx
            .resource
            .properties
            .get("body")
            .cloned()
            .unwrap_or_default();
        let (html, outline) = render_org_html(&body);
        Ok(PreviewModel::Org { html, outline })
    }
}
