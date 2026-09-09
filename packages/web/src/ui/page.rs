//! HTML rendering for the minimal workspace UI.
//!
//! Every page is a plain server-rendered document: real `<a href>`
//! links, real `<form method="post">` forms, and one small inline
//! script for the client-side filter. There is no hydration, no wasm,
//! and no client-side router — a click is a normal navigation.

use crate::ui::space::{Entry, Space};
use crate::ui::urls;

/// Escape for HTML text and attribute contexts.
pub fn esc(s: &str) -> String {
    html_escape::encode_safe(s).into_owned()
}

fn fmt_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit + 1 < UNITS.len() {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[unit]) }
}

fn is_doc(entry: &Entry) -> bool {
    entry.editable()
}

// ---------------------------------------------------------------- shell

/// Wrap `body` in the two-column shell. `entries` powers the sidebar.
pub fn shell(
    title: &str,
    space: Option<&Space>,
    current: Option<&str>,
    entries: &[Entry],
    topbar: &str,
    body: &str,
) -> String {
    let space_name = space.map(|s| s.name.as_str()).unwrap_or("notez");
    let sidebar = match space {
        Some(s) => sidebar(s, current, entries),
        None => String::new(),
    };
    let home = match space {
        Some(s) => urls::space_url(&s.encoded),
        None => "/".to_string(),
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title} · notez</title>
<link rel="stylesheet" href="{css}">
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'><text y='26' font-size='26'>📝</text></svg>">
</head>
<body>
<input type="checkbox" id="nav" class="nav-toggle">
<div class="shell">
<aside class="sidebar">
<div class="brand"><a href="{home}">notez</a><span class="space-name">{space_name}</span></div>
{sidebar}
</aside>
<label class="scrim" for="nav" aria-hidden="true"></label>
<div class="main">
<header class="topbar"><label class="btn ghost menu-btn" for="nav" title="Toggle navigation">☰</label>{topbar}</header>
<main class="content">{body}</main>
</div>
</div>
<script>{SCRIPT}</script>
</body>
</html>
"#,
        title = esc(title),
        css = urls::css_url(),
        home = home,
        space_name = esc(space_name),
        sidebar = sidebar,
        topbar = topbar,
        body = body,
    )
}

const SCRIPT: &str = r#"
(function () {
  var filter = document.getElementById('filter');
  var list = document.getElementById('pages');
  if (filter && list) {
    var rows = Array.prototype.slice.call(list.querySelectorAll('[data-k]'));
    var count = document.getElementById('count');
    var apply = function () {
      var q = filter.value.trim().toLowerCase();
      var shown = 0;
      rows.forEach(function (row) {
        var hit = !q || row.getAttribute('data-k').indexOf(q) !== -1;
        row.style.display = hit ? '' : 'none';
        if (hit) shown++;
      });
      if (count) count.textContent = shown + '/' + rows.length;
    };
    filter.addEventListener('input', apply);
    apply();
  }
  document.addEventListener('keydown', function (e) {
    if (e.key === '/' && !/^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement.tagName)) {
      if (filter) { e.preventDefault(); filter.focus(); }
    }
  });
  var form = document.getElementById('editor-form');
  var area = document.getElementById('editor');
  if (form && area) {
    var dirty = false;
    var initial = area.value;
    area.addEventListener('input', function () { dirty = area.value !== initial; });
    area.addEventListener('keydown', function (e) {
      if ((e.ctrlKey || e.metaKey) && e.key === 's') { e.preventDefault(); form.submit(); }
      if (e.key === 'Tab') {
        e.preventDefault();
        var s = area.selectionStart, t = area.selectionEnd;
        area.value = area.value.slice(0, s) + '  ' + area.value.slice(t);
        area.selectionStart = area.selectionEnd = s + 2;
      }
    });
    window.addEventListener('beforeunload', function (e) {
      if (dirty) { e.preventDefault(); e.returnValue = ''; }
    });
    form.addEventListener('submit', function () { dirty = false; });
  }
})();
"#;

