use crate::Heading;

/// Render an Org-mode source string into a minimal HTML body and a heading outline.
///
/// This is intentionally a tiny, predictable renderer. It supports:
/// - `*`, `**`, ... headings (levels 1..=6) — produced as `<h1>..<h6>` with a
///   slugified `id` anchor.
/// - `#+KEYWORD: value` metadata lines are dropped from the body (they are
///   rendered by the page chrome).
/// - Everything else is HTML-escaped and emitted as a paragraph line.
///
/// It is the *helper* used by `OrgPreviewer`; the previewer itself supplies
/// the source text from `ctx.resource.properties["body"]`.
pub fn render_org_html(src: &str) -> (String, Vec<Heading>) {
    let mut out = String::new();
    let mut outline = Vec::new();
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix('*') {
            if rest.starts_with(' ') {
                let stars = line.chars().take_while(|c| *c == '*').count();
                let level = stars.min(6) as u8;
                let title = rest.trim().to_string();
                if title.is_empty() {
                    continue;
                }
                let anchor = slugify(&title);
                outline.push(Heading {
                    level,
                    title: title.clone(),
                    anchor: anchor.clone(),
                });
                out.push_str(&format!(
                    "<h{level} id=\"{anchor}\">{title}</h{level}>",
                    level = level,
                    anchor = anchor,
                    title = html_escape(&title),
                ));
                out.push('\n');
                continue;
            }
        }
        if line.starts_with("#+") {
            // metadata — handled by the page chrome
            continue;
        }
        out.push_str(&html_escape(line));
        out.push('\n');
    }
    (out, outline)
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::render_org_html;

    #[test]
    fn heading_anchor_is_slugified() {
        let (html, outline) = render_org_html("#+TITLE: X\n\n* Hello World\n");
        assert!(html.contains("<h1"), "expected <h1 in html: {html}");
        assert!(
            html.contains("id=\"hello-world\""),
            "expected slugified id in html: {html}"
        );
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].title, "Hello World");
        assert_eq!(outline[0].anchor, "hello-world");
        assert_eq!(outline[0].level, 1);
    }
}
