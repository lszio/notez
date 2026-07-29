use super::{pdf_ctx, phase_b_catalog};
use bytes::Bytes;
use preview::PreviewModel;

const PDF_BYTES: &[u8] = include_bytes!("../fixtures/tiny.pdf");

#[test]
fn pdf_matches_pdf_attachment() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "pdf")
        .expect("pdf previewer registered");
    let c = pdf_ctx(&cat, Bytes::from_static(PDF_BYTES));
    assert!(p.matches(&c));
}

#[test]
fn pdf_renders_pages() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "pdf")
        .expect("pdf previewer registered");
    let c = pdf_ctx(&cat, Bytes::from_static(PDF_BYTES));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Pdf { pages, text } = m else {
        panic!("expected PreviewModel::Pdf");
    };
    assert!(!pages.is_empty(), "expected at least one page");
    // text may be empty for an image-only PDF, but pages vec must be non-empty.
    let _ = text;
}
