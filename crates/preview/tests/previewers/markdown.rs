use super::{markdown_ctx, phase_b_catalog};
use preview::PreviewModel;

#[test]
fn markdown_matches_md_locator() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "markdown")
        .expect("markdown previewer registered");
    let c = markdown_ctx(&cat, "# Title\n");
    assert!(p.matches(&c));
}

#[test]
fn markdown_renders_html_body() {
    let cat = phase_b_catalog();
    let p = cat
        .iter()
        .find(|p| p.id() == "markdown")
        .expect("markdown previewer registered");
    let body = "# Hello\n\n*world* and `code`\n";
    let c = markdown_ctx(&cat, body);
    let m = p.render(&c).expect("render ok");
    let PreviewModel::Markdown { html } = m else {
        panic!("expected PreviewModel::Markdown");
    };
    assert!(!html.is_empty());
    assert!(html.contains("<h1"), "expected <h1 in: {html}");
    assert!(html.contains("<em>world</em>"), "expected <em> in: {html}");
    assert!(html.contains("<code>code</code>"), "expected <code> in: {html}");
}
