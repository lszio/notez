//! `BacklinksPanel` — linked mentions for the note page right rail.
//!
//! Incoming ("linked mentions") and outgoing links come from the
//! resolved-relations projection (see `server::list_links_impl`). Rows
//! whose backing resource was deleted render dimmed with kind
//! `missing` and no link.

use dioxus::prelude::*;

use crate::router::route_for_space_note;
use crate::server::{list_links, LinkRow};
use crate::space_ctx::SpaceState;

#[component]
pub fn BacklinksPanel(active_encoded: Option<String>, active_ref: String) -> Element {
    let space = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space().map(|s| s.path.clone());
    let ref_str = active_ref.clone();

    let path_for_fetch = active_path.clone();
    let ref_for_fetch = ref_str.clone();
    let links_resource = use_server_future(move || {
        let p = path_for_fetch.clone();
        let r = ref_for_fetch.clone();
        async move {
            match (p, r.is_empty()) {
                (Some(p), false) => list_links(p, r).await.ok(),
                _ => None,
            }
        }
    })?;

    let links = links_resource.cloned().unwrap_or_default();
    let (incoming, outgoing) = match links {
        Some(l) => (l.incoming, l.outgoing),
        None => (Vec::new(), Vec::new()),
    };
    let decoded_space = space().map(|s| s.path.clone()).unwrap_or_default();

    rsx! {
        section { class: "rail-card", "data-rail": "backlinks",
            h3 { class: "rail-card-title", "Linked mentions" }
            if incoming.is_empty() {
                p { class: "rail-empty", "no notes link here yet" }
            } else {
                ul { class: "link-list",
                    for row in incoming.iter() {
                        li { class: "link-row", LinkRowView { row: row.clone(), space: decoded_space.clone() } }
                    }
                }
            }
            if !outgoing.is_empty() {
                h3 { class: "rail-card-title rail-card-title-sub", "Outgoing links" }
                ul { class: "link-list",
                    for row in outgoing.iter() {
                        li { class: "link-row", LinkRowView { row: row.clone(), space: decoded_space.clone() } }
                    }
                }
            }
        }
    }
}

#[component]
fn LinkRowView(row: LinkRow, space: String) -> Element {
    if row.kind == "missing" {
        rsx! {
            span { class: "link-missing", title: "{row.ref_str}", "{row.title}" }
        }
    } else {
        rsx! {
            a { class: "link-row-link", href: "{route_for_space_note(&space, &row.ref_str)}",
                title: "{row.ref_str}",
                "{row.title}"
            }
        }
    }
}
