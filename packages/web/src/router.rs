//! Web router — Dioxus routes + URL encoding helpers.
//!
//! v0.3 adds a fourth route: `/space/:encoded/graph` for the
//! force-directed graph view of all resources in a space.
//!
//! The Dioxus 0.7 router is path-only; the active space travels as a
//! base64-urlsafe-encoded path segment under `/space/:encoded/...`.
//! We use base64 instead of `urlencoding::encode` because the latter
//! percent-encodes `/`, which then collides with the path separator:
//! the router only captures one path segment per `:param`, so an
//! embedded slash breaks the route. Base64-urlsafe produces a single
//! path-safe token regardless of input content.

use base64::Engine;
use dioxus::prelude::*;

use crate::pages::{DetailPage, GraphPage, HomePage, ListPage};

#[derive(Routable, Clone, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/", HomePage)]
    Home {},
    #[route("/space/:encoded/list", ListPage)]
    List { encoded: String },
    #[route("/space/:encoded/resource/:encoded_ref", DetailPage)]
    Resource { encoded: String, encoded_ref: String },
    #[route("/space/:encoded/graph", GraphPage)]
    Graph { encoded: String },
}

/// Dioxus's `Router` re-exported under a non-colliding name.
pub use dioxus_router::Router as AppRouter;

pub fn encode_space(space_root: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(space_root.as_bytes())
}

pub fn decode_space(encoded: &str) -> String {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .unwrap_or_default();
    String::from_utf8(bytes).unwrap_or_default()
}

pub fn route_for_space_list(space_root: &str) -> String {
    format!("/space/{}/list", encode_space(space_root))
}

pub fn route_for_space_graph(space_root: &str) -> String {
    format!("/space/{}/graph", encode_space(space_root))
}

pub fn route_for_space_resource(space_root: &str, encoded_ref: &str) -> String {
    format!("/space/{}/resource/{}", encode_space(space_root), encoded_ref)
}
