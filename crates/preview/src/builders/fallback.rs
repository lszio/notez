use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};

/// Fallback previewer that matches every context.
///
/// MUST be registered LAST in the catalog so that every other previewer
/// gets a chance to claim the context first. `render` emits a short
/// human-readable `PreviewModel::Fallback` message describing the resource
/// (ref-id + payload size) so the UI can render a "download for raw view"
/// prompt.
pub struct FallbackPreviewer;

impl Previewer for FallbackPreviewer {
    fn id(&self) -> &'static str {
        "fallback"
    }

    fn matches(&self, _ctx: &PreviewContext) -> bool {
        true
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes = ctx.bytes.as_deref().map(|b| b.len()).unwrap_or(0);
        Ok(PreviewModel::Fallback {
            message: format!(
                "Resource {:?} — {bytes} bytes (download for raw view)",
                ctx.resource.r#ref
            ),
        })
    }
}
