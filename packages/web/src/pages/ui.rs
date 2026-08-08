//! Small UI primitives shared across pages.
//!
//! These are intentionally tiny: each one is a single HTML element
//! with a few classes from the stylesheet in `index.html`. They
//! exist so page components don't reach for raw class strings and
//! so the visual language stays consistent.

use dioxus::prelude::*;

/// A two-or-three character badge indicating a resource kind.
///
/// The visible label is a short tag (`doc`, `§ hd`, `attach`,
/// `block`); the colour comes from the matching `.kind-*` class.
/// The `id` is the canonical kind string returned by the server
/// (`document`, `heading`, `attachment`, `block`).
#[component]
pub fn KindIcon(kind: String) -> Element {
    let (label, class_suffix) = match kind.as_str() {
        "document" => ("doc", "kind-document"),
        "heading" => ("§ hd", "kind-heading"),
        "attachment" => ("attach", "kind-attachment"),
        "block" => ("block", "kind-block"),
        other => (other, "kind-block"),
    };
    rsx! {
        span { class: "kind {class_suffix}", "{label}" }
    }
}

/// Breadcrumb trail shown above a page heading.
///
/// `segments` is rendered left-to-right; the last segment is rendered
/// as plain text (the "here" position) while the rest are links.
#[component]
pub fn Breadcrumb(segments: Vec<BreadcrumbSegment>) -> Element {
    rsx! {
        nav { class: "crumb",
            for (i, seg) in segments.iter().enumerate() {
                if i > 0 {
                    span { class: "sep", "·" }
                }
                if let Some(href) = seg.href.as_ref() {
                    a { href: "{href}", "{seg.label}" }
                } else {
                    span { class: "here", "{seg.label}" }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    pub label: String,
    /// `None` means this is the current page (no link).
    pub href: Option<String>,
}

impl BreadcrumbSegment {
    pub fn here(label: impl Into<String>) -> Self {
        Self { label: label.into(), href: None }
    }
    pub fn link(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self { label: label.into(), href: Some(href.into()) }
    }
}
