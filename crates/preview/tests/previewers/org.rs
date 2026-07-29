use super::{first_heading, org_ctx, phase_b_catalog};
use preview::PreviewModel;

#[test]
fn org_matches_document() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "org")
        .expect("org previewer registered");
    let c = org_ctx(&cat, "* Hi\n");
    assert!(p.matches(&c));
}

#[test]
fn org_renders_document_with_body() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "org")
        .expect("org previewer registered");
    let body = "#+TITLE: Demo\n\n* Hello World\nbody text\n";
    let c = org_ctx(&cat, body);
    let m = p.render(&c).expect("render ok");
    let heading = first_heading(&m);
    assert_eq!(heading.title, "Hello World");
    assert_eq!(heading.anchor, "hello-world");
    let PreviewModel::Org { html, outline } = m else {
        panic!("expected PreviewModel::Org");
    };
    assert!(html.contains("<h1"), "html missing <h1: {html}");
    assert!(html.contains("id=\"hello-world\""));
    assert!(!outline.is_empty());
}
