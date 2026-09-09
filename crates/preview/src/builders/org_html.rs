use crate::Heading;

/// Render an Org-mode source string into an HTML body and a heading outline.
///
/// Dependency-free, block-structured renderer used by `OrgPreviewer`. It
/// supports the subset of Org that Notez spaces actually contain:
///
/// - Headings `*`..`*******` (levels 1..=6, deeper stars clamp to 6) with a
///   slugified `id` anchor, an optional TODO keyword badge
///   (`<span class="todo todo-todo">TODO</span>`) and an optional priority
///   badge (`<span class="prio">[#A]</span>`).
/// - `#+TITLE: x` becomes an `<h1>` only when the document has no level-1
///   heading; every other `#+KEY: value` line is dropped.
/// - Paragraphs: consecutive non-blank lines join into one `<p>`.
/// - Lists (`-`, `+`, indented `*`, `1.`, `1)`) nested by indentation, with
///   `[ ]` / `[x]` / `[-]` checkboxes.
/// - Inline emphasis (`*bold*`, `/italic/`, `_underline_`, `+strike+`),
///   code/verbatim (`=code=`, `~code~`) and `[[target][desc]]` links.
///   Delimiters follow Emacs Org's flanking rules, so `snake_case`,
///   `path/to/file`, `https://example.com/` and `对象/块/关系` stay literal.
/// - Image embeds for a paragraph that is exactly one image link.
/// - `#+BEGIN_SRC` / `EXAMPLE` / `VERSE` / `QUOTE` / `CENTER` blocks.
/// - Tables (`| a | b |`) with a `|---|` separator marking the header row.
/// - `:PROPERTIES:`..`:END:` drawers, `# ` comments and `-----` rules.
///
/// Every user-visible character is HTML-escaped; link `href`/`src` values are
/// emitted verbatim (metacharacters escaped) so the caller can rewrite
/// relative URLs itself.
pub fn render_org_html(src: &str) -> (String, Vec<Heading>) {
    let lines: Vec<&str> = src.lines().collect();
    let has_h1 = has_level_one_heading(&lines);
    let mut renderer = Renderer {
        out: String::with_capacity(src.len() + src.len() / 4 + 64),
        outline: Vec::new(),
        has_h1,
        title_emitted: false,
    };
    renderer.render_blocks(&lines);
    (renderer.out, renderer.outline)
}

// ---------------------------------------------------------------------------
// Block rendering
// ---------------------------------------------------------------------------

struct Renderer {
    out: String,
    outline: Vec<Heading>,
    has_h1: bool,
    title_emitted: bool,
}

impl Renderer {
    fn render_blocks(&mut self, lines: &[&str]) {
        let mut i = 0usize;
        while i < lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                i += 1;
                continue;
            }

