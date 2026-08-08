//! Landing page at `/`.
//!
//! Asks the server for registered spaces; if any are configured,
//! push to the first one's list. Otherwise show an onboarding card
//! that explains the three steps a new user needs to take: open
//! the picker, type a path, then scan the space.

use dioxus::prelude::*;

use crate::pages::PageHeader;
use crate::router::route_for_space_list;
use crate::server::list_registered_spaces;

#[component]
pub fn HomePage() -> Element {
    let navigator = use_navigator();
    let mut loaded = use_signal(|| false);

    use_future(move || async move {
        if let Ok(list) = list_registered_spaces().await {
            if let Some(first) = list.first() {
                navigator.push(route_for_space_list(&first.path));
                return;
            }
        }
        loaded.set(true);
    });

    rsx! {
        PageHeader {}
        main { class: "page",
            section { class: "onboard",
                h1 { "Notez" }
                p { class: "lede",
                    if loaded() {
                        "a local-first, Org-mode-native knowledge federation engine."
                    } else {
                        "loading registered spaces…"
                    }
                }

                if loaded() {
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
                        }
                    }
                    p { class: "lede dim",
                        "A space is a directory on disk — it is the single source of truth. The web reader is read-only."
                    }
                }
            }
        }
    }
}
