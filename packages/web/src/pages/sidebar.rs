//! `SpaceDropdown` — the source switcher at the top of the sidebar.
//!
//! Lists every registered (and auto-discovered) source plus any
//! remote namespaces of the active source. Registering a new source
//! is a plain POST form; the list itself is fetched with
//! `use_server_future` and fully SSR-rendered.

use dioxus::prelude::*;
use crate::router::route_for_space_home;
use crate::server::{list_registered_spaces, list_source_namespaces, RegisteredSpaceDto, SourceNamespaceDto};

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
        .unwrap_or_else(|| "all sources".to_string());

    let _ = &active_encoded; // retained for symmetry with callers

    rsx! {
        details { class: "nav-source",
            summary { class: "nav-source-summary",
                span { class: "nav-source-mark", "◐" }
                span { class: "nav-source-name", "{leaf}" }
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
                    "home"
                }
                if sources.is_empty() {
                    p { class: "side-empty",
                        "No sources yet — register one below."
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
                                            href: "{route_for_space_home(&s.path)}",
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
                if let Some(path) = active_path.as_ref() {
                    RemoteNamespaces { source_root: path.clone() }
                }
                details { class: "side-add",
                    summary { "+ register a source" }
                    form {
                        class: "side-add-form",
                        action: "/api/sources/register",
                        method: "post",
                        label { r#for: "nav-add-name", "name" }
                        input {
                            id: "nav-add-name",
                            r#type: "text",
                            name: "name",
                            placeholder: "personal",
                            required: true,
                        }
                        label { r#for: "nav-add-path", "absolute path" }
                        input {
                            id: "nav-add-path",
                            r#type: "text",
                            name: "path",
                            placeholder: "/absolute/path/to/space",
                            required: true,
                        }
                        button { r#type: "submit", "register" }
                    }
                }
            }
        }
    }
}

#[component]
fn RemoteNamespaces(source_root: String) -> Element {
    let remote_resource = use_server_future(move || {
        let root = source_root.clone();
        async move { list_source_namespaces(root).await.unwrap_or_default() }
    })?;
    let remotes: Vec<SourceNamespaceDto> = remote_resource.cloned().unwrap_or_default();
    if remotes.is_empty() {
        return rsx! { Fragment {} };
    }
    rsx! {
        section { class: "side-remotes", "aria-label": "remote sources",
            div { class: "side-head",
                span { class: "side-label", "remote namespaces" }
                span { class: "side-count", "({remotes.len()})" }
            }
            ul { class: "side-list",
                for remote in remotes.iter() {
                    li { class: "side-item remote-source", key: "{remote.id}",
                        div { class: "side-link",
                            span { class: "side-link-name", "{remote.id}" }
                            span { class: "side-link-path mono-sm", "{remote.kind}" }
                            span { class: "side-tag", "read-only" }
                            if remote.status == "ready" {
                                if let Some(caps) = remote.capabilities.as_ref() {
                                    span { class: "side-tag", "read" }
                                    if caps.can_import { span { class: "side-tag", "import" } }
                                    if caps.can_watch { span { class: "side-tag", "watch" } }
                                }
                            } else {
                                span { class: "side-tag is-error", "unavailable" }
                                if let Some(reason) = remote.reason.as_ref() {
                                    span { class: "side-link-path err-text", "{reason}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
