//! Previewer for Microsoft Word (DOCX) attachments.
//!
//! Matches on a `.docx` locator suffix or the
//! `application/vnd.openxmlformats-officedocument.wordprocessingml.document`
//! MIME.
//!
//! The modern DOCX preview is a *client-side viewer*: the previewer
//! emits `PreviewModel::Viewer { format: ViewerFormat::Docx }` and the
//! web UI mounts the `docx` viewer plugin (`<div data-viewer="docx">`
//! bound to the raw-bytes URL), which renders the actual document
//! (styles, tables, images) in the browser with `docx-preview`.
//!
//! This adapter intentionally does not extract paragraph text any
//! more: the extracted-text presentation lost the real layout, and the
//! browser renderer is the primary display. Search/segmentation for
//! attachments lives in `notez-core`'s artifact extractors, which
//! remain independent of preview.
//!
//! The previewer declares `can_edit` so the attachment page offers the
//! generic replace-file edit affordance; a richer in-place DOCX editor
//! can be shipped later as a viewer-plugin extension.

use crate::{PreviewCapabilities, PreviewContext, PreviewError, PreviewModel, Previewer, ViewerFormat};

/// Previewer for DOCX attachments.
pub struct DocxPreviewer;

impl Previewer for DocxPreviewer {
    fn id(&self) -> &'static str {
        "docx"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".docx")
            || ctx.mime.as_deref()
                == Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    }

    fn capabilities(&self) -> PreviewCapabilities {
        PreviewCapabilities { can_edit: true }
    }

    fn render(&self, _ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        Ok(PreviewModel::Viewer {
            format: ViewerFormat::Docx,
        })
    }
}
