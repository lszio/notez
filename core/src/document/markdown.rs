use crate::document::ScannedDocument;
use crate::domain::{
    LinkOccurrence, LinkTarget, Resource, ResourceKind, ResourceRef, ResourceRelation, TextSpan,
    derived_id, derived_object_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentError {
    #[error("I/O error reading {path}: {kind}")]
    Io {
        path: String,
        #[serde(with = "crate::error_serde::io_kind")]
        kind: std::io::ErrorKind,
    },
    #[error("malformed ID at {path}:{line}:{column}: {message}")]
    MalformedId {
        path: String,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("{0}")]
    Other(String),
}

pub struct MarkdownScanner;

impl MarkdownScanner {
    /// Parse Markdown bytes from in-memory payload without reading from disk.
    /// The `locator` is propagated for diagnostic messages and stored on the
    /// emitted resources; it is **not** opened.
    pub fn parse_bytes(
        bytes: &[u8],
        source_id: &str,
        locator: &str,
    ) -> Result<ScannedDocument, DocumentError> {
        let content_str = std::str::from_utf8(bytes)
            .map_err(|e| DocumentError::Other(format!("invalid UTF-8 in {locator}: {e}")))?;
        Self::parse_str(content_str, source_id, locator)
    }

    /// Same as [`parse_bytes`] but accepts an already-decoded `&str` body.
    pub fn parse_str(
        content_str: &str,
        source_id: &str,
        locator: &str,
    ) -> Result<ScannedDocument, DocumentError> {
        let path_str = locator.to_string();

        let mut hasher = Sha256::new();
        hasher.update(content_str.as_bytes());
        let revision = format!("{:x}", hasher.finalize());

        let raw: Arc<str> = Arc::from(content_str);

        let mut doc_title = Path::new(locator)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut doc_id_opt: Option<ResourceRef> = None;
        let mut doc_properties = BTreeMap::new();

        let lines: Vec<&str> = raw.lines().collect();
        let mut line_idx = 0;

        // Parse YAML Frontmatter
        if lines.first().is_some_and(|l| l.trim() == "---") {
            line_idx += 1;
            while line_idx < lines.len() {
                let line = lines[line_idx].trim();
                if line == "---" {
                    line_idx += 1;
                    break;
                }
                if let Some((key, val)) = line.split_once(':') {
                    let k = key.trim();
                    let v = val.trim();
                    let k_lower = k.to_lowercase();

                    if k_lower == "title" {
                        doc_title = v.to_string();
                    } else if k_lower == "id" {
                        let full_ref = if v.contains(':') {
                            v.to_string()
                        } else {
                            format!("document:{v}")
                        };
                        if let Ok(r_ref) = ResourceRef::parse(&full_ref) {
                            doc_id_opt = Some(r_ref);
                        }
                    }
                    doc_properties.insert(k.to_string(), v.to_string());
                }
                line_idx += 1;
            }
        }

        let doc_ref =
            doc_id_opt.unwrap_or_else(|| derived_id(ResourceKind::Document, source_id, &path_str, ""));

        let doc_resource = Resource {
            r#ref: doc_ref,
            kind: ResourceKind::Document,
            title: doc_title,
            revision: revision.clone(),
            source_id: source_id.to_string(),
            locator: path_str.clone(),
            properties: doc_properties,
            object_id: derived_object_id("", &path_str, ""),
        };

        let mut resources = vec![doc_resource];
        let mut links = Vec::new();
        let mut link_occurrences = Vec::new();
        let mut current_source_ref = doc_ref;
        let mut heading_count: usize = 0;

        while line_idx < lines.len() {
            let line = lines[line_idx];
            let trimmed = line.trim();

            if trimmed.starts_with('#') && trimmed.contains(' ') {
                let hashes = trimmed.chars().take_while(|c| *c == '#').count();
                if hashes > 0 && trimmed[hashes..].starts_with(' ') {
                    let mut heading_text = trimmed[hashes..].trim().to_string();
                    let mut heading_id_opt = None;

                    if let Some(comment_start) = heading_text.find("<!--") {
                        let comment_part = &heading_text[comment_start..];
                        if let Some(comment_end) = comment_part.find("-->") {
                            let comment_body = &comment_part[4..comment_end].trim();
                            if let Some((id_k, id_v)) = comment_body.split_once(':')
                                && id_k.trim().eq_ignore_ascii_case("id")
                            {
                                let id_val = id_v.trim();
                                let full_ref = if id_val.contains(':') {
                                    id_val.to_string()
                                } else {
                                    format!("heading:{id_val}")
                                };
                                if let Ok(r_ref) = ResourceRef::parse(&full_ref) {
                                    heading_id_opt = Some(r_ref);
                                }
                            }
                        }
                        heading_text = heading_text[..comment_start].trim().to_string();
                    }

                    let h_ref = heading_id_opt.unwrap_or_else(|| {
                        derived_id(
                            ResourceKind::Heading,
                            source_id,
                            &path_str,
                            &heading_count.to_string(),
                        )
                    });
                    heading_count += 1;
                    current_source_ref = h_ref;

                    let mut props = BTreeMap::new();
                    props.insert("LEVEL".to_string(), hashes.to_string());

                    resources.push(Resource {
                        r#ref: h_ref,
                        kind: ResourceKind::Heading,
                        title: heading_text,
                        revision: revision.clone(),
                        source_id: source_id.to_string(),
                        locator: path_str.clone(),
                        properties: props,
                        object_id: derived_object_id("", &path_str, &heading_count.to_string()),
                    });
                }
            }
            if !trimmed.starts_with('#')
                && let Some(comment_start) = trimmed.find("<!--")
                && let Some(comment_end) = trimmed[comment_start..].find("-->")
            {
                let comment_body = trimmed[comment_start + 4..comment_start + comment_end].trim();
                if let Some((id_k, id_v)) = comment_body.split_once(':')
                    && id_k.trim().eq_ignore_ascii_case("id")
                {
                    let id_val = id_v.trim();
                    let full_ref = if id_val.contains(':') {
                        id_val.to_string()
                    } else {
                        format!("block:{id_val}")
                    };
                    if let Ok(b_ref) = ResourceRef::parse(&full_ref) {
                        resources.push(Resource {
                            r#ref: b_ref,
                            kind: ResourceKind::Block,
                            title: trimmed[..comment_start].trim().to_string(),
                            revision: revision.clone(),
                            source_id: source_id.to_string(),
                            locator: path_str.clone(),
                            properties: BTreeMap::new(),
                            object_id: derived_object_id("", &path_str, &b_ref.to_string()),
                        });
                        current_source_ref = b_ref;
                    }
                }
            }

            // Extract all links from this line
            let extracted = extract_markdown_links(line, line_idx + 1, current_source_ref);
            for occ in extracted {
                if let Some(target_ref) = occ.target.as_resource_ref() {
                    links.push(ResourceRelation {
                        source_ref: current_source_ref,
                        relation: "id_link".to_string(),
                        target_ref,
                        relation_type: crate::domain::RelationType::References,
                        direction: crate::domain::RelationDirection::Unknown,
                        evidence_json: serde_json::json!({}),
                        created_at: String::new(),
                        creator: "scan".to_string(),
                    });
                }
                link_occurrences.push(occ);
            }

            line_idx += 1;
        }

        Ok(ScannedDocument {
            raw,
            resources,
            links,
            link_occurrences,
        })
    }

    /// Read a Markdown file from disk and parse it. Kept for callers that
    /// still have a `Path` available; the canonical path is [`parse_bytes`].
    pub fn scan(path: &Path, source_id: &str) -> Result<ScannedDocument, DocumentError> {
        let path_str = path.to_string_lossy().to_string();
        let bytes = std::fs::read(path).map_err(|e| DocumentError::Io {
            path: path_str.clone(),
            kind: e.kind(),
        })?;
        Self::parse_bytes(&bytes, source_id, &path_str)
    }
}

/// Extract all Markdown links from a line.
///
/// Supported forms:
/// - `[[wikilink]]` → `LinkTarget::Title` (bare wikilink)
/// - `[[id:VALUE]]` or `[[id:VALUE][desc]]` → `LinkTarget::Id`
/// - `[label](url)` → `LinkTarget::Url` or `LinkTarget::File`
/// - `[label](path#fragment)` → `LinkTarget::File` with fragment
fn extract_markdown_links(
    line: &str,
    line_num: usize,
    source_ref: ResourceRef,
) -> Vec<LinkOccurrence> {
    let mut results = Vec::new();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            // Wikilink: [[...]] or [[...][...]]
            let start = i;
            i += 2;
            let target_start = i;
            // Find ]] or ][
            let mut target_end = None;
            let mut desc_range = None;
            while i + 1 < len {
                if bytes[i] == b']' && bytes[i + 1] == b']' {
                    target_end = Some(i);
                    i += 2;
                    break;
                } else if bytes[i] == b']' && bytes[i + 1] == b'[' {
                    target_end = Some(i);
                    i += 2;
                    let desc_start = i;
                    while i + 1 < len {
                        if bytes[i] == b']' && bytes[i + 1] == b']' {
                            desc_range = Some((desc_start, i));
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    break;
                }
                i += 1;
            }

            if let Some(te) = target_end {
                let target_str = &line[target_start..te];
                let display_text = desc_range.map(|(ds, de)| line[ds..de].to_string());
                let raw_str = &line[start..i];

                let link_target = parse_markdown_link_target(target_str);

                results.push(LinkOccurrence {
                    source_ref,
                    target: link_target,
                    raw: raw_str.to_string(),
                    display_text,
                    span: TextSpan {
                        line: line_num,
                        col_start: start + 1,
                        col_end: i + 1,
                    },
                });
            }
        } else if bytes[i] == b'[' && (i == 0 || bytes[i - 1] != b'[') {
            // Standard Markdown link: [label](target)
            let start = i;
            i += 1;
            let label_start = i;
            let mut depth = 1;
            while i < len && depth > 0 {
                if bytes[i] == b'[' {
                    depth += 1;
                } else if bytes[i] == b']' {
                    depth -= 1;
                }
                if depth > 0 {
                    i += 1;
                }
            }
            if depth == 0 {
                let label_end = i;
                i += 1; // skip ]
                if i < len && bytes[i] == b'(' {
                    i += 1;
                    let url_start = i;
                    let mut paren_depth = 1;
                    while i < len && paren_depth > 0 {
                        if bytes[i] == b'(' {
                            paren_depth += 1;
                        } else if bytes[i] == b')' {
                            paren_depth -= 1;
                        }
                        if paren_depth > 0 {
                            i += 1;
                        }
                    }
                    if paren_depth == 0 {
                        let url_end = i;
                        i += 1; // skip )
                        let label = &line[label_start..label_end];
                        let url = &line[url_start..url_end];
                        let raw_str = &line[start..i];

                        // Skip images: ![alt](url)
                        if start > 0 && bytes[start - 1] == b'!' {
                            continue;
                        }

                        let link_target = parse_inline_link_target(url);

                        results.push(LinkOccurrence {
                            source_ref,
                            target: link_target,
                            raw: raw_str.to_string(),
                            display_text: if label.is_empty() {
                                None
                            } else {
                                Some(label.to_string())
                            },
                            span: TextSpan {
                                line: line_num,
                                col_start: start + 1,
                                col_end: i + 1,
                            },
                        });
                    } else {
                        continue;
                    }
                } else {
                    // Just a [label] without (url), skip
                    continue;
                }
            } else {
                continue;
            }
        } else {
            i += 1;
        }
    }

    results
}

/// Parse a wikilink target: `id:VALUE`, `Some Title`, `file:path`, etc.
fn parse_markdown_link_target(target: &str) -> LinkTarget {
    let trimmed = target.trim();

    if let Some(id_part) = trimmed.strip_prefix("id:") {
        let id_part = id_part.trim();
        if id_part.contains(':') {
            if let Some((kind_str, ulid_part)) = id_part.split_once(':') {
                let kind_hint = match kind_str {
                    "document" => Some(ResourceKind::Document),
                    "heading" => Some(ResourceKind::Heading),
                    "block" => Some(ResourceKind::Block),
                    "attachment" => Some(ResourceKind::Attachment),
                    _ => None,
                };
                LinkTarget::id(ulid_part, kind_hint)
            } else {
                LinkTarget::id(id_part, None)
            }
        } else {
            LinkTarget::id(id_part, None)
        }
    } else if let Some(file_part) = trimmed.strip_prefix("file:") {
        if let Some((path, fragment)) = file_part.split_once("::") {
            LinkTarget::file(path, Some(fragment.to_string()))
        } else {
            LinkTarget::file(file_part, None)
        }
    } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        LinkTarget::url(trimmed)
    } else if let Some((scheme, value)) = trimmed.split_once(':') {
        // Check if it's a known scheme
        if scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && !scheme.is_empty()
        {
            LinkTarget::custom(scheme, value, None)
        } else {
            LinkTarget::title(trimmed, None)
        }
    } else {
        // Bare title wikilink
        if let Some((title, fragment)) = trimmed.split_once('#') {
            LinkTarget::title(title, Some(fragment.to_string()))
        } else {
            LinkTarget::title(trimmed, None)
        }
    }
}

