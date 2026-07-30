//! Shared HTML escape helpers used by SSR routes.
//!
//! `escape` is the body-context escape — it produces text safe to drop
//! inside an element (`<p>...</p>`).
//!
//! `escape_attr` is the attribute-context escape — it also neutralises the
//! quote characters (`"` and `'`) that would otherwise break out of a
//! `key="..."` or `key='...'` attribute. Use this for ANY user-controlled
//! value interpolated into an attribute value.

/// Escape a string for use as element body text.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    push_escaped(&mut out, s, false);
    out
}

/// Escape a string for use as an attribute value (inside `key="…"`).
///
/// Differs from [`escape`] in that quote characters are also escaped, so a
/// payload like `" onload="alert(1)` cannot break out of the enclosing
/// attribute.
pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    push_escaped(&mut out, s, true);
    out
}

fn push_escaped(out: &mut String, s: &str, attr: bool) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\'' if attr => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_body_neutralises_tags() {
        assert_eq!(
            escape("<script>alert(1)</script>"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn escape_attr_neutralises_quotes() {
        let out = escape_attr(r#""" onload="x"#);
        assert!(out.contains("&quot;"), "expected &quot; in {out}");
        assert!(!out.contains('"'), "raw quote must not survive: {out}");
    }

    #[test]
    fn escape_attr_neutralises_single_quotes() {
        let out = escape_attr("' onclick='x");
        assert!(out.contains("&#39;"));
    }

    #[test]
    fn escape_attr_keeps_plain_text() {
        assert_eq!(escape_attr("hello world"), "hello world");
    }

    #[test]
    fn escape_attr_escapes_ampersand() {
        assert_eq!(escape_attr("a & b"), "a &amp; b");
    }
}
