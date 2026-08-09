//! Landing page at `/`.
//!
//! Asks the server for registered spaces; if any are configured, the
//! picker in the header offers them. The home page itself shows the
//! onboarding card so the user knows how to add a space and run a
//! scan.
//!
//! We previously tried to auto-redirect to the first registered space
//! using `navigator.push(route_for_space_list(...))` from a
//! `use_effect` callback. Dioxus 0.7's `use_server_future` is a
//! fire-and-forget primitive outside a `SuspenseBoundary`, so the
//! resource was `None` during SSR; the redirect never fired; and the
//! page rendered the "loading registered spaces…" placeholder
//! forever. Replacing that with a hand-rolled server function and a
//! blocking render is too invasive for the v0.1 reader, so we just
//! drop the auto-redirect: the user picks a space from the picker
//! (or types a path) and the picker does the navigation.

use dioxus::prelude::*;

use crate::pages::PageHeader;

#[component]
pub fn HomePage() -> Element {
    rsx! {
        PageHeader {}
        main { class: "page",
            section { class: "onboard",
                h1 { "Notez" }
                p { class: "lede",
                    "a local-first, Org-mode-native knowledge federation engine."
                }

                ol { class: "steps",
                    li {
                        "Open the space picker — the button in the top-right corner labelled "
                        span { class: "mono", "select space…" }
                        "."
                    }
                    li {
                        "Type the absolute path of a directory that already contains "
                        code { "notez.toml" }
                        " or "
                        code { ".notez/" }
                        ", then press "
                        span { class: "mono", "open" }
                        "."
                    }
                    li {
                        "If the space has not been scanned yet, run "
                        code { "notez scan" }
                        " in that directory from your terminal. The list will populate the next time the page loads."
                    li {
                        "Click any space link in the picker to open its resource list. On the list page, use the "
                        span { class: "mono", "scan" }
                        " button to re-index the space, and the "
                        span { class: "mono", "watch" }
                        " button to start a filesystem watcher. The watcher records events; combine it with a manual scan to keep the index fresh."
                    }
                    li {
                        "If a space is missing from the picker, expand the "
                        span { class: "mono", "register a space" }
                        " section at the bottom of the picker and submit the form."
                    }
                }
                p { class: "lede dim",
                    "A space is a directory on disk — it is the single source of truth. The web client covers register / scan / watch; mutation is still terminal-only."
                 }
             }
         }
        }
    }
}
