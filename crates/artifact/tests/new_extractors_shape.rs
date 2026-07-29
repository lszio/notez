//! Shape test for the four new extractors (PDF / XLSX / PPTX / ZIP).
//!
//! Constructing valid 1-page PDFs, XLSX, PPTX, and ZIP fixtures by hand is
//! impractical and brittle (each is a multi-section binary container). Phase A
//! only requires the extractors exist with the right names, target the right
//! MIME/extension, and fail cleanly for empty bytes. The fixtures tests are
//! deferred until we adopt proper fixture generation (e.g. a `tests/fixtures/`
//! helper that uses the real crates to round-trip) — see follow-up in plan §A3.

use artifact::extractor::{
    Extractor, ExtractionError, PdfExtractor, PptxExtractor, XlsxExtractor, ZipExtractor,
};

#[test]
fn new_extractors_exist_and_target_their_extension() {
    let pdf = PdfExtractor;
    let xlsx = XlsxExtractor;
    let pptx = PptxExtractor;
    let zip = ZipExtractor;

    assert_eq!(pdf.target_extension(), Some("pdf"));
    assert_eq!(xlsx.target_extension(), Some("xlsx"));
    assert_eq!(pptx.target_extension(), Some("pptx"));
    assert_eq!(zip.target_extension(), Some("zip"));
}

#[test]
fn new_extractors_reject_empty_bytes() {
    let empty: &[u8] = &[];

    let r1 = PdfExtractor.extract(empty, "application/pdf");
    let r2 = XlsxExtractor.extract(empty, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
    let r3 = PptxExtractor.extract(empty, "application/vnd.openxmlformats-officedocument.presentationml.presentation");
    let r4 = ZipExtractor.extract(empty, "application/zip");

    for r in [r1, r2, r3, r4] {
        assert!(
            matches!(r, Err(ExtractionError::Failed(_)) | Err(ExtractionError::UnsupportedMime(_))),
            "expected Failed/Unsupported for empty bytes, got {r:?}"
        );
    }
}

#[test]
fn new_extractors_reject_unrelated_mime() {
    let bytes = b"hello world";

    let r = PdfExtractor.extract(bytes, "text/plain");
    assert!(
        matches!(r, Err(ExtractionError::UnsupportedMime(_))),
        "expected UnsupportedMime for non-PDF mime, got {r:?}"
    );
}