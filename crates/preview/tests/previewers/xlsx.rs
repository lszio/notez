use super::{phase_b_catalog, xlsx_ctx};
use bytes::Bytes;
use preview::PreviewModel;

const XLSX_BYTES: &[u8] = include_bytes!("../fixtures/tiny.xlsx");

#[test]
fn xlsx_matches_xlsx_attachment() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "xlsx")
        .expect("xlsx previewer registered");
    let c = xlsx_ctx(&cat, Bytes::from_static(XLSX_BYTES));
    assert!(p.matches(&c));
}

#[test]
fn xlsx_renders_sheets() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "xlsx")
        .expect("xlsx previewer registered");
    let c = xlsx_ctx(&cat, Bytes::from_static(XLSX_BYTES));
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Xlsx { sheets } = m else {
        panic!("expected PreviewModel::Xlsx");
    };
    assert!(!sheets.is_empty(), "expected at least one sheet");
    let s = &sheets[0];
    assert!(!s.name.is_empty(), "sheet name should be present");
    assert!(!s.rows.is_empty(), "first sheet should have at least one row");
}
