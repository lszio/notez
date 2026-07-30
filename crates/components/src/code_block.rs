use dioxus::prelude::*;

/// Render a fenced code block. The `lang` value is emitted as a class on the
/// `<code>` element so the client can apply syntax highlighting (CSS-based
/// or via a downstream hydration step). Source content is HTML-escaped so
/// arbitrary user input cannot break out of the `<code>` node.
#[component]
pub fn CodeBlock(lang: String, source: String) -> Element {
    let escaped = crate::escape::escape_html(&source);
    let lang_class = if lang.is_empty() {
        "lang-plaintext".to_string()
    } else {
        format!("lang-{lang}")
    };
    rsx! {
        pre { class: "code-block",
            code { class: "{lang_class}",
                dangerous_inner_html: "{escaped}"
            }
        }
    }
}