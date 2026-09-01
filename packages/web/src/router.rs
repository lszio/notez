//! Web router — Dioxus routes + URL encoding helpers.
//!
//! v2 note-workspace route table:
//!
//! - `/` — configurable home dashboard
//! - `/source/:encoded` — space landing (configurable: journal/index/files)
//! - `/source/:encoded/journal` — journal (today + recent entries)
//! - `/source/:encoded/files` — all-files browser
//! - `/source/:encoded/note/:ref` — the note workbench
//! - `/source/:encoded/activity`, `/graph`, `/preview/:locator`

use base64::Engine;
use dioxus::prelude::*;
use crate::pages::{ActivityPage, GraphPage, HomePage, JournalPage, ListPage, NotePage, PreviewPage, SpaceHome};

/// Query string for the files browser. `?kind=attachment&sort=mtime`
/// deep-links land the user on the correct filtered/sorted view.
///
/// `Display` + `From<&str>` are required by `dioxus_router` to make
/// `#[route("/files?:..query")]` work. Field defaults are the empty
/// string (which `ListPage` treats as "all" / "title").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub kind: String,
    pub q: String,
    pub sort: String,
    pub mode: String,
    pub source: String,
}

impl std::fmt::Display for ListQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if !self.kind.is_empty() {
            parts.push(format!("kind={}", self.kind));
        }
        if !self.q.is_empty() {
            parts.push(format!("q={}", self.q));
        }
        if !self.sort.is_empty() {
            parts.push(format!("sort={}", self.sort));
        }
        if !self.mode.is_empty() {
            parts.push(format!("mode={}", self.mode));
        }
        if !self.source.is_empty() {
            parts.push(format!("source={}", self.source));
        }
        if parts.is_empty() {
            Ok(())
        } else {
            write!(f, "{}", parts.join("&"))
        }
    }
}

impl From<&str> for ListQuery {
    fn from(s: &str) -> Self {
        let mut out = ListQuery::default();
        for pair in s.split('&') {
            let Some((k, v)) = pair.split_once('=') else {
                continue;
            };
            match k {
                "kind" => out.kind = v.to_string(),
                "q" => out.q = v.to_string(),
                "sort" => out.sort = v.to_string(),
                "mode" => out.mode = v.to_string(),
                "source" => out.source = v.to_string(),
                _ => {}
            }
        }
        out
    }
}

/// Query string for the note page. Carries the result of a save
/// attempt so the read view can surface a structured error banner
/// (StaleRevision / ReadOnly / NotFound / Unsupported) without needing
/// a client-side event bridge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetailQuery {
    pub edit_err: String,
    pub edit_msg: String,
    pub edited: String,
}

impl std::fmt::Display for DetailQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if !self.edit_err.is_empty() {
            parts.push(format!("edit_err={}", url_encode(&self.edit_err)));
        }
        if !self.edit_msg.is_empty() {
            parts.push(format!("edit_msg={}", url_encode(&self.edit_msg)));
        }
        if !self.edited.is_empty() {
            parts.push(format!("edited={}", self.edited));
        }
        if parts.is_empty() {
            Ok(())
        } else {
            write!(f, "{}", parts.join("&"))
        }
    }
}

impl From<&str> for DetailQuery {
    fn from(s: &str) -> Self {
        let mut out = DetailQuery::default();
        for pair in s.split('&') {
            let Some((k, v)) = pair.split_once('=') else {
                continue;
            };
            match k {
                "edit_err" => out.edit_err = url_decode(v),
                "edit_msg" => out.edit_msg = url_decode(v),
                "edited" => out.edited = v.to_string(),
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
    urlencoding::decode(s).map(|c| c.into_owned()).unwrap_or_else(|_| s.to_string())
}

#[derive(Routable, Clone, Debug, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[route("/", HomePage)]
    Home {},
    #[route("/source/:encoded", SpaceHome)]
    Space { encoded: String },
    #[route("/source/:encoded/journal", JournalPage)]
    Journal { encoded: String },
    #[route("/source/:encoded/files?:..query", ListPage)]
    Files { encoded: String, query: ListQuery },
    #[route("/source/:encoded/note/:encoded_ref?:..query", NotePage)]
    Note { encoded: String, encoded_ref: String, query: DetailQuery },
    #[route("/source/:encoded/activity", ActivityPage)]
    Activity { encoded: String },
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
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_default()
}

/// Encode a file locator (path with `/`) into a single URL-safe
/// path segment. Used by the preview route where `:encoded_locator`
/// is a single segment that must not contain raw `/`.
pub fn encode_locator(locator: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(locator.as_bytes())
}

pub fn decode_locator(encoded: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_default()
}

pub fn route_for_space_home(source_root: &str) -> String {
    format!("/source/{}", encode_space(source_root))
}

pub fn route_for_space_journal(source_root: &str) -> String {
    format!("/source/{}/journal", encode_space(source_root))
}

pub fn route_for_space_files(source_root: &str) -> String {
    format!("/source/{}/files", encode_space(source_root))
}

pub fn route_for_space_graph(source_root: &str) -> String {
    format!("/source/{}/graph", encode_space(source_root))
}

pub fn route_for_space_activity(source_root: &str) -> String {
    format!("/source/{}/activity", encode_space(source_root))
}

pub fn route_for_space_note(source_root: &str, ref_str: &str) -> String {
    format!(
        "/source/{}/note/{}",
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

/// Build the files URL with the supplied query string. When `query`
/// is empty / default, the result is identical to
/// `route_for_space_files`.
pub fn route_for_space_files_with_query(source_root: &str, query: &ListQuery) -> String {
    let base = route_for_space_files(source_root);
    let qs = query.to_string();
    if qs.is_empty() {
        base
    } else {
        format!("{base}?{qs}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_round_trip() {
        let enc = encode_space("/home/user/notes");
        assert_eq!(decode_space(&enc), "/home/user/notes");
    }

    #[test]
    fn locator_round_trip_keeps_slashes_encoded() {
        let enc = encode_locator("journal/2026-09-01.md");
        assert!(!enc.contains('/'));
        assert_eq!(decode_locator(&enc), "journal/2026-09-01.md");
    }

    #[test]
    fn route_helpers_produce_expected_paths() {
        assert_eq!(route_for_space_home("/n"), "/source/L24");
        assert_eq!(route_for_space_journal("/n"), "/source/L24/journal");
        assert_eq!(route_for_space_files("/n"), "/source/L24/files");
        assert!(route_for_space_note("/n", "doc:1").starts_with("/source/L24/note/"));
        assert!(route_for_space_preview("/n", "a/b.md").starts_with("/source/L24/preview/"));
    }

    #[test]
    fn list_query_round_trips() {
        let q = ListQuery { kind: "attachment".into(), q: String::new(), sort: "mtime".into(), mode: String::new(), source: String::new() };
        let s = q.to_string();
        assert_eq!(ListQuery::from(s.as_str()), q);
    }

    #[test]
    fn detail_query_escapes_message() {
        let q = DetailQuery { edit_err: "stale_revision".into(), edit_msg: "a&b=c".into(), edited: String::new() };
        let s = q.to_string();
        assert!(!s.contains("a&b=c"));
        assert_eq!(ListQuery::from("").to_string(), "");
        assert_eq!(DetailQuery::from(s.as_str()), q);
    }
}
