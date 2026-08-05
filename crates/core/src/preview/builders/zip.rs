use crate::preview::{PreviewContext, PreviewError, PreviewModel, Previewer, ZipEntry};
use bytes::Bytes;
use std::io::Cursor;
use zip::ZipArchive;

/// Previewer for ZIP archive attachments.
///
/// Matches on a `.zip` locator suffix or the standard ZIP MIME types. The full
/// byte payload must be present in `ctx.bytes`; if it is missing the render
/// step returns `PreviewError::MissingBytes`. The bytes are interpreted as a
/// ZIP archive and every entry's `(name, size, is_dir)` triple is reported in
/// the `PreviewModel::Zip` payload.
pub struct ZipPreviewer;

impl Previewer for ZipPreviewer {
    fn id(&self) -> &'static str {
        "zip"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".zip")
            || ctx.mime.as_deref() == Some("application/zip")
            || ctx.mime.as_deref() == Some("application/x-zip-compressed")
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes: Bytes = ctx
            .bytes
            .clone()
            .ok_or_else(|| PreviewError::MissingBytes(ctx.resource.r#ref.to_string()))?;

        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| PreviewError::Extraction(format!("zip open: {e}")))?;

        let mut entries = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            let file = archive
                .by_index(i)
                .map_err(|e| PreviewError::Extraction(format!("zip entry: {e}")))?;
            entries.push(ZipEntry {
                path: file.name().to_string(),
                size: file.size(),
                is_dir: file.is_dir(),
            });
        }

        Ok(PreviewModel::Zip { entries })
    }
}