            if let Some(open) = parse_begin_block(line) {
                i = self.render_block(lines, i, &open);
                continue;
            }
            if line.trim().eq_ignore_ascii_case(":PROPERTIES:") {
                i = skip_drawer(lines, i);
                continue;
            }
            if let Some(rest) = line.trim_start().strip_prefix("#+") {
                if let Some(title) = title_meta_value(rest) {
                    if !self.has_h1 && !self.title_emitted && !title.is_empty() {
                        self.title_emitted = true;
                        self.emit_heading(1, None, None, &title, &title);
                    }
                }
                i += 1;
                continue;
            }
            if line.starts_with("# ") || line.trim() == "#" {
                i += 1;
                continue;
            }
            if let Some(heading) = parse_heading(line) {
                self.emit_heading(
                    heading.level,
                    heading.todo,
                    heading.priority,
                    heading.raw_title,
                    heading.title,
                );
                i += 1;
                continue;
            }
            if is_horizontal_rule(line) {
                self.out.push_str("<hr>\n");
                i += 1;
                continue;
            }
            if line.trim_start().starts_with('|') {
                i = self.render_table(lines, i);
                continue;
            }
            if list_marker(line).is_some() {
                i = self.render_list(lines, i);
                continue;
            }
            i = self.render_paragraph(lines, i);
        }
    }

    fn emit_heading(
        &mut self,
        level: u8,
        todo: Option<&str>,
        priority: Option<char>,
        raw_title: &str,
        title: &str,
    ) {
        let digit = char::from_digit(level as u32, 10).unwrap_or('6');
        let anchor = slugify(raw_title);

        let mut inner = String::new();
        let mut has_badge = false;
        if let Some(todo) = todo {
            inner.push_str("<span class=\"todo todo-");
            inner.push_str(&todo.to_ascii_lowercase());
            inner.push_str("\">");
            inner.push_str(&html_escape(todo));
            inner.push_str("</span>");
            has_badge = true;
        }
        if let Some(priority) = priority {
            if has_badge {
                inner.push(' ');
            }
            inner.push_str("<span class=\"prio\">[#");
            inner.push(priority);
            inner.push_str("]</span>");
            has_badge = true;
        }
        let title_html = inline(title);
        if has_badge && !title_html.is_empty() {
            inner.push(' ');
        }
        inner.push_str(&title_html);

        self.out.push_str("<h");
        self.out.push(digit);
        self.out.push_str(" id=\"");
        self.out.push_str(&anchor);
        self.out.push('"');
        if has_badge {
            // The visible text is split across badge elements; keep the plain
            // heading text as the accessible name for consumers.
            self.out.push_str(" aria-label=\"");
            self.out.push_str(&html_escape(raw_title));
            self.out.push('"');
        }
        self.out.push('>');
        self.out.push_str(&inner);
        self.out.push_str("</h");
        self.out.push(digit);
        self.out.push_str(">\n");

        self.outline.push(Heading {
            level,
            title: raw_title.to_string(),
            anchor,
        });
    }

    fn render_block(&mut self, lines: &[&str], start: usize, open: &BlockOpen) -> usize {
        let mut i = start + 1;
        match open.kind {
            BlockKind::Quote | BlockKind::Center => {
                // Quote/center keep block structure; nested markers are
                // tracked so their `#+END_*` lines do not close the outer
                // block prematurely.
                let mut depth = 0usize;
                let mut inner: Vec<&str> = Vec::new();
                while i < lines.len() {
                    if let Some(keyword) = parse_end_keyword(lines[i]) {
                        if depth == 0 && keyword == open.keyword {
                            i += 1;
                            break;
                        }
                        if depth > 0 {
                            depth -= 1;
                        }
                        inner.push(lines[i]);
                        i += 1;
                        continue;
                    }
                    if parse_begin_block(lines[i]).is_some() {
                        depth += 1;
                    }
                    inner.push(lines[i]);
                    i += 1;
                }
                let tag = if open.kind == BlockKind::Quote {
                    "blockquote"
                } else {
                    "div"
                };
                self.out.push('<');
                self.out.push_str(tag);
                if open.kind == BlockKind::Center {
                    self.out.push_str(" class=\"center\"");
                }
                self.out.push_str(">\n");
                self.render_blocks(&inner);
                self.out.push_str("</");
                self.out.push_str(tag);
                self.out.push_str(">\n");
            }
            _ => {
                // Src/example/verse (and unknown blocks) are literal: nested
                // markers are text and only the matching `#+END_*` closes.
                let mut body: Vec<&str> = Vec::new();
                while i < lines.len() {
                    if parse_end_keyword(lines[i]).as_deref() == Some(open.keyword.as_str()) {
                        i += 1;
                        break;
                    }
                    body.push(lines[i]);
                    i += 1;
                }
                let escaped = html_escape(&body.join("\n"));
                match open.kind {
                    BlockKind::Src => {
                        self.out.push_str("<pre class=\"src\"><code");
                        if let Some(lang) = &open.lang {
                            self.out.push_str(" class=\"language-");
                            self.out.push_str(&html_escape(lang));
                            self.out.push('"');
                        }
                        self.out.push('>');
                        self.out.push_str(&escaped);
                        self.out.push_str("</code></pre>\n");
                    }
                    BlockKind::Example => {
                        self.out.push_str("<pre class=\"example\">");
                        self.out.push_str(&escaped);
                        self.out.push_str("</pre>\n");
                    }
                    BlockKind::Verse => {
                        self.out.push_str("<pre class=\"verse\">");
                        self.out.push_str(&escaped);
                        self.out.push_str("</pre>\n");
                    }
                    _ => {
                        self.out.push_str("<pre class=\"block\">");
                        self.out.push_str(&escaped);
                        self.out.push_str("</pre>\n");
                    }
                }
            }
        }
        i
    }

    fn render_table(&mut self, lines: &[&str], start: usize) -> usize {
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut i = start;
        while i < lines.len() && lines[i].trim_start().starts_with('|') {
            rows.push(split_table_row(lines[i]));
            i += 1;
        }

        let separator = rows.iter().position(|row| is_separator_row(row));
        let (head, body): (&[Vec<String>], &[Vec<String>]) = match separator {
            Some(index) => (&rows[..index], &rows[index + 1..]),
            None => (&rows[..0], &rows[..]),
        };

        self.out.push_str("<table>");
        if !head.is_empty() {
            self.out.push_str("<thead>");
            for row in head {
                self.render_table_row("th", row);
            }
            self.out.push_str("</thead>");
        }
        let body: Vec<&Vec<String>> = body
            .iter()
            .filter(|row| !is_separator_row(row))
            .collect();
        if !body.is_empty() {
            self.out.push_str("<tbody>");
            for row in &body {
                self.render_table_row("td", row);
            }
            self.out.push_str("</tbody>");
        }
        self.out.push_str("</table>\n");
        i
    }

    fn render_table_row(&mut self, cell_tag: &str, cells: &[String]) {
        self.out.push_str("<tr>");
        for cell in cells {
            self.out.push('<');
            self.out.push_str(cell_tag);
            self.out.push('>');
            self.out.push_str(&inline(cell));
            self.out.push_str("</");
            self.out.push_str(cell_tag);
            self.out.push('>');
        }
        self.out.push_str("</tr>");
    }

    fn render_list(&mut self, lines: &[&str], start: usize) -> usize {
        let mut items: Vec<ListItem> = Vec::new();
        let mut i = start;
        while i < lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                break;
            }
            if let Some(marker) = list_marker(line) {
                items.push(ListItem {
                    indent: indent_bytes(line),
                    level: marker.level,
                    ordered: marker.ordered,
                    number: marker.number,
                    checkbox: marker.checkbox,
                    text: marker.text.to_string(),
                });
                i += 1;
                continue;
            }
            let continuation = items
                .last()
                .is_some_and(|last| indent_bytes(line) > last.indent && !is_block_start(line));
            if continuation {
                let last = items.last_mut().expect("checked above");
                join_text(&mut last.text, line.trim());
                i += 1;
                continue;
            }
            break;
        }

        let mut pos = 0usize;
        while pos < items.len() {
            let level = items[pos].level;
            self.render_list_level(&items, &mut pos, level);
        }
        i
    }

    fn render_list_level(&mut self, items: &[ListItem], pos: &mut usize, level: usize) {
        let ordered = items[*pos].ordered;
        let tag = if ordered { "ol" } else { "ul" };
        match items[*pos].number {
            Some(number) if ordered && number != 1 => {
                self.out.push_str("<ol start=\"");
                self.out.push_str(&number.to_string());
                self.out.push_str("\">");
            }
            _ => {
                self.out.push('<');
                self.out.push_str(tag);
                self.out.push('>');
            }
        }

        while *pos < items.len() {
            let item = &items[*pos];
            if item.level < level {
                break;
            }
            if item.level > level {
                self.render_list_level(items, pos, item.level);
                continue;
            }
            if item.ordered != ordered {
                break;
            }
            self.out.push_str("<li>");
            if let Some(marker) = item.checkbox {
                self.out.push_str(checkbox_html(marker));
                self.out.push(' ');
            }
            self.out.push_str(&inline(&item.text));
            *pos += 1;
            while *pos < items.len() && items[*pos].level > level {
                let child_level = items[*pos].level;
                self.render_list_level(items, pos, child_level);
            }
            self.out.push_str("</li>");
        }

        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push('>');
    }

    fn render_paragraph(&mut self, lines: &[&str], start: usize) -> usize {
        let mut text = String::new();
        let mut i = start;
        while i < lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                break;
            }
            if i > start && is_block_start(line) {
                break;
            }
            join_text(&mut text, line.trim());
            i += 1;
        }

        if let Some((src, alt)) = parse_image_embed(&text) {
            self.out.push_str("<img src=\"");
            self.out.push_str(&html_escape(&src));
            self.out.push_str("\" alt=\"");
            self.out.push_str(&html_escape(&alt));
            self.out.push_str("\">\n");
        } else {
            self.out.push_str("<p>");
            self.out.push_str(&inline(&text));
            self.out.push_str("</p>\n");
        }
        i
    }
}

