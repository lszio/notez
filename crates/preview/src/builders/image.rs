use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};

/// Previewer for binary image attachments.
///
/// Matches on any MIME that starts with `image/`. The render step emits a
/// `PreviewModel::Image` whose `src` is a synthetic URL path
/// (`/s/<source_id>/a/<ref_id>`) that the components/web layer can resolve to
/// the underlying byte payload. Width/height are parsed from the byte payload
/// when the format is recognisable (currently PNG only); otherwise `(0, 0)`.
pub struct ImagePreviewer;

impl Previewer for ImagePreviewer {
    fn id(&self) -> &'static str {
        "image"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.mime
            .as_deref()
            .map(|m| m.starts_with("image/"))
            .unwrap_or(false)
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let mime = ctx.mime.clone().unwrap_or_default();
        let (width, height) = ctx
            .bytes
            .as_deref()
            .map(|b| dimensions_from_bytes(b))
            .unwrap_or((0, 0));
        let ref_id = ctx.resource.r#ref.to_string().replace(':', "_");
        let src = format!("/s/{}/a/{}", ctx.resource.source_id, ref_id);
        Ok(PreviewModel::Image {
            src,
            width,
            height,
            mime,
        })
    }
}

/// Extract `(width, height)` from a byte payload for recognised formats.
///
/// Only PNG (8-byte signature `[0x89, b'P', b'N', b'G', ...]`) is supported
/// here. For every other format the dimensions default to `(0, 0)`, which the
/// client treats as "unknown size".
fn dimensions_from_bytes(b: &[u8]) -> (u32, u32) {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && b.len() >= 24 {
        let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
        return (w, h);
    }
    (0, 0)
}
