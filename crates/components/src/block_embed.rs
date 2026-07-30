use dioxus::prelude::*;
use domain::ResourceRef;

/// A `[[block:…]]` reference resolved by `BlockEmbedPreviewer`. The
/// `html` payload was produced server-side from a trusted markdown source
/// by the previewer, so we emit it via `dangerous_inner_html` to preserve
/// formatting. The `source` reference is preserved in `data-source` for
/// inspection / debugging.
#[component]
pub fn BlockEmbed(source: ResourceRef, html: String) -> Element {
    rsx! {
        div { class: "block-embed",
            "data-source": "{source}",
            div { class: "block-embed-body",
                div { dangerous_inner_html: "{html}" }
            }
        }
    }
}