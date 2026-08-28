use dioxus::prelude::*;
use ui::{NzBadge, NzCard};

#[component]
pub fn Home() -> Element {
    rsx! {
        main { class: "work-surface",
            NzCard { padded: true,
                h1 { "Notez" }
                p { "Knowledge workspace" }
                NzBadge { text: "Desktop shell".to_string(), tone: "muted".to_string() }
            }
        }
    }
}
