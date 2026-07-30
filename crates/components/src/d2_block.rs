use dioxus::prelude::*;

/// Server-rendered placeholder for a D2 diagram. Mirrors `MermaidBlock` —
/// the client ESM hydrator will replace this with the rendered SVG.
#[component]
pub fn D2Block(source: String) -> Element {
    rsx! {
        div { class: "d2-block",
            "data-source": "{source}",
            pre { class: "d2-source", "{source}" }
        }
    }
}