// ---------------------------------------------------------------------------
// Block syntax
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Src,
    Example,
    Verse,
    Quote,
    Center,
    Other,
}

struct BlockOpen {
    kind: BlockKind,
    keyword: String,
    lang: Option<String>,
}

fn parse_begin_block(line: &str) -> Option<BlockOpen> {
    let rest = line.trim_start().strip_prefix("#+")?;
    let prefix = rest.get(..6)?;
    if !prefix.eq_ignore_ascii_case("begin_") {
        return None;
    }
    let after = &rest[6..];
    let keyword_end = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    if keyword_end == 0 {
        return None;
    }
    let keyword = after[..keyword_end].to_ascii_uppercase();
    let args = after[keyword_end..].trim();
    let kind = match keyword.as_str() {
        "SRC" => BlockKind::Src,
        "EXAMPLE" => BlockKind::Example,
        "VERSE" => BlockKind::Verse,
        "QUOTE" => BlockKind::Quote,
        "CENTER" => BlockKind::Center,
        _ => BlockKind::Other,
    };
    let lang = if kind == BlockKind::Src {
        args.split_whitespace()
            .next()
            .filter(|token| !token.starts_with('-'))
            .map(str::to_string)
    } else {
        None
    };
    Some(BlockOpen {
        kind,
        keyword,
        lang,
    })
}

