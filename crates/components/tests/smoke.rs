//! Smoke tests for the `components` crate.
//!
//! Most rendering is exercised end-to-end by the `crates/web` smoke tests in
//! Phase D (which spin up a real axum + dioxus SSR stack). Here we lock
//! down the pure helpers that have a stable output regardless of the
//! Dioxus runtime — the HTML-escape helper and the data-carrying DTOs.

use components::escape::escape_html;

#[test]
fn escape_html_escapes_special_chars() {
    assert_eq!(escape_html("<script>"), "&lt;script&gt;");
    assert_eq!(escape_html("a & b"), "a &amp; b");
    assert_eq!(escape_html("\"quoted\""), "&quot;quoted&quot;");
    assert_eq!(escape_html("'apos'"), "&#x27;apos&#x27;");
    // Empty / plain ASCII pass through unchanged.
    assert_eq!(escape_html(""), "");
    assert_eq!(escape_html("hello world"), "hello world");
    // Multi-byte UTF-8 (emoji) is preserved — html-escape is byte-safe.
    assert_eq!(escape_html("🦀"), "🦀");
}

#[test]
fn space_summary_serialization_roundtrip() {
    let summary = components::SpaceSummary {
        name: "primary".to_string(),
        root: std::path::PathBuf::from("/srv/spaces/primary"),
    };
    let json = serde_json::to_string(&summary).expect("serialize");
    let back: components::SpaceSummary = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, summary);
}