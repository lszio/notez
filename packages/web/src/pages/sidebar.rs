//! `SpaceSidebar` and `SpaceDropdown` — the space switcher.
//!
//! v0.2 lifts the space switcher into a top-nav dropdown. The sidebar
//! remains as a fallback used on the global home page (where there is
//! no active space to show a tree for); on every `/space/...` page the
//! top nav handles switching.
//!
//! The list is fetched via `use_server_future`; the SSR pass
//! suspends until the list is ready, then the rendered HTML is
//! static. No client hydration is required.

use dioxus::prelude::*;
use crate::router::route_for_space_list;
use crate::server::{list_registered_spaces, RegisteredSpaceDto};

/// Old "always-on" sidebar used on the global home page.
#[component]
pub fn SpaceSidebar(active_path: Option<String>) -> Element {
    let spaces_resource = use_server_future(|| async {
        list_registered_spaces().await.unwrap_or_default()
    })?;
    let sources: Vec<RegisteredSpaceDto> = spaces_resource.cloned().unwrap_or_default();

    let current_normalized = active_path
        .as_deref()
        .map(|p| std::fs::canonicalize(p).ok())
        .flatten()
        .map(|p| p.to_string_lossy().into_owned())
        .or(active_path.clone());

    rsx! {
        nav { class: "side", aria_label: "sources",
            details { class: "side-add space-picker-dropdown", open: true,
                summary { class: "side-label",
                    if let Some(ref p) = active_path {
                        span { class: "accent", "{p}" }
                    } else {
                        span { "sources" }
                    }
                }

                div { class: "side-head", style: "margin-top:0.5rem;",
                    span { class: "side-label", "registered" }
                    span { class: "side-count", "({sources.len()})" }
                }

                a {
                    class: if current_normalized.is_none() { "side-home-link is-current" } else { "side-home-link" },
                    href: "/",
                    "all notes"
                }

                if sources.is_empty() {
                    p { class: "side-empty",
                        "No sources yet. Use the form below."
                    }
                } else {
                    ul { class: "side-list",
                        for s in sources.iter() {
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
                        action: "/api/sources/register",
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
        }
    }
}

/// Top-left dropdown that switches the active space. Always visible
/// in the top nav bar.
#[component]
pub fn SpaceDropdown(active_path: Option<String>, active_encoded: Option<String>) -> Element {
    let spaces_resource = use_server_future(|| async {
        list_registered_spaces().await.unwrap_or_default()
    })?;
    let sources: Vec<RegisteredSpaceDto> = spaces_resource.cloned().unwrap_or_default();

    let current_normalized = active_path
        .as_deref()
        .map(|p| std::fs::canonicalize(p).ok())
        .flatten()
        .map(|p| p.to_string_lossy().into_owned())
        .or(active_path.clone());

    let leaf = active_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "(select space)".to_string());

    rsx! {
        details { class: "nav-source",
            summary { class: "nav-source-summary",
                span { class: "nav-source-mark", "◐" }
                span { class: "nav-source-name", "{leaf}" }
                if let Some(ref p) = active_path {
                    span { class: "nav-source-path", "{p}" }
                }
                span { class: "nav-source-caret", "▾" }
            }
            div { class: "nav-source-panel",
                div { class: "nav-source-head",
                    span { class: "nav-source-label", "sources" }
                    span { class: "nav-source-count", "({sources.len()})" }
                }
                a {
                    class: if current_normalized.is_none() { "nav-source-link is-current" } else { "nav-source-link" },
                    href: "/",
                    "all notes"
                }
                if let Some(enc) = active_encoded.as_ref() {
                    a {
                        class: "nav-source-link",
                        href: "/source/{enc}/list",
                        "this space · all resources"
                    }
                    a {
                        class: "nav-source-link",
                        href: "/source/{enc}/graph",
                        "this space · graph"
                    }
                }
                if sources.is_empty() {
                    p { class: "side-empty",
                        "No sources yet. Use the form below."
                    }
                } else {
                    ul { class: "side-list",
                        for s in sources.iter() {
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
                        action: "/api/sources/register",
                        method: "post",
                        label {
                            r#for: "nav-add-name",
                            "name"
                        }
                        input {
                            id: "nav-add-name",
                            r#type: "text",
                            name: "name",
                            placeholder: "personal",
                            required: true,
                        }
                        label {
                            r#for: "nav-add-path",
                            "absolute path"
                        }
                        input {
                            id: "nav-add-path",
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
        }
    }
}