fn sidebar(space: &Space, current: Option<&str>, entries: &[Entry]) -> String {
    let mut pages = String::new();
    let mut files = String::new();
    let mut page_count = 0usize;
    let mut file_count = 0usize;
    for e in entries {
        let href = urls::view_url(&space.encoded, &e.locator);
        let cur = if Some(e.locator.as_str()) == current { " current" } else { "" };
        let dir = e.dir();
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("<span class=\"dir\">{}/</span>", esc(dir))
        };
        let row = format!(
            "<a class=\"pg{cur}\" href=\"{href}\" data-k=\"{k}\" title=\"{title_full}\">{prefix}{name}</a>",
            cur = cur,
            href = href,
            k = esc(&e.locator.to_lowercase()),
            title_full = esc(&e.locator),
            prefix = prefix,
            name = esc(e.file_name()),
        );
        if is_doc(e) {
            page_count += 1;
            pages.push_str(&row);
        } else {
            file_count += 1;
            files.push_str(&format!(
                "<a class=\"pg{cur}\" href=\"{href}\" data-k=\"{k}\" title=\"{title_full}\">{prefix}{name}<span class=\"tag\">{ext}</span></a>",
                cur = cur,
                href = href,
                k = esc(&e.locator.to_lowercase()),
                title_full = esc(&e.locator),
                prefix = prefix,
                name = esc(e.file_name()),
                ext = esc(&e.ext),
            ));
        }
    }
    let files_block = if file_count == 0 {
        String::new()
    } else {
        format!(
            "<details class=\"files\"><summary>attachments ({file_count})</summary><div class=\"pages\">{files}</div></details>"
        )
    };
    format!(
        r#"<div class="sb-top">
<input class="filter" id="filter" type="text" placeholder="filter pages…  ( / )" autocomplete="off" spellcheck="false">
<span class="edit-hint"><span id="count">{page_count}</span></span>
</div>
{files_block}
<div class="pages" id="pages">{pages}</div>"#,
        pages = pages,
        files_block = files_block,
        page_count = page_count,
    )
}

/// Topbar for a space-level page.
pub fn topbar_space(space: &Space, title: &str) -> String {
    format!(
        r#"<span class="title">{title}</span><span class="spacer"></span>
<a class="btn" href="{new}">+ New</a>
<form method="post" action="{scan}" style="display:inline"><button class="btn ghost" type="submit">Scan</button></form>"#,
        title = esc(title),
        new = urls::new_url(&space.encoded),
        scan = urls::scan_url(&space.encoded),
    )
}

/// Topbar for a document page.
pub fn topbar_doc(space: &Space, locator: &str, title: &str, editing: bool) -> String {
    let crumb = locator.to_string();
    let action = if editing {
        format!(
            "<a class=\"btn\" href=\"{view}\">Cancel</a>",
            view = urls::view_url(&space.encoded, locator)
        )
    } else {
        format!(
            "<a class=\"btn primary\" href=\"{edit}\">Edit</a><a class=\"btn ghost\" href=\"{raw}\">Raw</a>",
            edit = urls::edit_url(&space.encoded, locator),
            raw = urls::raw_url(&space.encoded, locator),
        )
    };
    format!(
        "<span class=\"title\">{title}</span><span class=\"crumb\">{crumb}</span><span class=\"spacer\"></span>{action}",
        title = esc(title),
        crumb = esc(&crumb),
    )
}

/// Topbar for an attachment page.
pub fn topbar_file(space: &Space, locator: &str, title: &str) -> String {
    format!(
        r#"<span class="title">{title}</span><span class="crumb">{crumb}</span><span class="spacer"></span>
<a class="btn ghost" href="{raw}" target="_blank" rel="noopener">Open raw</a>"#,
        title = esc(title),
        crumb = esc(locator),
        raw = urls::raw_url(&space.encoded, locator),
    )
}

// ---------------------------------------------------------------- pages

pub fn banner(kind: &str, text: &str) -> String {
    format!("<div class=\"banner {kind}\">{text}</div>", text = esc(text))
}

