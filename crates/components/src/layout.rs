use dioxus::prelude::*;

/// Page chrome — produces a fragment with `<head><title>…</title></head>`
/// and `<body>…children…</body>`. The actual `<html>` element is added by the
/// server-side renderer (see `crates/web`), which wraps the rendered fragment
/// in the full HTML document. Dioxus 0.6 does not define an `html` element,
/// so we emit the document-metadata and content as siblings.
#[component]
pub fn Layout(title: String, children: Element) -> Element {
    rsx! {
        Fragment {
            head { title { "{title}" } }
            body { {children} }
        }
    }
}