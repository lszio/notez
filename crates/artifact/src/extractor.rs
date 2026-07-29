use domain::SegmentRecord;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

// -----------------------------------------------------------------------------
// Binary-format extractors (PDF / XLSX / PPTX / ZIP).
//
// Each extractor guards on MIME type and rejects empty buffers. Phase A only
// asserts the shape: the four types exist, target their extension, and fail
// cleanly for empty bytes. Per-format fixture-based tests are deferred to a
// follow-up that synthesises fixtures via the real crates (see plan §A3).
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
    pub fn target_extension(&self) -> Option<&'static str> { Some("pdf") }
}

impl Extractor for PdfExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, PDF_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "pdf")?;
        let doc = lopdf::Document::load_mem(bytes)
            .map_err(|e| ExtractionError::Failed(format!("lopdf load: {e}")))?;
        let page_count = doc.get_pages().len() as u64;
        let mut metadata = BTreeMap::new();
        metadata.insert("format".to_string(), "pdf".to_string());
        metadata.insert("pages".to_string(), page_count.to_string());
        Ok(ExtractedContent { text: String::new(), metadata })
    }
}

pub struct XlsxExtractor;

impl XlsxExtractor {
    pub fn target_extension(&self) -> Option<&'static str> { Some("xlsx") }
}

impl Extractor for XlsxExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, XLSX_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "xlsx")?;
        let mut metadata = BTreeMap::new();
        metadata.insert("format".to_string(), "xlsx".to_string());
        metadata.insert("byte_size".to_string(), bytes.len().to_string());
        Ok(ExtractedContent { text: String::new(), metadata })
    }
}

pub struct PptxExtractor;

impl PptxExtractor {
    pub fn target_extension(&self) -> Option<&'static str> { Some("pptx") }
}

impl Extractor for PptxExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, PPTX_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "pptx")?;
        let mut metadata = BTreeMap::new();
        metadata.insert("format".to_string(), "pptx".to_string());
        metadata.insert("byte_size".to_string(), bytes.len().to_string());
        Ok(ExtractedContent { text: String::new(), metadata })
    }
}

pub struct ZipExtractor;

impl ZipExtractor {
    pub fn target_extension(&self) -> Option<&'static str> { Some("zip") }
}

impl Extractor for ZipExtractor {
    fn extract(&self, bytes: &[u8], mime_type: &str) -> Result<ExtractedContent, ExtractionError> {
        if !mime_matches(mime_type, ZIP_MIMES) {
            return Err(ExtractionError::UnsupportedMime(mime_type.to_string()));
        }
        reject_if_empty(bytes, "zip")?;
        let mut metadata = BTreeMap::new();
        metadata.insert("format".to_string(), "zip".to_string());
        metadata.insert("byte_size".to_string(), bytes.len().to_string());
        Ok(ExtractedContent { text: String::new(), metadata })
    }
}
