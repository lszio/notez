//! URL routes for the workspace app.
//!
//! The write endpoints and `/` live in [`crate::data`] as plain axum
//! routes; everything the user *reads* is a Dioxus page rendered by
//! [`crate::app`].

use dioxus::prelude::*;

use crate::app::pages::{DocPage, NewPage, SpaceIndexPage};

/// Query string carried by a document URL.
///
/// `?edit=1` opens the editor; `saved`/`err`/`err_msg`/`revision` carry
/// the result of a save attempt back from the POST handler.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocQuery {
    pub edit: Option<String>,
    pub saved: Option<String>,
    /// One-shot token for a failed save whose text must be restored.
    pub restore: Option<String>,
}

impl std::fmt::Display for DocQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        for (key, value) in [
            ("edit", &self.edit),
            ("saved", &self.saved),
            ("restore", &self.restore),
        ] {
            if let Some(v) = value {
                if !v.is_empty() {
                    parts.push(format!("{key}={}", urlencoding::encode(v)));
                }
            }
        }
        if parts.is_empty() {
            Ok(())
        } else {
            write!(f, "?{}", parts.join("&"))
        }
    }
}

impl From<&str> for DocQuery {
    fn from(raw: &str) -> Self {
        let mut out = DocQuery::default();
        for pair in raw.trim_start_matches('?').split('&') {
            let Some((k, v)) = pair.split_once('=') else {
                continue;
            };
            let value = urlencoding::decode(v)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| v.to_string());
            match k {
                "edit" => out.edit = Some(value),
                "saved" => out.saved = Some(value),
                "restore" => out.restore = Some(value),
                _ => {}
            }
        }
        out
    }
}

#[derive(Routable, Clone, PartialEq, Debug)]
#[rustfmt::skip]
pub enum Route {
    #[route("/s/:encoded/new")]
    NewPage { encoded: String },
    #[route("/s/:encoded")]
    SpaceIndexPage { encoded: String },
    #[route("/s/:encoded/:..locator?:..query")]
    DocPage { encoded: String, locator: Vec<String>, query: DocQuery },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_round_trips() {
        let q = DocQuery {
            edit: Some("1".into()),
            saved: Some("abc def".into()),
            restore: None,
        };
        let rendered = q.to_string();
        assert!(rendered.starts_with("?edit=1&saved=abc%20def"), "{rendered}");
        let back = DocQuery::from(rendered.as_str());
        assert_eq!(back, q);
    }

    #[test]
    fn empty_query_renders_nothing() {
        assert_eq!(DocQuery::default().to_string(), "");
    }
}
