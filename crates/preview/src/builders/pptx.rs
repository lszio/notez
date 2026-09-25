//! Previewer for PowerPoint (PPTX) attachments.
//!
//! Matches on a `.pptx` locator suffix or the
//! `application/vnd.openxmlformats-officedocument.presentationml.presentation`
//! MIME.
//!
//! The PPTX preview is a *client-side viewer*: the previewer emits
//! `PreviewModel::Viewer { format: ViewerFormat::Pptx }` and the web UI
//! mounts the `pptx` viewer plugin (`<div data-viewer="pptx">` bound to
//! the raw-bytes URL), which renders the actual slides in the browser
//! with `pptx-preview`.
//!
//! Text extraction is deliberately gone from preview: the extracted
//! bullet list was not a faithful presentation preview, and attachment
//! segmentation/search stays in `notez-core`'s artifact extractors.
//!
//! The previewer declares `can_edit` so the attachment page offers the
//! generic replace-file edit affordance.

use crate::{PreviewCapabilities, PreviewContext, PreviewError, PreviewModel, Previewer, ViewerFormat};

/// Previewer for PowerPoint (PPTX) attachments.
pub struct PptxPreviewer;

impl Previewer for PptxPreviewer {
    fn id(&self) -> &'static str {
        "pptx"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".pptx")
            || ctx.mime.as_deref()
                == Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
    }

    fn capabilities(&self) -> PreviewCapabilities {
        PreviewCapabilities { can_edit: true }
    }

    fn render(&self, _ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        Ok(PreviewModel::Viewer {
            format: ViewerFormat::Pptx,
        })
    }
}
