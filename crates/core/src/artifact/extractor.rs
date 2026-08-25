use crate::domain::SegmentRecord;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Unsupported MIME type: {0}")]
    UnsupportedMime(String),
    #[error("Extraction failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExtractedContent {
    pub text: String,
    pub metadata: BTreeMap<String, String>,
}

pub trait Extractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError>;
}

pub struct TextExtractor;

impl Extractor for TextExtractor {
    fn extract(&self, bytes: &[u8], _mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| ExtractionError::Failed(format!("Invalid UTF-8: {e}")))?
            .to_string();

        let mut metadata = BTreeMap::new();
        metadata.insert("char_count".to_string(), text.chars().count().to_string());

        Ok(ExtractedContent { text, metadata })
    }
}

pub struct ImageMetadataExtractor;

impl Extractor for ImageMetadataExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        let mut metadata = BTreeMap::new();
        metadata.insert("mime".to_string(), mime_type.to_string());
        metadata.insert("byte_size".to_string(), bytes.len().to_string());

        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            metadata.insert("dimensions".to_string(), format!("{width}x{height}"));
        } else {
            metadata.insert("dimensions".to_string(), "unknown".to_string());
        }

        Ok(ExtractedContent {
            text: String::new(),
            metadata,
        })
    }
}

pub struct SegmentSlicer {
    pub max_segment_size: usize,
    pub overlap_size: usize,
}

impl Default for SegmentSlicer {
    fn default() -> Self {
        Self {
            max_segment_size: 200,
            overlap_size: 20,
        }
    }
}

impl SegmentSlicer {
    pub fn new(max_segment_size: usize, overlap_size: usize) -> Self {
        Self {
            max_segment_size,
            overlap_size,
        }
    }

    pub fn slice(&self, attachment_ref: &str, text: &str) -> Vec<SegmentRecord> {
        if text.is_empty() {
            return Vec::new();
        }

        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();
        let mut segments = Vec::new();
        let mut start = 0;
        let mut idx = 1;

        while start < total {
            let end = (start + self.max_segment_size).min(total);
            let segment_text: String = chars[start..end].iter().collect();

            segments.push(SegmentRecord {
                id: format!("{attachment_ref}_seg_{idx}"),
                attachment_ref: attachment_ref.to_string(),
                text: segment_text,
                offset_start: start,
                offset_end: end,
            });

            idx += 1;
            if end == total {
                break;
            }
            start = end.saturating_sub(self.overlap_size);
        }

        segments
    }
}

// Binary-format extractors (PDF / XLSX / PPTX / ZIP).
// -----------------------------------------------------------------------------

const PDF_MIMES: &[&str] = &["application/pdf"];
const XLSX_MIMES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-excel",
];
const PPTX_MIMES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.ms-powerpoint",
];
const ZIP_MIMES: &[&str] = &["application/zip", "application/x-zip-compressed"];

fn mime_matches(mime: &str, allowed: &[&str]) -> bool {
    let lower = mime.to_ascii_lowercase();
    allowed.iter().any(|m| m.eq_ignore_ascii_case(&lower))
}

fn reject_if_empty(bytes: &[u8], label: &str) -> Result<(), ExtractionError> {
    if bytes.is_empty() {
        return Err(ExtractionError::Failed(format!("{label}: empty buffer")));
    }
    Ok(())
}

pub struct PdfExtractor;

impl PdfExtractor {
    pub fn target_extension(&self) -> Option<&'static str> {
        Some("pdf")
    }
}

impl Extractor for PdfExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, PDF_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "pdf")?;
        let doc =
            lopdf::Document::load_mem(bytes).map_err(|e| ExtractionError::Failed(e.to_string()))?;
        let pages = doc.get_pages();
        let mut text = String::new();
        for (i, _) in pages.iter().enumerate() {
            if let Ok(t) = doc.extract_text(&[(i + 1) as u32]) {
                text.push_str(&t);
                text.push('\n');
            }
        }
        let mut metadata = BTreeMap::new();
        metadata.insert("page_count".into(), pages.len().to_string());
        Ok(ExtractedContent { text, metadata })
    }
}