fn parse_end_keyword(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("#+")?;
    let prefix = rest.get(..4)?;
    if !prefix.eq_ignore_ascii_case("end_") {
        return None;
    }
    let after = &rest[4..];
    let keyword_end = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    if keyword_end == 0 {
        return None;
    }
    Some(after[..keyword_end].to_ascii_uppercase())
}

/// `#+TITLE: x` → `Some("x")`; every other `#+KEY: value` → `None`.
fn title_meta_value(rest: &str) -> Option<String> {
    let (key, value) = rest.split_once(':')?;
    if key.trim().eq_ignore_ascii_case("title") {
        Some(value.trim().to_string())
    } else {
        None
    }
}

fn skip_drawer(lines: &[&str], start: usize) -> usize {
    let mut i = start + 1;
    while i < lines.len() {
        if lines[i].trim().eq_ignore_ascii_case(":END:") {
            return i + 1;
        }
        i += 1;
    }
    start + 1
}

fn has_level_one_heading(lines: &[&str]) -> bool {
    let mut literal: Option<String> = None;
    for line in lines {
        if let Some(keyword) = &literal {
            if parse_end_keyword(line).as_deref() == Some(keyword.as_str()) {
                literal = None;
            }
            continue;
        }
        if let Some(open) = parse_begin_block(line) {
            if open.kind != BlockKind::Quote && open.kind != BlockKind::Center {
                literal = Some(open.keyword.clone());
            }
            continue;
        }
        if matches!(parse_heading(line), Some(heading) if heading.level == 1) {
            return true;
        }
    }
    false
}

fn is_block_start(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("#+") {
        return true;
    }
    if line.starts_with("# ") || line.trim() == "#" {
        return true;
    }
    if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
        return true;
    }
    if parse_heading(line).is_some() {
        return true;
    }
    if trimmed.starts_with('|') {
        return true;
    }
    if list_marker(line).is_some() {
        return true;
    }
    is_horizontal_rule(line)
}

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 5 && trimmed.chars().all(|c| c == '-')
}

// ---------------------------------------------------------------------------
// Headings
// ---------------------------------------------------------------------------

struct HeadingParts<'a> {
    level: u8,
    todo: Option<&'a str>,
    priority: Option<char>,
    /// Everything after the stars, before badge stripping (outline/slug/anchor).
    raw_title: &'a str,
    /// Title text after the TODO keyword and priority badge.
    title: &'a str,
}

fn parse_heading(line: &str) -> Option<HeadingParts<'_>> {
    let stars = line.bytes().take_while(|b| *b == b'*').count();
    if stars == 0 {
        return None;
    }
    let rest = &line[stars..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let raw_title = rest.trim();
    if raw_title.is_empty() {
        return None;
    }
    let mut title = raw_title;
    let mut todo = None;
    let mut priority = None;
    if let Some((keyword, after)) = split_todo(title) {
        todo = Some(keyword);
        title = after.trim_start();
    }
    if let Some((marker, after)) = split_priority(title) {
        priority = Some(marker);
        title = after.trim_start();
    }
    Some(HeadingParts {
        level: stars.min(6) as u8,
        todo,
        priority,
        raw_title,
        title,
    })
}

fn split_todo(text: &str) -> Option<(&str, &str)> {
    let end = text.find(' ').unwrap_or(text.len());
    let (word, rest) = text.split_at(end);
    if matches!(word, "TODO" | "NEXT" | "WAIT" | "DONE" | "QUIT") {
        Some((word, rest))
    } else {
        None
    }
}

fn split_priority(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    if chars.next() != Some('[') || chars.next() != Some('#') {
        return None;
    }
    let marker = chars.next()?;
    if !marker.is_ascii_uppercase() || chars.next() != Some(']') {
        return None;
    }
    let rest = &text[4..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    Some((marker, rest))
}

// ---------------------------------------------------------------------------
// Lists
// ---------------------------------------------------------------------------

struct ListMarker<'a> {
    level: usize,
    ordered: bool,
    number: Option<u64>,
    checkbox: Option<char>,
    text: &'a str,
}

struct ListItem {
    indent: usize,
    level: usize,
    ordered: bool,
    number: Option<u64>,
    checkbox: Option<char>,
    text: String,
}

