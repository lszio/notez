use dioxus::prelude::*;
use preview::Heading;

/// Render a flat `<ul>` outline from a sequence of headings. Each heading
/// is wrapped in an anchor link to its `anchor` so the client CSS can
/// style indentation by heading depth.
#[component]
pub fn Outline(headings: Vec<Heading>) -> Element {
    rsx! {
        aside { class: "outline",
            if headings.is_empty() {
                p { class: "outline-empty", "No headings." }
            } else {
                ul { class: "outline-list",
                    for h in headings {
                        li { key: "{h.anchor}",
                            class: "outline-item level-{h.level}",
                            a { href: "#{h.anchor}",
                                "{crate::escape::escape_html(&h.title)}"
                            }
                        }
                    }
                }
            }
        }
    }
}