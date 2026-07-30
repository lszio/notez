use dioxus::prelude::*;

/// Render an unordered list of `(name, root)` pairs as links to `/s/{name}`.
/// Each entry becomes an `<a>` so the user can navigate between spaces from
/// the landing index.
#[component]
pub fn SpaceList(entries: Vec<(String, std::path::PathBuf)>) -> Element {
    rsx! {
        ul { class: "space-list",
            for (name, root) in entries {
                li { key: "{name}",
                    a { href: "/s/{name}",
                        class: "space-link",
                        "{name}"
                        span { class: "space-root", " — {root.display()}" }
                    }
                }
            }
        }
    }
}