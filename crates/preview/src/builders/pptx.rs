use crate::{PreviewContext, PreviewError, PreviewModel, Previewer, Slide};
use bytes::Bytes;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Previewer for PowerPoint (PPTX) attachments.
///
/// Matches on a `.pptx` locator suffix or the
/// `application/vnd.openxmlformats-officedocument.presentationml.presentation`
/// MIME. The full byte payload must be present in `ctx.bytes`; if it is missing
/// the render step returns `PreviewError::MissingBytes`. The bytes are
/// interpreted as a ZIP archive; for every member that lives under
/// `ppt/slides/slideN.xml` the textual content of every `<a:t>` element is
/// collected into a `Slide` body, and the first run is used as the title.
pub struct PptxPreviewer;

impl Previewer for PptxPreviewer {
    fn id(&self) -> &'static str {
        "pptx"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".pptx")
            || ctx.mime.as_deref() == Some(
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            )
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes: Bytes = ctx
            .bytes
            .clone()
            .ok_or_else(|| PreviewError::MissingBytes(ctx.resource.r#ref.to_string()))?;

        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| PreviewError::Extraction(format!("pptx zip: {e}")))?;

        let mut slides = Vec::new();
        let mut idx: u32 = 1;
        loop {
            let name = format!("ppt/slides/slide{idx}.xml");
            let Ok(mut entry) = archive.by_name(&name) else {
                break;
            };
            let mut xml = String::new();
            entry
                .read_to_string(&mut xml)
                .map_err(PreviewError::Io)?;
            let body = extract_text_runs(&xml)?;
            let title = body.first().cloned();
            slides.push(Slide {
                index: idx,
                title,
                body,
                notes: None,
            });
            idx += 1;
        }

        Ok(PreviewModel::Pptx { slides })
    }
}

/// Parse a slide XML document and collect every text run under `<a:t>` elements.
///
/// Text inside `<a:t>` may appear either as character data (`Event::Text`,
/// already XML-decoded by `quick_xml`) or inside a CDATA section
/// (`Event::CData`, raw bytes). We accept both. XML parse errors are
/// surfaced to the caller rather than silently swallowed, since a
/// truncated or malformed slide is a real bug, not an empty result.
fn extract_text_runs(xml: &str) -> Result<Vec<String>, PreviewError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut in_t = false;
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"a:t" => {
                in_t = true;
                buf.clear();
            }
            Ok(Event::Text(t)) if in_t => {
                if let Ok(unescaped) = t.unescape() {
                    buf.push_str(&unescaped);
                }
            }
            Ok(Event::CData(c)) if in_t => {
                let chunk = c.into_inner();
                buf.push_str(&String::from_utf8_lossy(&chunk));
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:t" => {
                in_t = false;
                if !buf.is_empty() {
                    out.push(std::mem::take(&mut buf));
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(PreviewError::Extraction(format!(
                    "pptx xml parse error: {e}"
                )));
            }
            _ => {}
        }
    }
    Ok(out)
}
