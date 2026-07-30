use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};

/// Previewer for D2 diagrams embedded in source blocks.
///
/// Matches when the resource body contains the substring `d2` (the canonical
/// Org source-block marker is `#+BEGIN_SRC d2`, but a plain substring check
/// is sufficient for finger-of-intent routing). The full body is emitted as
/// the `source` of the `PreviewModel::D2` payload so the client-side D2 ESM
/// shim can render it.
pub struct D2Previewer;

impl Previewer for D2Previewer {
    fn id(&self) -> &'static str {
        "d2"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource
            .properties
            .get("body")
            .map(|b| b.contains("d2"))
            .unwrap_or(false)
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let source = ctx
            .resource
            .properties
            .get("body")
            .cloned()
            .unwrap_or_default();
        Ok(PreviewModel::D2 { source })
    }
}
