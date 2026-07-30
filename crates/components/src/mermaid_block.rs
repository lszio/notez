use dioxus::prelude::*;

/// Server-rendered placeholder for a Mermaid diagram. The client-side ESM
/// hydrator (Task D4) scans for `.mermaid-block` and replaces this `<div>`
/// with the rendered SVG. We emit the source verbatim inside the same
/// element so the hydrator can recover it without a second round-trip.
#[component]
pub fn MermaidBlock(source: String) -> Element {
    rsx! {
        div { class: "mermaid-block",
            "data-source": "{source}",
            pre { class: "mermaid-source", "{source}" }
        }
    }
}