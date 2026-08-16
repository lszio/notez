//! Landing page at `/`.
//!
//! Three short, content-led steps. The real interactive surface
//! is the left sidebar (`SpaceSidebar`); the home page just tells
//! the user what to do.

use dioxus::prelude::*;

use crate::pages::PageHeader;

#[component]
pub fn HomePage() -> Element {
    rsx! {
        PageHeader {}
        div { class: "page",
            section { class: "onboard",
                h1 { "Notez" }
                p { class: "lede",
                    "a local-first, Org-mode-native knowledge federation engine."
                }
                ol { class: "steps",
                    li {
                        "Click any space link in the left sidebar to open its resource list. The sidebar is the index; pick from there to switch spaces."
                    }
                    li {
                        "To add a new space, expand "
                        span { class: "mono", "register a space" }
                        " at the bottom of the sidebar and submit the form."
                    }
                    li {
                        "On a list page, the header holds "
                        span { class: "mono", "scan" }
                        " / "
                        span { class: "mono", "watch" }
                        " / "
                        span { class: "mono", "graph" }
                        " actions. The watcher records filesystem events; combine it with a manual scan to keep the index fresh."
                    }
                }
                p { class: "lede dim",
                    "A space is a directory on disk — it is the single source of truth. The web client covers register / scan / watch / graph; mutation is still terminal-only."
                }
            }
        }
    }
}
