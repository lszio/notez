//! Web router — Dioxus routes + URL encoding helpers.

use base64::Engine;
use dioxus::prelude::*;
use crate::pages::{DetailPage, GraphPage, HomePage, ListPage, PreviewPage, SpaceHome};

/// Query string for the resource list page. PR7 introduces this so
/// `?kind=attachment&sort=mtime` deep-links land the user on the
/// correct filtered/sorted view, and so the FilesPanel kind pill
/// can navigate via the same source of truth.
///
/// `Display` + `From<&str>` are required by `dioxus_router` to make
/// `#[route("/list?:..query")]` work. Field defaults are the empty
/// string (which `ListPage` treats as "all" / "title").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub q: String,
    pub kind: String,
    pub sort: String,
}

impl std::fmt::Display for ListQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        if !self.q.is_empty() {
            parts.push(format!("q={}", url_encode(&self.q)));
        }
        if !self.kind.is_empty() {
            parts.push(format!("kind={}", url_encode(&self.kind)));
        }
        if !self.sort.is_empty() {
            parts.push(format!("sort={}", url_encode(&self.sort)));
        }
        if parts.is_empty() {
            Ok(())
        } else {
            write!(f, "?{}", parts.join("&"))
        }
    }
}

impl From<&str> for ListQuery {
    fn from(query: &str) -> Self {
        // Dioxus invokes `FromQuery::from_query` twice per match;
        // and the second call hands us the query string with a
        // leading `?` already attached. Strip it so the parser sees
        // a clean `k=v&k=v` string in both calls.
        let query = query.trim_start_matches('?');
        let mut out = ListQuery::default();
        for pair in query.split('&').filter(|s| !s.is_empty()) {
            let (k, v) = match pair.split_once('=') {
                Some((k, v)) => (k, url_decode(v)),
                None => (pair, String::new()),
            };
            match k {
                "q" => out.q = v,
                "kind" => out.kind = v,
                "sort" => out.sort = v,
                _ => {}
            }
        }
        out
    }
}

fn url_encode(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn url_decode(s: &str) -> String {
    match urlencoding::decode(s) {
        Ok(c) => c.into_owned(),
        Err(_) => s.to_string(),
    }
}

#[derive(Routable, Clone, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/", HomePage)]
    Home {},
    #[route("/space/:encoded", SpaceHome)]
    Space { encoded: String },
    #[route("/space/:encoded/list?:..query", ListPage)]
    List { encoded: String, query: ListQuery },
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
pub fn route_for_space_home(space_root: &str) -> String {
    format!("/space/{}", encode_space(space_root))
}

pub fn route_for_space_list(space_root: &str) -> String {
    format!("/space/{}/list", encode_space(space_root))
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

/// Build the list URL with the supplied query string. When `query`
/// is empty / default, the result is identical to
/// `route_for_space_list`.
pub fn route_for_space_list_with_query(space_root: &str, query: &ListQuery) -> String {
    let base = format!("/space/{}/list", encode_space(space_root));
    let qs = query.to_string();
    if qs.is_empty() {
        base
    } else {
        format!("{base}{qs}")
    }
}