fn list_marker(line: &str) -> Option<ListMarker<'_>> {
    let indent = indent_bytes(line);
    let rest = &line[indent..];
    let first = rest.chars().next()?;

    let (ordered, number, after_bullet) = if first.is_ascii_digit() {
        let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
        let after_digits = &rest[digits..];
        let separator = after_digits.chars().next()?;
        if separator != '.' && separator != ')' {
            return None;
        }
        let after = &after_digits[1..];
        if !after.is_empty() && !after.starts_with(' ') {
            return None;
        }
        (true, rest[..digits].parse::<u64>().ok(), after)
    } else if first == '-' || first == '+' || (first == '*' && indent > 0) {
        let after = &rest[1..];
        if !after.is_empty() && !after.starts_with(' ') {
            return None;
        }
        (false, None, after)
    } else {
        return None;
    };

    let mut text = after_bullet.trim_start();
    let mut checkbox = None;
    if let Some((marker, remainder)) = split_checkbox(text) {
        checkbox = Some(marker);
        text = remainder;
    }

    Some(ListMarker {
        level: indent / 2,
        ordered,
        number,
        checkbox,
        text,
    })
}

fn split_checkbox(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    if chars.next() != Some('[') {
        return None;
    }
    let marker = chars.next()?;
    if !matches!(marker, ' ' | 'x' | 'X' | '-') || chars.next() != Some(']') {
        return None;
    }
    let rest = &text[3..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    Some((marker, rest.trim_start()))
}

fn checkbox_html(marker: char) -> &'static str {
    match marker {
        'x' | 'X' => "<input type=\"checkbox\" checked disabled>",
        '-' => "<input type=\"checkbox\" disabled data-state=\"partial\">",
        _ => "<input type=\"checkbox\" disabled>",
    }
}

fn indent_bytes(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
    trimmed.split('|').map(|cell| cell.trim().to_string()).collect()
}

fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            !cell.is_empty()
                && cell.contains('-')
                && cell.chars().all(|c| matches!(c, '-' | '+' | ':'))
        })
}

// ---------------------------------------------------------------------------
// Inline formatting
// ---------------------------------------------------------------------------

fn inline(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];

        if c == '[' && chars.get(i + 1) == Some(&'[') {
            if let Some(link) = parse_link(&chars, i) {
                out.push_str(&link_html(&link));
                i = link.end;
                continue;
            }
        }

        if matches!(c, '=' | '~') && emphasis_open_ok(&chars, i) {
            if let Some(end) = find_emphasis_close(&chars, i, c) {
                let inner: String = chars[i + 1..end].iter().collect();
                out.push_str("<code>");
                out.push_str(&html_escape(&inner));
                out.push_str("</code>");
                i = end + 1;
                continue;
            }
        }

        if matches!(c, '*' | '/' | '_' | '+') && emphasis_open_ok(&chars, i) {
            if let Some(end) = find_emphasis_close(&chars, i, c) {
                let inner: String = chars[i + 1..end].iter().collect();
                let tag = match c {
                    '*' => "strong",
                    '/' => "em",
                    '_' => "u",
                    _ => "del",
                };
                out.push('<');
                out.push_str(tag);
                out.push('>');
                out.push_str(&inline(&inner));
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
                i = end + 1;
                continue;
            }
        }

        push_escaped(&mut out, c);
        i += 1;
    }
    out
}

/// A delimiter opens markup only when the character after it is neither
/// whitespace nor the delimiter itself and the character before it is one of
/// Org's opening borders (start of text, whitespace, or `-('" {`).
///
/// These are Emacs Org's defaults (`org-emphasis-regexp-components`), which
/// keeps `snake_case`, `a*b*c`, `path/to/file`, `https://example.com/` and CJK
/// enumerations such as `对象/块/关系` literal while `*bold*` and `=code=`
/// still work.
fn emphasis_open_ok(chars: &[char], index: usize) -> bool {
    let next = match chars.get(index + 1) {
        Some(next) => *next,
        None => return false,
    };
    if next.is_whitespace() || next == chars[index] {
        return false;
    }
    match index.checked_sub(1).map(|i| chars[i]) {
        None => true,
        Some(prev) => prev.is_whitespace() || matches!(prev, '-' | '(' | '\'' | '"' | '{'),
    }
}

fn find_emphasis_close(chars: &[char], open: usize, delimiter: char) -> Option<usize> {
    let mut i = open + 2;
    while i < chars.len() {
        if chars[i] == delimiter && !chars[i - 1].is_whitespace() {
            match chars.get(i + 1) {
                None => return Some(i),
                Some(next)
                    if next.is_whitespace()
                        || matches!(
                            next,
                            '-' | '.' | ',' | ':' | '!' | '?' | ';' | '\'' | '"' | ')' | '}' | '\\'
                        ) =>
                {
                    return Some(i);
                }
                Some(_) => {}
            }
        }
        i += 1;
    }
    None
}

struct Link {
    target: String,
    desc: Option<String>,
    end: usize,
}

