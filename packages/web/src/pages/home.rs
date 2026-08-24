//! Landing page at `/` — global onboarding.
//!
//! The new shell's left column renders the per-space sidebar here
//! (the home page has no active space). The center shows a short
//! overview; the search palette is reachable from the top nav.

use dioxus::prelude::*;

#[component]
pub fn HomePage() -> Element {
    rsx! {
        div { class: "page",
            section { class: "onboard",
                h1 { "Notez" }
                p { class: "lede",
                    "a local-first, Org-mode-native knowledge federation engine."
                }
                ol { class: "steps",
                    li {
                        "Pick a space from the dropdown in the top-left, or register one with the form below."
                    }
                    li {
                        "Inside a space, the left column shows the directory tree and the source-file list; the right column shows properties and the resource graph."
                    }
                    li {
                        "Hit "
                        span { class: "mono", "⌘K" }
                        " (or click the search bar) to open the command palette and jump anywhere."
                    }
                }
                p { class: "lede dim",
                    "Files on disk are authoritative: every doc, heading, attachment and block lives in the original .org / .md files; the SQLite projection is rebuilt with `notez scan`."
                }
            }
        }
    }
}