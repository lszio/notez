use crate::{PreviewContext, PreviewError, PreviewModel, Previewer, PreviewerCatalog, Sheet};
use bytes::Bytes;
use calamine::{open_workbook_auto, Reader};
use std::io::Write;

/// Previewer for Excel (XLSX) attachments.
///
/// Matches on a `.xlsx` locator suffix or the
/// `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet` MIME.
/// The full byte payload must be present in `ctx.bytes`; if it is missing the
/// render step returns `PreviewError::MissingBytes`. The bytes are written to
/// a temporary file because the `calamine` XLSX reader needs a file path.
/// The temp file is removed automatically when the `NamedTempFile` is
/// dropped at the end of `render`.
pub struct XlsxPreviewer;

impl XlsxPreviewer {
    /// Convenience to register this previewer in a catalog.
    pub fn register(c: &mut PreviewerCatalog) {
        c.register(Self);
    }
}

impl Previewer for XlsxPreviewer {
    fn id(&self) -> &'static str {
        "xlsx"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".xlsx")
            || ctx
                .mime
                .as_deref()
                .map(|m| m == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
                .unwrap_or(false)
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes: Bytes = ctx
            .bytes
            .clone()
            .ok_or_else(|| PreviewError::MissingBytes(ctx.resource.r#ref.to_string()))?;

        // Spill to a temp file (calamine's XLSX reader wants Seek+file path).
        let safe = ctx.resource.r#ref.to_string().replace([':', '/'], "_");
        let mut tmp = tempfile::Builder::new()
            .prefix(&format!("notez-xlsx-{safe}-"))
            .suffix(".xlsx")
            .tempfile()
            .map_err(PreviewError::Io)?;
        tmp.write_all(&bytes).map_err(PreviewError::Io)?;

        let mut wb = open_workbook_auto(tmp.path())
            .map_err(|e| PreviewError::Extraction(format!("calamine open: {e}")))?;

        let sheets: Vec<Sheet> = wb
            .worksheets()
            .into_iter()
            .map(|(name, range)| Sheet {
                name,
                rows: range
                    .rows()
                    .map(|r| r.iter().map(|c| c.to_string()).collect())
                    .collect(),
            })
            .collect();

        // `tmp` is dropped at end of scope, which removes the file from disk.
        Ok(PreviewModel::Xlsx { sheets })
    }
}
