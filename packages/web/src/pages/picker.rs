//! SpaceSidebar — the fixed left sidebar that lists every
//! registered space and offers the register-a-space form.
//!
//! v0.3 of the web client moves the picker out of the page header
//! into a proper left column so the page main column can breathe.
//! The sidebar is rendered by `Layout`; it is identical on every
//! page so the user can switch spaces from anywhere.
//!
//! The list is fetched via `use_server_future`; the SSR pass
//! suspends until the list is ready, then the rendered HTML is
//! static. No client hydration is required.
//!
//! The "current space" indicator is the encoded path on the active
//! route (if any) compared against each entry's path. We do not
//! need `space_ctx` because the route segment is authoritative.

use dioxus::prelude::*;
use crate::router::route_for_space_list;
use crate::server::{list_registered_spaces, RegisteredSpaceDto};
/// Render the spaces sidebar.
///
/// `active_path`: the decoded path of the space the user is
/// currently inside, or `None` when on the home page. When
/// `Some`, the matching sidebar row gets the `.is-current`
/// modifier so the user can see where they are.
#[component]
pub fn SpaceSidebar(active_path: Option<String>) -> Element {
    let spaces_resource = use_server_future(|| async {
        list_registered_spaces().await.unwrap_or_default()
    })?;
    let spaces: Vec<RegisteredSpaceDto> = spaces_resource.cloned().unwrap_or_default();

    let current_normalized = active_path
        .as_deref()
        .map(|p| std::fs::canonicalize(p).ok())
        .flatten()
        .map(|p| p.to_string_lossy().into_owned())
        .or(active_path.clone());

    let current_encoded = active_path.as_deref().map(crate::router::encode_space).unwrap_or_default();
    rsx! {
        nav { class: "side", aria_label: "spaces",
            // ----- Top: Space Switcher Dropdown -----
            details { class: "side-add space-picker-dropdown", open: false,
                summary { class: "side-label",
                    "space · "
                    if let Some(ref p) = active_path {
                        span { class: "accent", "{p}" }
                    } else {
                        span { "select space" }
                    }
                }
                div { class: "side-head", style: "margin-top:0.5rem;",
                    span { class: "side-label", "spaces" }
                    span { class: "side-count", "({spaces.len()})" }
                }

                a {
                    class: if current_normalized.is_none() { "side-home-link is-current" } else { "side-home-link" },
                    href: "/",
                    "all notes"
                }

                if spaces.is_empty() {
                    p { class: "side-empty",
                        "No spaces yet. Use the form below."
                    }
                } else {
                    ul { class: "side-list",
                        for s in spaces.iter() {
                            {
                                let is_current = current_normalized
                                    .as_deref()
                                    .map(|cur| {
                                        std::fs::canonicalize(&s.path)
                                            .map(|p| p.to_string_lossy() == cur)
                                            .unwrap_or(false)
                                            || cur == s.path
                                    })
                                    .unwrap_or(false);
                                rsx! {
                                    li { class: "side-item", key: "{s.path}",
                                        a {
                                            class: if is_current { "side-link is-current" } else { "side-link" },
                                            href: "{route_for_space_list(&s.path)}",
                                            span { class: "side-link-name",
                                                "{s.name}"
                                                if s.source == "discovered" {
                                                    span { class: "side-tag", "found" }
                                                }
                                            }
                                            span { class: "side-link-path", "{s.path}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                details { class: "side-add", style: "margin-top:0.5rem;",
                    summary { "+ register a space" }
                    form {
                        class: "side-add-form",
                        action: "/api/spaces/register",
                        method: "post",
                        label {
                            r#for: "side-add-name",
                            "name"
                        }
                        input {
                            id: "side-add-name",
                            r#type: "text",
                            name: "name",
                            placeholder: "personal",
                            required: true,
                        }
                        label {
                            r#for: "side-add-path",
                            "absolute path"
                        }
                        input {
                            id: "side-add-path",
                            r#type: "text",
                            name: "path",
                            placeholder: "/absolute/path/to/space",
                            required: true,
                        }
                        button {
                            r#type: "submit",
                            "register"
                        }
                    }
                }
            }

            // ----- Lower: Directory Tree Section -----
            if !current_encoded.is_empty() {
                div { class: "side-tree-section", style: "margin-top:1.5rem; border-top:1px solid var(--ink-rule); padding-top:0.8rem;",
                    div { class: "side-head",
                        span { class: "side-label", "directory tree" }
                    }
                    a {
                        class: "side-home-link",
                        href: "/space/{current_encoded}/list",
                        "📁 / (all files)"
                    }
                }
            }
        }
    }
}
