//! Surgical source-file writer.
//!
//! [`TextPatch`] addresses a byte-exact region of a text file by line
//! number plus the exact old text expected there; [`FsSpanWriter`]
//! applies patches back-to-front, verifying each region's current
//! content before replacing it, and commits atomically
//! (`<file>.tmp` + rename). Any mismatch aborts the whole batch —
//! the "surgical" guarantee: nothing outside the patched regions can
//! change, and a stale patch can never corrupt unrelated content.
//!
//! This is the mechanism behind the MVP-1 gate "写回必须是外科手术式
//! 补丁，不得倒灌正文".

use std::fs;
use std::io;
use std::path::Path;

/// One surgical replacement: lines `line_no` (1-based, inclusive
/// range `line_no..=line_no_end`) currently equal to `old_lines` must
/// be replaced by `new_lines`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPatch {
    /// 1-based first line of the region.
    pub line_no: usize,
    /// 1-based last line of the region (inclusive).
    pub line_no_end: usize,
    /// Exact current content of the region, one entry per line.
    pub old_lines: Vec<String>,
    /// Replacement content, one entry per line (may be empty).
    pub new_lines: Vec<String>,
}

impl TextPatch {
    pub fn replace_line(line_no: usize, old: impl Into<String>, new: impl Into<String>) -> Self {
        Self {
            line_no,
            line_no_end: line_no,
            old_lines: vec![old.into()],
            new_lines: vec![new.into()],
        }
    }

    pub fn insert_after(line_no: usize, new_lines: Vec<String>) -> Self {
        Self {
            line_no,
            line_no_end: line_no,
            old_lines: Vec::new(),
            new_lines,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WriterError {
    #[error("I/O error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("patch mismatch at line {line}: file content drifted from scanned snapshot")]
    StalePatch { line: usize },
    #[error("line {line} out of range (file has {total} lines)")]
    OutOfRange { line: usize, total: usize },
}

/// Filesystem-backed surgical writer. The only production adapter;
/// tests use in-memory line vectors directly through
/// [`apply_to_lines`].
pub struct FsSpanWriter;

impl FsSpanWriter {
    /// Apply `patches` to `path` atomically. Patches are applied
    /// bottom-up so earlier line numbers stay valid; each patch's
    /// `old_lines` must match the file exactly or the entire batch
    /// fails without touching the file.
    pub fn apply(path: &Path, patches: &[TextPatch]) -> Result<(), WriterError> {
        let content = fs::read_to_string(path).map_err(|e| WriterError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
        // Note: `lines()` drops info about a trailing newline; keep
        // one if the original had one.
        let had_trailing_newline = content.ends_with('\n');
        lines = apply_to_lines(lines, patches)?;
        let mut out = lines.join("\n");
        if had_trailing_newline {
            out.push('\n');
        }
        // Atomic commit: write sibling temp file then rename.
        let tmp = path.with_extension(format!(
            "{}tmp",
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| format!("{e}."))
                .unwrap_or_default()
        ));
        fs::write(&tmp, out).map_err(|e| WriterError::Io {
            path: tmp.display().to_string(),
            source: e,
        })?;
        fs::rename(&tmp, path).map_err(|e| WriterError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }
}

/// Pure-line variant used by tests and by callers that already hold
/// the file content. Consumes and returns the line vector; patches
/// are validated before any is applied.
pub fn apply_to_lines(
    mut lines: Vec<String>,
    patches: &[TextPatch],
) -> Result<Vec<String>, WriterError> {
    // Validate everything up front so a bad batch never half-applies.
    let total = lines.len();
    for p in patches {
        if p.line_no == 0 || p.line_no > total || p.line_no_end > total || p.line_no > p.line_no_end
        {
            return Err(WriterError::OutOfRange {
                line: p.line_no,
                total,
            });
        }
        let start = p.line_no - 1;
        let end = p.line_no_end; // exclusive slice bound
        let current: Vec<String> = lines[start..end].to_vec();
        // Insertions carry empty old_lines: they only require the
        // anchor line to exist.
        if !p.old_lines.is_empty() && current != p.old_lines {
            return Err(WriterError::StalePatch { line: p.line_no });
        }
    }
    for p in patches.iter().rev() {
        let start = p.line_no - 1;
        let end = p.line_no_end; // exclusive
        if p.old_lines.is_empty() {
            lines.splice(start + 1..start + 1, p.new_lines.iter().cloned());
        } else {
            lines.splice(start..end, p.new_lines.iter().cloned());
        }
    }
    Ok(lines)
}

/// Rewrite the TODO state token on one Org heading line.
///
/// The heading line looks like `* STATE [#A] Title`; only the state
/// word is replaced. Returns the patch (caller applies).
pub fn org_heading_state_patch(
    heading_line_no: usize,
    heading_line: &str,
    new_state: &str,
) -> Option<TextPatch> {
    let mut parts = heading_line.splitn(3, ' ');
    let stars = parts.next()?;
    if !stars.starts_with('*') {
        return None;
    }
    let old_state = parts.next()?;
    let rest = parts.next().unwrap_or_default();
    Some(TextPatch::replace_line(
        heading_line_no,
        heading_line.to_string(),
        format!("{stars} {new_state} {rest}").trim_end().to_string(),
    ))
    .filter(|_| !old_state.is_empty())
}

/// Rewrite the Markdown frontmatter / inline task marker `state` on
/// one heading line: `- [ ]` ↔ `- [x]`, or a leading `STATE ` prefix.
pub fn markdown_heading_state_patch(
    heading_line_no: usize,
    heading_line: &str,
    done: bool,
) -> Option<TextPatch> {
    let trimmed = heading_line.trim_start();
    let indent_len = heading_line.len() - trimmed.len();
    let indent = &heading_line[..indent_len];
    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        if done {
            return Some(TextPatch::replace_line(
                heading_line_no,
                heading_line.to_string(),
                format!("{indent}- [x] {rest}"),
            ));
        }
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("- [x] ") {
        if !done {
            return Some(TextPatch::replace_line(
                heading_line_no,
                heading_line.to_string(),
                format!("{indent}- [ ] {rest}"),
            ));
        }
        return None;
    }
    None
}

/// Locate the 1-based line number of the heading whose text contains
/// `title_needle`, starting the scan at `after_line` (inclusive).
/// Returns `(line_no, line_text)` on the first match.
pub fn find_heading_line(
    lines: &[String],
    title_needle: &str,
    after_line: usize,
) -> Option<(usize, String)> {
    for (idx, line) in lines.iter().enumerate().skip(after_line.saturating_sub(1)) {
        let t = line.trim_start();
        let is_org_heading = t.starts_with('*') && t.as_bytes().get(1) != Some(&b'*');
        let is_md_heading = t.starts_with('#');
        if (is_org_heading || is_md_heading) && line.contains(title_needle) {
            return Some((idx + 1, line.clone()));
        }
    }
    None
}
