//! App shell — top nav bar + 3-column body.

use dioxus::prelude::*;

use crate::pages::{
    CommandPalette, FilesPanel, GraphPanel, PropertiesPanel, SearchTrigger, SpaceDropdown, SpaceSidebar, TreePanel,
};
use crate::space_ctx::{SpaceState, SpaceStatus};

#[component]
pub fn Layout(children: Element) -> Element {
    use_context_provider(|| Signal::new(None::<SpaceState>));
    use_context_provider(|| Signal::new(None::<String>));
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space_ctx().map(|s| s.path.clone());
    let active_path_for_side = active_path.clone();
    let active_encoded = space_ctx().map(|s| s.encoded.clone());

    rsx! {
        div { class: "shell",
            // ----- Global error banner (above the spine) -----
            {
                let s = space_ctx();
                if let Some(SpaceState { status: SpaceStatus::Error(e), path, .. }) = s.clone() {
                    rsx! {
                        div { class: "banner-err",
                            div { class: "banner-err-inner",
                                span { class: "label", "space error →" }
                                span { class: "mono-sm", "{path}" }
                                span { class: "label", "—" }
                                span { "{e}" }
                            }
                        }
                    }
                } else {
                    rsx! { Fragment {} }
                }
            }

            // ----- Top navigation bar -----
            nav { class: "topnav",
                div { class: "topnav-inner",
                    SpaceSwitcher {
                        active_path: active_path_for_side.clone(),
                        active_encoded: active_encoded.clone(),
                    }
                    SearchTrigger {}
                    div { class: "topnav-spacer" }
                    TopNavActions {
                        active_path: active_path_for_side.clone(),
                        active_encoded: active_encoded.clone(),
                    }
                }
            }

            // ----- 3-column body -----
            div { class: "shell-body",
                aside { class: "col-left",
                    if active_path.is_some() {
                        TreePanel {
                            active_encoded: active_encoded.clone(),
                        }
                        div { class: "left-divider" }
                        FilesPanel {
                            active_encoded: active_encoded.clone(),
                        }
                    } else {
                        SpaceSidebar { active_path: active_path_for_side.clone() }
                    }
                }
                main { class: "col-main",
                    {children}
                }
                aside { class: "col-right",
                    PropertiesPanel {
                        active_encoded: active_encoded.clone(),
                    }
                    div { class: "right-divider" }
                    GraphPanel {
                        active_encoded: active_encoded.clone(),
                    }
                }
            }

            // ----- Command palette overlay -----
            CommandPalette {}
        }
    }
}

#[component]
fn SpaceSwitcher(active_path: Option<String>, active_encoded: Option<String>) -> Element {
    rsx! {
        div { class: "space-switcher",
            SpaceDropdown {
                active_path,
                active_encoded,
            }
        }
    }
}

#[component]
fn TopNavActions(active_path: Option<String>, active_encoded: Option<String>) -> Element {
    rsx! {
        div { class: "topnav-actions",
            if let (Some(p), Some(enc)) = (active_path.clone(), active_encoded.clone()) {
                form {
                    class: "topnav-form",
                    action: "/api/spaces/scan",
                    method: "post",
                    input {
                        r#type: "hidden",
                        name: "space_root",
                        value: "{p}",
                    }
                    button {
                        class: "topnav-action",
                        r#type: "submit",
                        title: "Re-index the space",
                        "scan"
                    }
                }
                a {
                    class: "topnav-action",
                    href: "{crate::router::route_for_space_list(&p)}",
                    title: "Resource list",
                    "list"
                }
                a {
                    class: "topnav-action",
                    href: "{crate::router::route_for_space_graph(&enc)}",
                    title: "Graph view",
                    "graph"
                }
            } else {
                a { class: "topnav-action", href: "/", "home" }
            }
        }
    }
}