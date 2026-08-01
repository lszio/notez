use crate::preview::{PdfPage, PreviewContext, PreviewError, PreviewModel, Previewer, PreviewerCatalog};
use bytes::Bytes;
use lopdf::Document;

/// Previewer for PDF attachments.
///
/// Matches on either the declared MIME (`application/pdf`) or a `.pdf` locator
/// suffix. The full byte payload must be present in `ctx.bytes`; if it is
/// missing the render step returns `PreviewError::MissingBytes`.
pub struct PdfPreviewer;

impl PdfPreviewer {
    /// Convenience to register this previewer in a catalog.
    pub fn register(c: &mut PreviewerCatalog) {
        c.register(Self);
    }
}

impl Previewer for PdfPreviewer {
    fn id(&self) -> &'static str {
        "pdf"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.mime.as_deref() == Some("application/pdf")
            || ctx.locator.to_string_lossy().ends_with(".pdf")
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes: Bytes = ctx
            .bytes
            .clone()
            .ok_or_else(|| PreviewError::MissingBytes(ctx.resource.r#ref.to_string()))?;
        let doc = Document::load_mem(&bytes)
            .map_err(|e| PreviewError::Extraction(format!("lopdf load: {e}")))?;
        let mut pages: Vec<PdfPage> = doc
            .get_pages()
            .keys()
            .copied()
            .map(|i| {
                let text = doc.extract_text(&[i]).unwrap_or_default();
                PdfPage { index: i, text }
            })
            .collect();
        pages.sort_by_key(|p| p.index);
        let full_text = pages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(PreviewModel::Pdf { pages, text: full_text })
    }
}
