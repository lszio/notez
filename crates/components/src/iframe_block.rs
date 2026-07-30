use dioxus::prelude::*;

/// Sandboxed `<iframe>` for embedding arbitrary URLs (`[[iframe:…]]`).
/// `sandbox` is emitted as a raw string attribute because dioxus 0.6.3
/// does not yet type the `sandbox` attribute (see TODO in
/// `dioxus-html/src/elements.rs`).
#[component]
pub fn IframeBlock(src: String, sandbox: String) -> Element {
    rsx! {
        div { class: "iframe-block",
            iframe {
                class: "iframe-embed",
                src: "{src}",
                "sandbox": "{sandbox}"
            }
        }
    }
}