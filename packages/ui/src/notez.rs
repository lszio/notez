//! Notez design-system components.
//!
//! Every component styles itself exclusively through the design
//! tokens defined in `packages/web/public/index.html`
//! (`--paper*`, `--ink*`, `--accent*`, `--sp-*`, `--r-*`, …) so
//! themes (light/dark) apply without touching component code.
//!
//! Shared primitives here must stay platform-neutral: they render
//! plain HTML elements, take optional accessibility props
//! (`aria_label`, `title`, `name`, …) and never reach for page
//! classes. Web/Desktop/Mobile all compose these.

use dioxus::prelude::*;

/// Solid primary button. `accent` flips to the accent palette.
///
/// `kind` maps to the button `type` attribute (`button` default,
/// pass `"submit"` inside a form). `aria_label` and `title` are
/// rendered when non-empty so icon-only buttons stay announced.
#[component]
pub fn NzButton(
    #[props(default = "button".to_string())] kind: String,
    accent: bool,
    #[props(default = "".to_string())] aria_label: String,
    #[props(default = "".to_string())] title: String,
    children: Element,
) -> Element {
    let border = if accent { "var(--accent)" } else { "var(--ink)" };
    let color = if accent { "var(--accent)" } else { "var(--ink)" };
    rsx! {
        button {
            r#type: "{kind}",
            "aria-label": aria_label,
            title: title,
            style: "font-family: var(--mono); font-size: 0.8rem; \
                    color: var(--paper); background: {color}; \
                    border: 1px solid {border}; border-radius: var(--r-sm); \
                    padding: var(--sp-1) var(--sp-3); cursor: pointer;",
            {children}
        }
    }
}

/// Outlined secondary button.
#[component]
pub fn NzButtonGhost(
    #[props(default = "button".to_string())] kind: String,
    #[props(default = "".to_string())] aria_label: String,
    #[props(default = "".to_string())] title: String,
    children: Element,
) -> Element {
    rsx! {
        button {
            r#type: "{kind}",
            "aria-label": aria_label,
            title: title,
            style: "font-family: var(--mono); font-size: 0.8rem; \
                    color: var(--ink-2); background: transparent; \
                    border: 1px solid var(--ink-rule); border-radius: var(--r-sm); \
                    padding: var(--sp-1) var(--sp-3); cursor: pointer;",
            {children}
        }
    }
}

/// Underlined token-style text input.
///
/// `name` and `input_type` make the input usable inside native
/// `<form method="post">` SSR forms (the page-level progressive
/// enhancement reads `input[name=…]` from the DOM).
#[component]
pub fn NzInput(
    placeholder: String,
    #[props(default = "".to_string())] value: String,
    #[props(default = "".to_string())] name: String,
    #[props(default = "text".to_string())] input_type: String,
    #[props(default = "".to_string())] aria_label: String,
    oninput: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        input {
            r#type: "{input_type}",
            name: name,
            placeholder: "{placeholder}",
            value: "{value}",
            "aria-label": aria_label,
            oninput: move |e| oninput.call(e),
            style: "font-family: var(--mono); font-size: 0.85rem; \
                    color: var(--ink); background: transparent; \
                    border: 0; border-bottom: 1px solid var(--ink-rule); \
                    padding: var(--sp-1) var(--sp-2); flex: 1;",
        }
    }
}

/// Small uppercase label chip. Status is carried by the visible text
/// (never colour alone); `tone` picks the token.
#[component]
pub fn NzBadge(text: String, tone: String) -> Element {
    let color = match tone.as_str() {
        "ok" => "var(--ok)",
        "warn" => "var(--warn)",
        "err" => "var(--err)",
        "info" => "var(--info)",
        _ => "var(--ink-2)",
    };
    let title = text.clone();
    rsx! {
        span {
            title: "{title}",
            style: "font-family: var(--mono); font-size: 0.62rem; \
                    color: {color}; border: 1px solid {color}; \
                    border-radius: var(--r-sm); \
                    padding: 0 0.35rem; letter-spacing: 0.08em; \
                    text-transform: uppercase; white-space: nowrap;",
            {text}
        }
    }
}

/// Paper card with hairline rule and soft elevation.
#[component]
pub fn NzCard(children: Element, padded: bool) -> Element {
    let pad = if padded { "var(--sp-4)" } else { "0" };
    rsx! {
        div {
            style: "background: var(--paper-2); border: 1px solid var(--ink-rule); \
                    border-radius: var(--r-md); box-shadow: var(--shadow-1); \
                    padding: {pad};",
            {children}
        }
    }
}