fn parse_link(chars: &[char], start: usize) -> Option<Link> {
    let mut i = start + 2;
    let mut target = String::new();
    while i < chars.len() {
        let c = chars[i];
        if c == ']' && chars.get(i + 1) == Some(&']') {
            return Some(Link {
                target,
                desc: None,
                end: i + 2,
            });
        }
        if c == ']' && chars.get(i + 1) == Some(&'[') {
            let desc_start = i + 2;
            let mut j = desc_start;
            while j < chars.len() {
                if chars[j] == ']' && chars.get(j + 1) == Some(&']') {
                    return Some(Link {
                        target,
                        desc: Some(chars[desc_start..j].iter().collect()),
                        end: j + 2,
                    });
                }
                j += 1;
            }
            return None;
        }
        if c == '[' && chars.get(i + 1) == Some(&'[') {
            return None;
        }
        target.push(c);
        i += 1;
    }
    None
}

fn link_html(link: &Link) -> String {
    let external = link.target.starts_with("http://") || link.target.starts_with("https://");
    let href = link.target.strip_prefix("file:").unwrap_or(&link.target);
    let desc = link
        .desc
        .clone()
        .unwrap_or_else(|| default_link_desc(&link.target));

    let mut out = String::new();
    out.push_str("<a href=\"");
    out.push_str(&html_escape(href));
    out.push('"');
    if external {
        out.push_str(" target=\"_blank\" rel=\"noopener noreferrer\"");
    }
    out.push('>');
    out.push_str(&inline(&desc));
    out.push_str("</a>");
    out
}

fn default_link_desc(target: &str) -> String {
    let stripped = target
        .strip_prefix("file:")
        .or_else(|| target.strip_prefix("attachment:"))
        .unwrap_or(target);
    let segment = stripped.rsplit('/').next().unwrap_or(stripped);
    if segment.is_empty() {
        target.to_string()
    } else {
        segment.to_string()
    }
}

fn parse_image_embed(text: &str) -> Option<(String, String)> {
    let chars: Vec<char> = text.chars().collect();
    if chars.first() != Some(&'[') {
        return None;
    }
    let link = parse_link(&chars, 0)?;
    if link.end != chars.len() {
        return None;
    }
    let src = link
        .target
        .strip_prefix("file:")
        .unwrap_or(&link.target)
        .to_string();
    if !is_image_path(&src) {
        return None;
    }
    let alt = link
        .desc
        .clone()
        .unwrap_or_else(|| default_link_desc(&link.target));
    Some((src, alt))
}

fn is_image_path(path: &str) -> bool {
    let path = path.split(['#', '?']).next().unwrap_or(path);
    match path.rsplit_once('.') {
        Some((_, ext)) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "avif"
        ),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Join wrapped source lines. A space is inserted between lines unless both
/// sides are CJK/full-width characters, so Chinese prose does not gain
/// spurious spaces at line breaks.
fn join_text(acc: &mut String, next: &str) {
    if acc.is_empty() {
        acc.push_str(next);
        return;
    }
    let last = acc.chars().next_back().unwrap_or(' ');
    let first = next.chars().next().unwrap_or(' ');
    if !is_cjk(last) || !is_cjk(first) {
        acc.push(' ');
    }
    acc.push_str(next);
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{2014}' | '\u{2018}'..='\u{201F}' | '\u{2026}'
        | '\u{3000}'..='\u{303F}' | '\u{3040}'..='\u{30FF}'
        | '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}' | '\u{FE30}'..='\u{FE4F}'
        | '\u{FF00}'..='\u{FFEF}'
        | '\u{20000}'..='\u{2FA1F}')
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
        push_escaped(&mut out, c);
    }
    out
}

fn push_escaped(out: &mut String, c: char) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&#39;"),
        _ => out.push(c),
    }
}

#[cfg(test)]
mod tests {
    use super::render_org_html;

    fn render(src: &str) -> String {
        render_org_html(src).0
    }

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

    #[test]
    fn heading_todo_and_priority_badges() {
        let (html, outline) = render_org_html("* TODO [#A] Ship it\n\n** DONE shipped\n");
        assert!(
            html.contains("<span class=\"todo todo-todo\">TODO</span>"),
            "{html}"
        );
        assert!(html.contains("<span class=\"prio\">[#A]</span>"), "{html}");
        assert!(html.contains("Ship it"), "{html}");
        assert!(
            html.contains("<span class=\"todo todo-done\">DONE</span> shipped"),
            "{html}"
        );
        assert_eq!(outline[0].level, 1);
        assert_eq!(outline[0].title, "TODO [#A] Ship it");
        assert_eq!(outline[0].anchor, "todo-a-ship-it");
        assert_eq!(outline[1].level, 2);
        assert_eq!(outline[1].anchor, "done-shipped");
    }

    #[test]
    fn deep_headings_clamp_to_six() {
        let (html, outline) = render_org_html("******* deep\n");
        assert!(html.contains("<h6 id=\"deep\">deep</h6>"), "{html}");
        assert_eq!(outline[0].level, 6);
    }

