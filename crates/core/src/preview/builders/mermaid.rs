use crate::preview::{PreviewContext, PreviewError, PreviewModel, Previewer};

/// Previewer for Mermaid diagrams embedded in Org (or Markdown) sources.
///
/// Matches when the resource body contains the substring `mermaid` (the
/// canonical Org source-block marker is `#+BEGIN_SRC mermaid`, but a plain
/// substring check is sufficient to finger the intent and to disambiguate
/// from the more permissive Org previewer that runs first in the catalog).
/// The full body is emitted verbatim as the `source` of the
/// `PreviewModel::Mermaid` payload so the client-side hydration can render
/// it through the Mermaid ESM shim.
pub struct MermaidPreviewer;

impl Previewer for MermaidPreviewer {
    fn id(&self) -> &'static str {
        "mermaid"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource
            .properties
            .get("body")
            .map(|b| b.contains("mermaid"))
            .unwrap_or(false)
            || ctx.mime.as_deref() == Some("text/vnd.mermaid")
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let source = ctx
            .resource
            .properties
            .get("body")
            .cloned()
            .unwrap_or_default();
        Ok(PreviewModel::Mermaid { source })
    }
}
