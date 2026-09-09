//! URL helpers for the minimal workspace UI.
//!
//! Two kinds of identifiers travel in the URL:
//!
//! * the **space** — an absolute filesystem path, carried as
//!   base64url (no padding) so `/` survives a path segment;
//! * the **locator** — a space-relative POSIX path, carried verbatim
//!   in a catch-all segment with each component percent-encoded, so
//!   CJK names and spaces work and the URL stays readable
//!   (`/s/<space>/projects/2026-09-09.md`).

use base64::Engine;

const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// base64url(space root) — a single opaque path segment.
pub fn encode_space(root: &str) -> String {
    B64.encode(root.as_bytes())
}

/// Decode a space segment. Invalid input yields the empty string; the
/// caller reports it as a not-found space.
pub fn decode_space(encoded: &str) -> String {
    B64.decode(encoded)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

/// Percent-encode every `/`-separated component of a locator.
pub fn encode_locator(locator: &str) -> String {
    locator
        .split('/')
        .map(|seg| urlencoding::encode(seg).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Percent-decode a locator captured from a catch-all path segment.
pub fn decode_locator(raw: &str) -> String {
    raw.split('/')
        .map(|seg| {
            urlencoding::decode(seg)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| seg.to_string())
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub fn space_url(encoded: &str) -> String {
    format!("/s/{encoded}")
}

pub fn view_url(encoded: &str, locator: &str) -> String {
    format!("/s/{encoded}/{}", encode_locator(locator))
}

pub fn edit_url(encoded: &str, locator: &str) -> String {
    format!("/s/{encoded}/{}?edit=1", encode_locator(locator))
}

pub fn raw_url(encoded: &str, locator: &str) -> String {
    format!("/raw/{encoded}/{}", encode_locator(locator))
}

/// Form target that creates the note.
pub fn new_action_url(encoded: &str) -> String {
    format!("/new/{encoded}")
}

/// The new-note page.
pub fn new_url(encoded: &str) -> String {
    format!("/s/{encoded}/new")
}

pub fn save_url(encoded: &str) -> String {
    format!("/save/{encoded}")
}

pub fn scan_url(encoded: &str) -> String {
    format!("/scan/{encoded}")
}

pub fn css_url() -> String {
    // Cache-bust on every release: the stylesheet is embedded in the
    // binary, so the URL only changes when the file changes.
    format!("/app.css?v={}", crate::data::ASSET_VERSION)
}

/// Split a locator into (directory, file name).
pub fn split_locator(locator: &str) -> (&str, &str) {
    match locator.rfind('/') {
        Some(i) => (&locator[..i], &locator[i + 1..]),
        None => ("", locator),
    }
}

/// Resolve a relative link target against the directory of the
/// document that contains it. Absolute targets and URLs are returned
/// unchanged so callers can decide what to do with them.
pub fn resolve_relative(dir: &str, target: &str) -> Option<String> {
    if target.is_empty() || target.starts_with('#') || target.starts_with('/') {
        return None;
    }
    if let Some(colon) = target.find(':') {
        let scheme = &target[..colon];
        let looks_like_scheme = scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.');
        if looks_like_scheme {
            return None; // http:, mailto:, file:, id:, data:, …
        }
    }
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_round_trips_through_base64url() {
        let enc = encode_space("/home/lszio/Notes");
        assert_eq!(decode_space(&enc), "/home/lszio/Notes");
    }

    #[test]
    fn locator_encodes_each_segment_but_keeps_slashes() {
        assert_eq!(encode_locator("a b/c+d.md"), "a%20b/c%2Bd.md");
        assert_eq!(decode_locator("a%20b/c%2Bd.md"), "a b/c+d.md");
    }

    #[test]
    fn locator_round_trips_cjk() {
        let locator = "projects/准备材料/技术总结.org";
        assert_eq!(decode_locator(&encode_locator(locator)), locator);
    }

    #[test]
    fn relative_targets_resolve_against_directory() {
        assert_eq!(
            resolve_relative("projects", "./silverbullet.org").as_deref(),
            Some("projects/silverbullet.org")
        );
        assert_eq!(
            resolve_relative("projects/2609", "../assets/x.png").as_deref(),
            Some("projects/assets/x.png")
        );
        assert_eq!(resolve_relative("", "README.org").as_deref(), Some("README.org"));
    }

    #[test]
    fn absolute_and_scheme_targets_are_left_alone() {
        assert_eq!(resolve_relative("a", "https://example.com"), None);
        assert_eq!(resolve_relative("a", "mailto:x@y.z"), None);
        assert_eq!(resolve_relative("a", "/already/absolute"), None);
        assert_eq!(resolve_relative("a", "#anchor"), None);
        assert_eq!(resolve_relative("a", "file:x.org"), None);
    }
}
