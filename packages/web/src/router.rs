//! Web router — Dioxus routes + URL encoding helpers.

use base64::Engine;
use dioxus::prelude::*;

use crate::pages::{DetailPage, GraphPage, HomePage, ListPage, PreviewPage, SpaceHome};

#[derive(Routable, Clone, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/", HomePage)]
    Home {},
    #[route("/space/:encoded", SpaceHome)]
    Space { encoded: String },
    #[route("/space/:encoded/list", ListPage)]
    List { encoded: String },
    #[route("/space/:encoded/resource/:encoded_ref", DetailPage)]
    Resource { encoded: String, encoded_ref: String },
    #[route("/space/:encoded/graph", GraphPage)]
    Graph { encoded: String },
    #[route("/space/:encoded/preview/:encoded_locator", PreviewPage)]
    Preview { encoded: String, encoded_locator: String },
}

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

/// Encode a file locator (path with `/`) into a single URL-safe
/// path segment. Used by the preview route where `:encoded_locator`
/// is a single segment that must not contain raw `/`.
pub fn encode_locator(locator: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(locator.as_bytes())
}

pub fn decode_locator(encoded: &str) -> String {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .unwrap_or_default();
    String::from_utf8(bytes).unwrap_or_default()
}

pub fn route_for_space_list(space_root: &str) -> String {
    format!("/space/{}/list", encode_space(space_root))
}

pub fn route_for_space_home(space_root: &str) -> String {
    format!("/space/{}", encode_space(space_root))
}

pub fn route_for_space_graph(space_root: &str) -> String {
    format!("/space/{}/graph", encode_space(space_root))
}

pub fn route_for_space_resource(space_root: &str, ref_str: &str) -> String {
    format!(
        "/space/{}/resource/{}",
        encode_space(space_root),
        encode_space(ref_str)
    )
}

pub fn route_for_space_preview(space_root: &str, locator: &str) -> String {
    format!(
        "/space/{}/preview/{}",
        encode_space(space_root),
        encode_locator(locator)
    )
}