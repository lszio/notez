//! SpacePicker — the trigger + popover for switching spaces.
//!
//! Closed by default; clicking the trigger toggles a popover below
//! it. The popover lists every registered space (from the
//! `list_registered_spaces` server fn) as a clickable option and
//! offers a text input for an arbitrary absolute path. Manual
//! entries are validated by `resolve_space_path` before navigating.
//!
//! Visual style: an old filing-cabinet drawer. The trigger reads
//! "[space name] ▾"; the panel is a small index-card with a thin
//! border and a 4px drop-shadow drawn as a solid offset block
//! (no real `box-shadow` blur — that would feel too modern).

use dioxus::prelude::*;

use crate::router::route_for_space_list;
use crate::server::{list_registered_spaces, resolve_space_path, RegisteredSpaceDto};

#[component]
pub fn SpacePicker() -> Element {
    let navigator = use_navigator();
    let mut registered = use_signal(Vec::<RegisteredSpaceDto>::new);
    let mut open = use_signal(|| false);
    let mut manual = use_signal(String::new);
    let mut pending = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    // Fetch the registered list on first open, not at mount — that
    // way the home page doesn't pay the cost when there's no
    // popover to populate. The fetch is cached by the server
    // runtime; re-opening won't re-hit the server.
    use_effect(move || {
        let is_open = open();
        if is_open {
            spawn(async move {
                if let Ok(list) = list_registered_spaces().await {
                    registered.set(list);
                }
            });
        }
    });

    let trigger_label = if registered().is_empty() {
        "select space…".to_string()
    } else {
        format!("{} spaces ▾", registered().len())
    };

    rsx! {
        button {
            class: "picker-trigger",
            onclick: move |_| open.set(!open()),
            "{trigger_label}"
        }
        if open() {
            // Click-outside to close: a transparent overlay that
            // absorbs clicks anywhere outside the panel.
            div {
                class: "picker-overlay",
                style: "position: fixed; inset: 0; z-index: 40;",
                onclick: move |_| open.set(false),
            }
            div { class: "picker-panel",
                // Stop propagation so clicks inside the panel don't
                // reach the overlay.
                onclick: move |e| e.stop_propagation(),

                p { class: "picker-label", "Spaces" }
                if registered().is_empty() {
                    p { class: "picker-empty",
                        "No spaces yet — type a path below, or run `notez space register <name> --path <dir>` from a terminal to add one permanently."
                    }
                } else {
                    for r in registered().iter() {
                        button {
                            class: "picker-option",
                            onclick: {
                                let path = r.path.clone();
                                move |_| {
                                    navigator.push(route_for_space_list(&path));
                                    open.set(false);
                                }
                            },
                            span { class: "name", "{r.name}" }
                            if r.source == "discovered" {
                                span { class: "picker-tag", "found" }
                            }
                            span { class: "path", "{r.path}" }
                        }
                    }
                }

                hr { class: "rule" }
                p { class: "picker-label", "Or open by path" }
                input {
                    class: "picker-input",
                    r#type: "text",
                    placeholder: "/absolute/path/to/space",
                    value: "{manual}",
                    oninput: move |e| manual.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            // Trigger switch via the same path the
                            // button uses; close popover on success.
                            let p = manual().trim().to_string();
                            if p.is_empty() { return; }
                            let nav = navigator.clone();
                            pending.set(true);
                            error.set(None);
                            spawn(async move {
                                match resolve_space_path(p.clone()).await {
                                    Ok(_) => {
                                        nav.push(route_for_space_list(&p));
                                        open.set(false);
                                        pending.set(false);
                                    }
                                    Err(e) => {
                                        error.set(Some(e.to_string()));
                                        pending.set(false);
                                    }
                                }
                            });
                        }
                    },
                }
                button {
                    class: "picker-go",
                    disabled: pending() || manual().trim().is_empty(),
                    onclick: move |_| {
                        let p = manual().trim().to_string();
                        if p.is_empty() { return; }
                        let nav = navigator.clone();
                        pending.set(true);
                        error.set(None);
                        spawn(async move {
                            match resolve_space_path(p.clone()).await {
                                Ok(_) => {
                                    nav.push(route_for_space_list(&p));
                                    open.set(false);
                                    pending.set(false);
                                }
                                Err(e) => {
                                    error.set(Some(e.to_string()));
                                    pending.set(false);
                                }
                            }
                        });
                    },
                    if pending() { "…" } else { "open" }
                }
                if let Some(msg) = error() {
                    p { class: "picker-error", "{msg}" }
                }
            }
        }
    }
}
