//! SpacePicker — a navigation aid for switching spaces.
//!
//! The Dioxus fullstack web client runs without a hydrated WASM
//! bundle: every interaction is a plain HTML form submit or anchor
//! click. The picker reflects that:
//!
//! - A short table of every registered / discovered space, each row
//!   a link to `/space/<encoded>/list`.
//! - A `<details>` block with a form to register a new space by path.
//!   Submitting POSTs to `/api/spaces/register` and the server
//!   redirects to the new space's list page on success.
//!
//! The list is fetched via `use_server_future`; the SSR pass
//! suspends until the list is ready, then the rendered HTML is
//! static. No client hydration is required. That is the right
//! trade-off for a v0.1 reader that must work without a client
//! bundle.
//!
//! Visual style: an old filing-cabinet drawer. The picker label
//! reads "spaces"; each option is a small index-card with the space
//! name and absolute path.

use dioxus::prelude::*;
use crate::pages::ui::{Breadcrumb, BreadcrumbSegment};
use crate::router::route_for_space_list;
use crate::server::{list_registered_spaces, RegisteredSpaceDto};

#[component]
pub fn SpacePicker() -> Element {
    // `use_server_future` blocks the SSR render until the future
    // resolves, so the list is already in the HTML on first paint.
    // Without client hydration, the value is read once.
    let spaces_resource = use_server_future(|| async {
        list_registered_spaces().await.unwrap_or_default()
    })?;

    let spaces = spaces_resource.cloned().unwrap_or_default();

    rsx! {
        section { class: "picker",

            div { class: "picker-head",
                span { class: "picker-label", "spaces" }
                span { class: "picker-count", "({spaces.len()})" }
            }
            if spaces.is_empty() {
                p { class: "picker-empty",
                    "No spaces yet. Register a path below, or run "
                    code { "notez space register <name> --path <dir>" }
                    " from a terminal to add one permanently."
                }
            } else {
                ul { class: "picker-list",
                    for s in spaces.iter() {
                        li { class: "picker-row",
                            a {
                                class: "picker-link",
                                href: "{route_for_space_list(&s.path)}",
                                span { class: "picker-name", "{s.name}" }
                                span { class: "picker-path", "{s.path}" }
                                if s.source == "discovered" {
                                    span { class: "picker-tag", "found" }
                                }
                            }
                        }
                    }
                }
            }
            details { class: "picker-add",
                summary { class: "picker-add-summary", "register a space" }
                form {
                    class: "picker-add-form",
                    action: "/api/spaces/register",
                    method: "post",
                    label {
                        class: "picker-add-label",
                        r#for: "picker-add-name",
                        "name"
                    }
                    input {
                        id: "picker-add-name",
                        class: "picker-add-input",
                        r#type: "text",
                        name: "name",
                        placeholder: "personal",
                        required: true,
                    }
                    label {
                        class: "picker-add-label",
                        r#for: "picker-add-path",
                        "absolute path"
                    }
                    input {
                        id: "picker-add-path",
                        class: "picker-add-input",
                        r#type: "text",
                        name: "path",
                        placeholder: "/absolute/path/to/space",
                        required: true,
                    }
                    button {
                        class: "picker-add-go",
                        r#type: "submit",
                        "register"
                    }
                }
            }
        }
    }
}