/// Parse a standard Markdown inline link target: `url`, `path`, `path#fragment`.
fn parse_inline_link_target(url: &str) -> LinkTarget {
    let trimmed = url.trim();

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        LinkTarget::url(trimmed)
    } else if trimmed.starts_with("mailto:") {
        LinkTarget::url(trimmed)
    } else if let Some((scheme, value)) = trimmed.split_once(':') {
        if scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            && !scheme.is_empty()
            && scheme != "C"  // Avoid matching Windows paths like C:\...
        {
            LinkTarget::custom(scheme, value, None)
        } else {
            parse_file_or_title(trimmed)
        }
    } else {
        parse_file_or_title(trimmed)
    }
}

fn parse_file_or_title(target: &str) -> LinkTarget {
    // If it looks like a path (contains . or /), treat as file
    if target.contains('/') || target.contains('.') {
        if let Some((path, fragment)) = target.split_once('#') {
            LinkTarget::file(path, Some(fragment.to_string()))
        } else {
            LinkTarget::file(target, None)
        }
    } else if let Some((title, fragment)) = target.split_once('#') {
        LinkTarget::title(title, Some(fragment.to_string()))
    } else {
        LinkTarget::title(target, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wikilink_title() {
        let t = parse_markdown_link_target("Some Note");
        assert!(matches!(t, LinkTarget::Title { .. }));
    }

    #[test]
    fn parse_wikilink_id() {
        let t = parse_markdown_link_target("id:01J00000000000000000000001");
        assert!(matches!(t, LinkTarget::Id { .. }));
    }

    #[test]
    fn parse_inline_url() {
        let t = parse_inline_link_target("https://example.com");
        assert!(matches!(t, LinkTarget::Url { .. }));
    }

    #[test]
    fn parse_inline_file() {
        let t = parse_inline_link_target("notes/todo.md#section");
        assert!(matches!(t, LinkTarget::File { .. }));
        if let LinkTarget::File { path, fragment } = &t {
            assert_eq!(path, "notes/todo.md");
            assert_eq!(fragment.as_deref(), Some("section"));
        }
    }

    #[test]
    fn extract_mixed_links() {
        let ref_id = ResourceRef::parse("document:01J00000000000000000000001").unwrap();
        let line =
            "See [[Obsidian Vault]] and [docs](https://example.com) and [[id:01J00000000000000000000002][target]].";
        let occs = extract_markdown_links(line, 1, ref_id);
        assert_eq!(occs.len(), 3);
        assert!(matches!(occs[0].target, LinkTarget::Title { .. }));
        assert!(matches!(occs[1].target, LinkTarget::Url { .. }));
        assert!(matches!(occs[2].target, LinkTarget::Id { .. }));
    }
}
