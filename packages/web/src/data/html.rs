//! HTML escaping and URL rewriting for rendered document bodies.
//!
//! The previewers emit plain HTML with the document's own relative
//! `href`/`src` values. This module turns those into notez URLs (view
//! for pages, raw bytes for assets) and provides the escaping helper
//! used by the components.

use crate::data::urls;

/// Escape for HTML text and attribute contexts.
pub fn esc(s: &str) -> String {
    html_escape::encode_safe(s).into_owned()
}

/// Rewrite relative `href`/`src` values produced by the markdown/org
/// renderers into notez URLs (view for pages, raw for assets).
pub fn rewrite_local_urls(html: &str, encoded_space: &str, doc_dir: &str) -> String {
    let mut out = String::with_capacity(html.len() + 64);
    let mut i = 0usize;
    while i < html.len() {
        let href = html[i..].find("href=\"").map(|p| (p + i, 6));
        let src = html[i..].find("src=\"").map(|p| (p + i, 5));
        let next = match (href, src) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some((pos, len)) = next else {
            out.push_str(&html[i..]);
            break;
        };
        let val_start = pos + len;
        let Some(rel_end) = html[val_start..].find('"') else {
            out.push_str(&html[i..]);
            break;
        };
        let val_end = val_start + rel_end;
        out.push_str(&html[i..val_start]);
        out.push_str(&rewrite_one(
            &html[val_start..val_end],
            len == 5,
            encoded_space,
            doc_dir,
        ));
        i = val_end;
    }
    out
}

fn rewrite_one(value: &str, is_asset: bool, encoded_space: &str, doc_dir: &str) -> String {
    // Absolute URLs (including HTML-escaped leading slashes produced by
    // attribute escaping) are already complete — never re-resolve them.
    if value.starts_with('/') || value.starts_with("&#x2F;") || value.starts_with("&#47;") {
        return value.to_string();
    }
    let (target, forced_asset, target_dir) = if let Some(rest) = value.strip_prefix("attachment:") {
        // Org attachment targets are space-relative, not document-relative.
        (rest, true, "")
    } else if let Some(rest) = value.strip_prefix("file:") {
        (rest, is_asset, doc_dir)
    } else {
        (value, is_asset, doc_dir)
    };
    if target.starts_with("id:") || target.starts_with('#') || target.starts_with("mailto:") {
        return value.to_string();
    }
    let Some(locator) = urls::resolve_relative(target_dir, target) else {
        return value.to_string();
    };
    if forced_asset {
        urls::raw_url(encoded_space, &locator)
    } else {
        urls::view_url(encoded_space, &locator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_image_src_becomes_raw_url() {
        let html = r#"<p><img src="assets/x.png" alt="x"></p>"#;
        let out = rewrite_local_urls(html, "SPACE", "projects");
        assert!(out.contains(r#"src="/raw/SPACE/projects/assets/x.png""#), "{out}");
    }

    #[test]
    fn relative_page_link_becomes_view_url() {
        let html = r#"<a href="./other.org">other</a>"#;
        let out = rewrite_local_urls(html, "S", "projects");
        assert!(out.contains(r#"href="/s/S/projects/other.org""#), "{out}");
    }

    #[test]
    fn external_and_anchor_links_are_untouched() {
        let html = r##"<a href="https://example.com">a</a><a href="#top">b</a>"##;
        assert_eq!(rewrite_local_urls(html, "S", "a"), html);
    }

    #[test]
    fn already_absolute_and_escaped_urls_are_never_double_resolved() {
        let html = r#"<img src="/raw/S/a/b.png"><img src="&#x2F;raw&#x2F;S&#x2F;a&#x2F;b.png">"#;
        assert_eq!(rewrite_local_urls(html, "S", "a"), html);
    }

    #[test]
    fn attachment_scheme_targets_resolve_to_raw() {
        let html = r#"<a href="attachment:files/report.docx">r</a>"#;
        let out = rewrite_local_urls(html, "S", "notes");
        assert!(out.contains(r#"href="/raw/S/files/report.docx""#), "{out}");
    }

    #[test]
    fn escaping_covers_attribute_contexts() {
        assert_eq!(esc("a\"b<c>&"), "a&quot;b&lt;c&gt;&amp;");
    }
}
