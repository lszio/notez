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
    pub source: String,
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
        if !self.source.is_empty() {
            parts.push(format!("source={}", url_encode(&self.source)));
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
                "source" => out.source = v,
                _ => {}
            }
        }
        out
    }
}

/// Query string for the resource detail page. Currently carries the
/// result of a save attempt so the read view can surface a structured
/// error banner (StaleRevision / ReadOnly / NotFound / Unsupported)
/// without needing a client-side event bridge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailQuery {
    pub edit_err: String,
    pub edit_msg: String,
    /// Set to "1" after a successful save, so the read view can show
    /// a confirmation banner instead of silently re-rendering.
    pub edited: String,
}

impl std::fmt::Display for DetailQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        if !self.edit_err.is_empty() {
            parts.push(format!("edit_err={}", url_encode(&self.edit_err)));
        }
        if !self.edit_msg.is_empty() {
            parts.push(format!("edit_msg={}", url_encode(&self.edit_msg)));
        }
        if self.edited == "1" {
            parts.push("edited=1".to_string());
        }
        if parts.is_empty() {
            Ok(())
        } else {
            write!(f, "?{}", parts.join("&"))
        }
    }
}

impl From<&str> for DetailQuery {
    fn from(query: &str) -> Self {
        let query = query.trim_start_matches('?');
        let mut out = DetailQuery::default();
        for pair in query.split('&').filter(|s| !s.is_empty()) {
            let (k, v) = match pair.split_once('=') {
                Some((k, v)) => (k, url_decode(v)),
                None => (pair, String::new()),
            };
            match k {
                "edit_err" => out.edit_err = v,
                "edit_msg" => out.edit_msg = v,
                "edited" => out.edited = v,
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
    #[route("/source/:encoded", SpaceHome)]
    Space { encoded: String },
    #[route("/source/:encoded/list?:..query", ListPage)]
    List { encoded: String, query: ListQuery },
    #[route("/source/:encoded/resource/:encoded_ref?:..query", DetailPage)]
    Resource { encoded: String, encoded_ref: String, query: DetailQuery },
    #[route("/source/:encoded/graph", GraphPage)]
    Graph { encoded: String },
    #[route("/source/:encoded/preview/:encoded_locator", PreviewPage)]
    Preview { encoded: String, encoded_locator: String },
}

pub use dioxus_router::Router as AppRouter;

pub fn encode_space(source_root: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(source_root.as_bytes())
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
pub fn route_for_space_home(source_root: &str) -> String {
    format!("/source/{}", encode_space(source_root))
}

pub fn route_for_space_list(source_root: &str) -> String {
    format!("/source/{}/list", encode_space(source_root))
}

pub fn route_for_space_graph(source_root: &str) -> String {
    format!("/source/{}/graph", encode_space(source_root))
}

pub fn route_for_space_resource(source_root: &str, ref_str: &str) -> String {
    format!(
        "/source/{}/resource/{}",
        encode_space(source_root),
        encode_space(ref_str)
    )
}

pub fn route_for_space_preview(source_root: &str, locator: &str) -> String {
    format!(
        "/source/{}/preview/{}",
        encode_space(source_root),
        encode_locator(locator)
    )
}

/// Build the list URL with the supplied query string. When `query`
/// is empty / default, the result is identical to
/// `route_for_space_list`.
pub fn route_for_space_list_with_query(source_root: &str, query: &ListQuery) -> String {
    let base = format!("/source/{}/list", encode_space(source_root));
    let qs = query.to_string();
    if qs.is_empty() {
        base
    } else {
        format!("{base}{qs}")
    }
}