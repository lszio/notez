use artifact::extractor::{Extractor, PdfExtractor, PptxExtractor, XlsxExtractor, ZipExtractor};
use std::io::Write;

const PDF: &[u8] = include_bytes!("fixtures/tiny.pdf");
const XLSX: &[u8] = include_bytes!("fixtures/tiny.xlsx");
const PPTX: &[u8] = include_bytes!("fixtures/tiny.pptx");

fn build_zip() -> Vec<u8> {
    let buf = std::io::Cursor::new(Vec::<u8>::new());
    let mut zip = zip::write::ZipWriter::new(buf);
    zip.start_file("hello.txt", zip::write::FileOptions::default()).unwrap();
    zip.write_all(b"hello").unwrap();
    zip.add_directory("nested/", zip::write::FileOptions::default()).unwrap();
    zip.start_file("nested/inner.txt", zip::write::FileOptions::default()).unwrap();
    zip.write_all(b"world").unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn pdf_extractor_reports_page_count() {
    let r = PdfExtractor.extract(PDF, "application/pdf").unwrap();
    assert_eq!(r.metadata.get("page_count").map(String::as_str), Some("1"));
}

#[test]
fn xlsx_extractor_emits_sheets_json() {
    let r = XlsxExtractor.extract(XLSX, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet").unwrap();
    let sheets_json = r.metadata.get("sheets_json").expect("sheets_json present");
    let v: serde_json::Value = serde_json::from_str(sheets_json).unwrap();
    assert!(v.is_array() && !v.as_array().unwrap().is_empty());
}

#[test]
fn pptx_extractor_emits_slides_json() {
    let r = PptxExtractor.extract(PPTX, "application/vnd.openxmlformats-officedocument.presentationml.presentation").unwrap();
    let slides_json = r.metadata.get("slides_json").expect("slides_json present");
    let v: serde_json::Value = serde_json::from_str(slides_json).unwrap();
    assert!(v.is_array() && !v.as_array().unwrap().is_empty());
}

#[test]
fn zip_extractor_lists_entries() {
    let bytes = build_zip();
    let r = ZipExtractor.extract(&bytes, "application/zip").unwrap();
    let entries_json = r.metadata.get("entries_json").expect("entries_json present");
    let v: serde_json::Value = serde_json::from_str(entries_json).unwrap();
    let arr = v.as_array().unwrap();
    assert!(arr.iter().any(|e| e[0] == "hello.txt"));
    assert!(r.text.contains("hello.txt"));
}
