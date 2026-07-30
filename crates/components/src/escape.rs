//! HTML escaping helpers shared across all components.
//!
//! `escape_html` wraps `html_escape::encode_safe` — a comprehensive
//! (XSS-resistant) escaper that handles `<`, `>`, `&`, `"`, `'`, and `/`.
//! Every string component that originates in a user-controlled file must
//! flow through this helper before being emitted into HTML; the
//! `dangerous_inner_html` attribute is reserved for trusted outputs from
//! our own previewers (Markdown → HTML, Org → HTML).

/// Escape the special HTML characters in `s`. The returned `String` is
/// safe to drop into a `dangerous_inner_html` attribute or a `{}` text
/// node in `rsx!`.
pub fn escape_html(s: &str) -> String {
    html_escape::encode_safe(s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_angle_brackets() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn escapes_ampersand_and_quote() {
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("\"x\""), "&quot;x&quot;");
    }

    #[test]
    fn passes_through_plain_ascii() {
        assert_eq!(escape_html("hello world"), "hello world");
    }
}