/// Space picker shown when no single default space exists.
pub fn picker(spaces: &[Space], message: Option<&str>) -> String {
    let mut rows = String::new();
    for s in spaces {
        rows.push_str(&format!(
            "<a class=\"space-row\" href=\"{url}\"><span>{name}</span><span class=\"path\">{path}</span></a>",
            url = urls::space_url(&s.encoded),
            name = esc(&s.name),
            path = esc(&s.root.to_string_lossy()),
        ));
    }
    if rows.is_empty() {
        rows.push_str("<div class=\"empty\">No space registered yet.</div>");
    }
    format!(
        r#"<div class="picker">
<h1>Spaces</h1>
{message}
<div class="spaces">{rows}</div>
<form method="post" action="/register">
<input type="text" name="path" placeholder="/home/you/Notes" autocomplete="off" spellcheck="false">
<button class="btn primary" type="submit">Open</button>
</form>
<p class="edit-hint">Registers the directory in <code>~/.config/notez/config.toml</code>.</p>
</div>"#,
        message = message.map(|m| banner("err", m)).unwrap_or_default(),
    )
}

/// Folder-grouped list of every document in the space.
pub fn page_list(space: &Space, entries: &[Entry]) -> String {
    let mut groups: std::collections::BTreeMap<&str, Vec<&Entry>> = std::collections::BTreeMap::new();
    for e in entries.iter().filter(|e| is_doc(e)) {
        groups.entry(e.dir()).or_default().push(e);
    }
    if groups.is_empty() {
        return "<p class=\"edit-hint\">No markdown or org files in this space yet.</p>".to_string();
    }
    let mut out = String::new();
    for (dir, rows) in groups {
        out.push_str(&format!(
            "<h3>{}</h3><ul>",
            if dir.is_empty() { "(root)" } else { dir }
        ));
        for e in rows {
            out.push_str(&format!(
                "<li><a href=\"{href}\">{title}</a> <span class=\"edit-hint\">{size}</span></li>",
                href = urls::view_url(&space.encoded, &e.locator),
                title = esc(&e.title),
                size = fmt_size(e.size),
            ));
        }
        out.push_str("</ul>");
    }
    out
}

/// Document view.
pub fn doc_view(space: &Space, locator: &str, body_html: &str, notice: &str) -> String {
    let raw = urls::raw_url(&space.encoded, locator);
    format!(
        r#"{notice}
<article class="doc">{body_html}</article>
<hr>
<p class="edit-hint"><a href="{raw}">raw source</a> · <a href="{edit}">edit</a></p>"#,
        notice = notice,
        body_html = body_html,
        raw = raw,
        edit = urls::edit_url(&space.encoded, locator),
    )
}

/// Editor view (shared by the edit route and the create route).
pub fn editor(
    space: &Space,
    locator: &str,
    revision: &str,
    content: &str,
    notice: &str,
) -> String {
    format!(
        r#"{notice}
<form id="editor-form" method="post" action="{save}">
<input type="hidden" name="locator" value="{locator}">
<input type="hidden" name="revision" value="{revision}">
<div class="edit-bar"><span class="edit-hint">Ctrl/Cmd-S saves · Tab indents</span><span class="spacer"></span><button class="btn primary" type="submit">Save</button></div>
<textarea class="editor" id="editor" name="content" spellcheck="false" autocomplete="off">{content}</textarea>
</form>"#,
        notice = notice,
        save = urls::save_url(&space.encoded),
        locator = esc(locator),
        revision = esc(revision),
        content = esc(content),
    )
}

/// Attachment / preview view.
pub fn file_view(preview_html: &str, notice: &str) -> String {
    format!(
        r#"{notice}
<article class="doc">{preview_html}</article>"#,
        notice = notice,
        preview_html = preview_html,
    )
}

/// Error page body.
pub fn error_page(message: &str) -> String {
    format!(
        "<div class=\"banner err\">{}</div><p class=\"edit-hint\">Go back to <a href=\"/\">the space picker</a>.</p>",
        esc(message)
    )
}

pub fn save_failure_text(failure: &crate::server::SaveFailure) -> String {
    use crate::server::SaveFailure;
    match failure {
        SaveFailure::StaleRevision { expected, actual } => format!(
            "The file changed on disk since you opened it (expected {}, found {}). Copy your text, reload, then paste it back.",
            &expected[..expected.len().min(12)],
            &actual[..actual.len().min(12)],
        ),
        SaveFailure::ReadOnly { reason } => format!("Read-only source: {reason}"),
        SaveFailure::NotFound { path } => format!("File not found: {path}"),
        SaveFailure::Unsupported { reason } => format!("Cannot edit this file: {reason}"),
        SaveFailure::Internal { message } => format!("Save failed: {message}"),
    }
}

// ---------------------------------------------------- URL rewriting

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