pub struct XlsxExtractor;

impl XlsxExtractor {
    pub fn target_extension(&self) -> Option<&'static str> {
        Some("xlsx")
    }
}

impl Extractor for XlsxExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, XLSX_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "xlsx")?;
        let mut file = NamedTempFile::new().map_err(|e| ExtractionError::Failed(e.to_string()))?;
        file.write_all(bytes)
            .map_err(|e| ExtractionError::Failed(e.to_string()))?;
        use calamine::Reader;
        let mut workbook = calamine::open_workbook_auto(file.path())
            .map_err(|e| ExtractionError::Failed(e.to_string()))?;
        let mut sheets = Vec::new();
        let mut text = String::new();
        for name in workbook.sheet_names().to_owned() {
            if let Ok(range) = workbook.worksheet_range(&name) {
                let rows: Vec<Vec<String>> = range
                    .rows()
                    .map(|r| r.iter().map(|c| c.to_string()).collect())
                    .collect();
                text.push_str(&rows.iter().flatten().cloned().collect::<Vec<_>>().join(" "));
                text.push('\n');
                sheets.push(serde_json::json!({"name": name, "rows": rows}));
            }
        }
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "sheets_json".into(),
            serde_json::to_string(&sheets).map_err(|e| ExtractionError::Failed(e.to_string()))?,
        );
        Ok(ExtractedContent { text, metadata })
    }
}

pub struct PptxExtractor;

impl PptxExtractor {
    pub fn target_extension(&self) -> Option<&'static str> {
        Some("pptx")
    }
}

impl Extractor for PptxExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, PPTX_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "pptx")?;
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| ExtractionError::Failed(e.to_string()))?;
        let mut slides = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| ExtractionError::Failed(e.to_string()))?;
            let name = entry.name().to_string();
            if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                let mut xml = String::new();
                entry
                    .read_to_string(&mut xml)
                    .map_err(|e| ExtractionError::Failed(e.to_string()))?;
                let mut reader = quick_xml::Reader::from_str(&xml);
                reader.config_mut().trim_text(true);
                let mut buf = Vec::new();
                let mut vals = Vec::new();
                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(quick_xml::events::Event::Start(e)) if e.name().as_ref() == b"a:t" => {
                            if let Ok(quick_xml::events::Event::Text(t)) =
                                reader.read_event_into(&mut buf)
                            {
                                vals.push(
                                    t.unescape()
                                        .map_err(|e| ExtractionError::Failed(e.to_string()))?
                                        .into_owned(),
                                );
                            }
                        }
                        Ok(quick_xml::events::Event::Eof) => break,
                        Err(e) => return Err(ExtractionError::Failed(e.to_string())),
                        _ => {}
                    }
                    buf.clear();
                }
                slides.push(vals);
            }
        }
        let text = slides
            .iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "slides_json".into(),
            serde_json::to_string(&slides).map_err(|e| ExtractionError::Failed(e.to_string()))?,
        );
        Ok(ExtractedContent { text, metadata })
    }
}

pub struct ZipExtractor;

impl ZipExtractor {
    pub fn target_extension(&self) -> Option<&'static str> {
        Some("zip")
    }
}

impl Extractor for ZipExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, ZIP_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "zip")?;
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| ExtractionError::Failed(e.to_string()))?;
        let mut entries = Vec::new();
        let mut names = Vec::new();
        for i in 0..archive.len() {
            let entry = archive
                .by_index(i)
                .map_err(|e| ExtractionError::Failed(e.to_string()))?;
            names.push(entry.name().to_string());
            entries.push((entry.name().to_string(), entry.size(), entry.is_dir()));
        }
        let mut metadata = BTreeMap::new();
        metadata.insert(
            "entries_json".into(),
            serde_json::to_string(&entries).map_err(|e| ExtractionError::Failed(e.to_string()))?,
        );
        Ok(ExtractedContent {
            text: names.join("\n"),
            metadata,
        })
    }
}