    #[test]
    fn title_meta_becomes_h1_only_without_level_one_heading() {
        let (html, outline) = render_org_html("#+TITLE: My Notes\n\nbody\n");
        assert!(html.contains("<h1 id=\"my-notes\">My Notes</h1>"), "{html}");
        assert!(html.contains("<p>body</p>"), "{html}");
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].level, 1);
        assert_eq!(outline[0].anchor, "my-notes");

        let (html, outline) = render_org_html("#+TITLE: Dropped\n\n* Real\n");
        assert_eq!(html.matches("<h1").count(), 1, "{html}");
        assert!(!html.contains("Dropped"), "{html}");
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].title, "Real");
    }

    #[test]
    fn paragraphs_join_wrapped_lines_until_blank_line() {
        let html = render("one\ntwo\n\nthree\n");
        assert!(html.contains("<p>one two</p>"), "{html}");
        assert!(html.contains("<p>three</p>"), "{html}");
        assert_eq!(html.matches("<p>").count(), 2, "{html}");

        let html = render("新的\n笔记格式\n");
        assert!(html.contains("<p>新的笔记格式</p>"), "{html}");
    }

    #[test]
    fn nested_lists_and_checkboxes() {
        let html = render("- top\n  - child\n- [ ] todo\n- [x] done\n- [-] half\n");
        assert!(html.contains("<li>top<ul><li>child</li></ul></li>"), "{html}");
        assert!(html.contains("<input type=\"checkbox\" disabled> todo"), "{html}");
        assert!(
            html.contains("<input type=\"checkbox\" checked disabled> done"),
            "{html}"
        );
        assert!(
            html.contains("<input type=\"checkbox\" disabled data-state=\"partial\"> half"),
            "{html}"
        );
    }

    #[test]
    fn ordered_lists_keep_start_number() {
        let html = render("1. one\n2. two\n");
        assert!(html.contains("<ol><li>one</li><li>two</li></ol>"), "{html}");

        let html = render("3. three\n");
        assert!(html.contains("<ol start=\"3\"><li>three</li></ol>"), "{html}");
    }

    #[test]
    fn inline_emphasis_variants() {
        let html = render("*bold* /italic/ _under_ +strike+ =code= ~verb~\n");
        for expected in [
            "<strong>bold</strong>",
            "<em>italic</em>",
            "<u>under</u>",
            "<del>strike</del>",
            "<code>code</code>",
            "<code>verb</code>",
        ] {
            assert!(html.contains(expected), "missing {expected} in {html}");
        }
    }

    #[test]
    fn emphasis_needs_boundaries_and_code_spans_are_literal() {
        let html = render("snake_case_name and a*b*c and path/to/file\n");
        assert!(!html.contains("<u>"), "{html}");
        assert!(!html.contains("<strong>"), "{html}");
        assert!(!html.contains("<em>"), "{html}");

        let html = render("=*not bold*= and ~/not italic/~\n");
        assert!(html.contains("<code>*not bold*</code>"), "{html}");
        assert!(html.contains("<code>/not italic/</code>"), "{html}");
        assert!(!html.contains("<strong>"), "{html}");

        let html = render("对象/块/关系 与 排序/标签/视图\n");
        assert!(!html.contains("<em>"), "{html}");

        let html = render("本文的 *重点* 请注意\n");
        assert!(html.contains("<strong>重点</strong>"), "{html}");

        let html = render("see https://example.com/ and api/v1 here\n");
        assert!(!html.contains("<em>"), "{html}");
    }

    #[test]
    fn links_keep_targets_verbatim_and_default_descriptions() {
        let html = render("[[file:projects/index.org]]\n");
        assert!(html.contains("<a href=\"projects/index.org\">index.org</a>"), "{html}");

        let html = render("[[./silverbullet.org][Silver Bullet]]\n");
        assert!(
            html.contains("<a href=\"./silverbullet.org\">Silver Bullet</a>"),
            "{html}"
        );

        let html = render("[[https://example.com/a][Ex]]\n");
        assert!(html.contains("href=\"https://example.com/a\""), "{html}");
        assert!(html.contains("target=\"_blank\""), "{html}");
        assert!(html.contains(">Ex</a>"), "{html}");

        let html = render("[[attachment:paper.pdf]]\n");
        assert!(html.contains("<a href=\"attachment:paper.pdf\">paper.pdf</a>"), "{html}");

        let html = render("[[id:01HZX9]]\n");
        assert!(html.contains("<a href=\"id:01HZX9\">id:01HZX9</a>"), "{html}");

        let html = render("see [[../x.png]] here\n");
        assert!(html.contains("href=\"../x.png\""), "{html}");
    }

    #[test]
    fn image_embed_only_for_image_links() {
        let html = render("[[file:shot.png]]\n");
        assert!(html.contains("<img src=\"shot.png\" alt=\"shot.png\">"), "{html}");
        assert!(!html.contains("<a "), "{html}");

        let html = render("[[file:notes.org]]\n");
        assert!(html.contains("<a href=\"notes.org\">notes.org</a>"), "{html}");
        assert!(!html.contains("<img"), "{html}");
    }

    #[test]
    fn src_blocks_escape_content_and_keep_language() {
        let html = render("#+BEGIN_SRC rust\nfn main() { println!(\"<hi>\"); }\n#+END_SRC\n");
        assert!(html.contains("<pre class=\"src\"><code class=\"language-rust\">"), "{html}");
        assert!(html.contains("&lt;hi&gt;"), "{html}");
        assert!(html.contains("&quot;"), "{html}");
        assert!(!html.contains("<hi>"), "{html}");

        let html = render("#+begin_src\nplain\n#+end_src\n");
        assert!(html.contains("<pre class=\"src\"><code>plain</code></pre>"), "{html}");
        assert!(!html.contains("language-"), "{html}");
    }

    #[test]
    fn nested_markers_inside_example_are_literal() {
        let html = render("#+BEGIN_EXAMPLE\n#+BEGIN_SRC rust\n#+END_EXAMPLE\n");
        assert!(
            html.contains("<pre class=\"example\">#+BEGIN_SRC rust</pre>"),
            "{html}"
        );
        assert!(!html.contains("<pre class=\"src\">"), "{html}");
    }

    #[test]
    fn quote_verse_and_center_blocks() {
        let html = render("#+BEGIN_QUOTE\nquoted *bold* text\n#+END_QUOTE\n");
        assert!(
            html.contains("<blockquote>\n<p>quoted <strong>bold</strong> text</p>\n</blockquote>"),
            "{html}"
        );

        let html = render("#+BEGIN_VERSE\nline one\nline two\n#+END_VERSE\n");
        assert!(
            html.contains("<pre class=\"verse\">line one\nline two</pre>"),
            "{html}"
        );

        let html = render("#+BEGIN_CENTER\ncentered\n#+END_CENTER\n");
        assert!(
            html.contains("<div class=\"center\">\n<p>centered</p>\n</div>"),
            "{html}"
        );
    }

    #[test]
    fn tables_split_header_on_separator_row() {
        let html = render("| Name | Value |\n|------+------|\n| a | 1 |\n| b | 2 |\n");
        assert!(
            html.contains("<table><thead><tr><th>Name</th><th>Value</th></tr></thead>"),
            "{html}"
        );
        assert!(
            html.contains("<tbody><tr><td>a</td><td>1</td></tr><tr><td>b</td><td>2</td></tr></tbody>"),
            "{html}"
        );
        assert!(!html.contains("<td>------+------</td>"), "{html}");

        let html = render("| a | b |\n| c | d |\n");
        assert!(!html.contains("<thead>"), "{html}");
        assert!(
            html.contains("<tbody><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></tbody>"),
            "{html}"
        );
    }

    #[test]
    fn table_cells_get_inline_markup() {
        let html = render("| *bold* | [[file:x.org][link]] |\n");
        assert!(html.contains("<td><strong>bold</strong></td>"), "{html}");
        assert!(html.contains("<td><a href=\"x.org\">link</a></td>"), "{html}");
    }

    #[test]
    fn drawers_and_comments_are_dropped() {
        let html = render("* Head\n:PROPERTIES:\n:ID: 01HZX9\n:END:\n# a comment\nvisible\n");
        assert!(html.contains("<p>visible</p>"), "{html}");
        assert!(!html.contains("PROPERTIES"), "{html}");
        assert!(!html.contains("01HZX9"), "{html}");
        assert!(!html.contains("a comment"), "{html}");
    }

    #[test]
    fn horizontal_rule() {
        let html = render("before\n\n-----\n\nafter\n");
        assert!(html.contains("<hr>"), "{html}");
        assert!(!html.contains("<p>-----</p>"), "{html}");
        assert!(html.contains("<p>before</p>"), "{html}");
        assert!(html.contains("<p>after</p>"), "{html}");
    }

    #[test]
    fn dangerous_text_is_escaped() {
        let html = render("* <script>alert(\"x\")</script>\n\ntext & 'quote'\n");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(html.contains("&amp;"), "{html}");
        assert!(html.contains("&#39;quote&#39;"), "{html}");
    }

    #[test]
    fn headings_inside_literal_blocks_are_not_outline_entries() {
        let (html, outline) = render_org_html("#+TITLE: T\n#+BEGIN_SRC\n* fake\n#+END_SRC\n");
        assert!(html.contains("<h1 id=\"t\">T</h1>"), "{html}");
        assert!(html.contains("* fake"), "{html}");
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].title, "T");
    }
}
