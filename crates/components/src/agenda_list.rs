use dioxus::prelude::*;
use domain::ResourceRef;

/// A single row in the agenda (todo/next/done list) — title plus lifecycle
/// state (`TODO` / `NEXT` / `DONE` / …) and a locator pointing back at the
/// originating resource. The `r_ref` is preserved so callers can build deep
/// links into the space.
#[derive(PartialEq, Clone, Debug)]
pub struct TaskItem {
    pub title: String,
    pub state: String,
    pub locator: String,
    pub r_ref: ResourceRef,
}

/// Render an ordered list of agenda items as a semantic `<ul class="agenda">`.
/// Each `<li>` carries a `task-<state>` class so the client CSS can colour
/// states consistently (e.g. `.task-DONE` muted, `.task-NEXT` highlighted).
#[component]
pub fn AgendaList(items: Vec<TaskItem>) -> Element {
    rsx! {
        ul { class: "agenda",
            for t in items {
                li { key: "{t.r_ref}",
                    class: "task-{t.state}",
                    "{t.state} {t.title} ({t.locator})"
                }
            }
        }
    }
}