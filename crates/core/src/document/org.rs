use crate::document::{ScannedDocument, content_hash_of_bytes};
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
use ulid::Ulid;
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

pub struct OrgScanner;

const TODO_KEYWORDS: &[&str] = &["TODO", "NEXT", "PEND", "WAIT", "DONE", "QUIT"];

impl OrgScanner {
    pub fn parse_bytes(
        bytes: &[u8],
        source_id: &str,
        locator: &str,
    ) -> Result<ScannedDocument, DocumentError> {
        let content_str = std::str::from_utf8(bytes)
            .map_err(|e| DocumentError::Other(format!("invalid UTF-8 in {locator}: {e}")))?;
        Self::parse_str(content_str, source_id, locator)
    }

    pub fn parse_str(
        content_str: &str,
        source_id: &str,
        locator: &str,
    ) -> Result<ScannedDocument, DocumentError> {
        let path_str = locator.to_string();

        let mut hasher = Sha256::new();
        hasher.update(content_str.as_bytes());
        let revision = format!("{:x}", hasher.finalize());

        let mut doc_title = Path::new(locator)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut doc_id_opt: Option<(ResourceRef, usize, usize)> = None;
        let mut doc_properties = BTreeMap::new();
        let raw: Arc<str> = Arc::from(content_str);
        struct PendingHeading {
            level: usize,
            title: String,
            properties: BTreeMap<String, String>,
            id_opt: Option<(ResourceRef, usize, usize)>,
            /// 0-based index among headings, for deterministic ID derivation
            index: usize,
        }
        let mut headings: Vec<PendingHeading> = Vec::new();

        let lines: Vec<&str> = raw.lines().collect();
        let mut line_idx = 0;
        let mut heading_count = 0;

        while line_idx < lines.len() {
            let line = lines[line_idx];
            let trimmed = line.trim();

            if trimmed.starts_with("#+") {
                if let Some((key, val)) = parse_keyword_line(trimmed) {
                    let key_upper = key.to_uppercase();
                    if key_upper == "TITLE" {
                        doc_title = val.to_string();
                    } else if key_upper == "ID" {
                        let col = line.find(val).unwrap_or(0) + 1;
                        let r_ref = parse_id_val(
                            val,
                            ResourceKind::Document,
                            &path_str,
                            line_idx + 1,
                            col,
                        )?;
                        doc_id_opt = Some((r_ref, line_idx + 1, col));
                    } else if key_upper == "NAME" {
                        let block_ref = ResourceRef::new(ResourceKind::Block, Ulid::new());
                        headings.push(PendingHeading {
                            level: 99,
                            title: val.to_string(),
                            properties: BTreeMap::new(),
                            id_opt: Some((block_ref, line_idx + 1, 1)),
                            index: heading_count,
                        });
                        heading_count += 1;
                    }
                    doc_properties.insert(key_upper, val.to_string());
                }
            } else if trimmed.starts_with('*') && trimmed.contains(' ') {
                let stars = trimmed.chars().take_while(|c| *c == '*').count();
                if stars > 0 && trimmed[stars..].starts_with(' ') {
                    let heading_text = trimmed[stars..].trim();
                    let (todo, title) = parse_heading_title(heading_text);

                    let mut properties = BTreeMap::new();
                    properties.insert("LEVEL".to_string(), stars.to_string());
                    if let Some(t) = &todo {
                        properties.insert("TODO".to_string(), t.clone());
                    }

                    headings.push(PendingHeading {
                        level: stars,
                        title,
                        properties,
                        id_opt: None,
                        index: heading_count,
                    });
                    heading_count += 1;
                }
            } else if trimmed.starts_with("SCHEDULED:")
                || trimmed.starts_with("DEADLINE:")
                || trimmed.starts_with("CLOSED:")
            {
                if let Some(cur_heading) = headings.last_mut() {
                    if let Some((key, val)) = trimmed.split_once(':') {
                        cur_heading
                            .properties
                            .insert(key.trim().to_uppercase(), val.trim().to_string());
                    }
                }
            } else if trimmed.eq_ignore_ascii_case(":PROPERTIES:") {
                line_idx += 1;
                while line_idx < lines.len() {
                    let drawer_line = lines[line_idx].trim();
                    if drawer_line.eq_ignore_ascii_case(":END:") {
                        break;
                    }
                    if drawer_line.starts_with(':')
                        && let Some((prop_key, prop_val)) = parse_property_line(drawer_line)
                    {
                        let key_upper = prop_key.to_uppercase();
                        if let Some(cur_heading) = headings.last_mut() {
                            if key_upper == "ID" {
                                let col = lines[line_idx].find(prop_val).unwrap_or(0) + 1;
                                let r_ref = parse_id_val(
                                    prop_val,
                                    ResourceKind::Heading,
                                    &path_str,
                                    line_idx + 1,
                                    col,
                                )?;
                                cur_heading.id_opt = Some((r_ref, line_idx + 1, col));
                            } else {
                                cur_heading
                                    .properties
                                    .insert(key_upper, prop_val.to_string());
                            }
                        } else if key_upper == "ID" {
                            let col = lines[line_idx].find(prop_val).unwrap_or(0) + 1;
                            let r_ref = parse_id_val(
                                prop_val,
                                ResourceKind::Document,
                                &path_str,
                                line_idx + 1,
                                col,
                            )?;
                            doc_id_opt = Some((r_ref, line_idx + 1, col));
                        } else {
                            doc_properties.insert(key_upper, prop_val.to_string());
                        }
                    }
                    line_idx += 1;
                }
            }
            line_idx += 1;
        }

        // Normalize locator: use source-relative path if possible
        let locator = path_str.clone();

        let doc_ref = match doc_id_opt {
            Some((r, _, _)) => r,
            None => derived_id(ResourceKind::Document, source_id, &locator, ""),
        };

        // Document content_hash covers the full file body (truncated to
        // 64 KiB + size mix by content_hash_of_bytes — see spec §3.1).
        let doc_object_id =
            derived_object_id(&content_hash_of_bytes(content_str.as_bytes()), &locator, "");

        let doc_resource = Resource {
            r#ref: doc_ref,
            kind: ResourceKind::Document,
            title: doc_title,
            revision: revision.clone(),
            source_id: source_id.to_string(),
            locator: locator.clone(),
            properties: doc_properties,
            object_id: doc_object_id,
            primary_source_id: String::new(),
        };

        let mut resources = vec![doc_resource];

        // Process headings stack to set parent ref
        let mut heading_refs = Vec::new();
        let mut level_stack: Vec<(usize, ResourceRef)> = Vec::new();

        for h in &headings {
            let h_ref = match h.id_opt {
                Some((r, _, _)) => r,
                None => derived_id(
                    ResourceKind::Heading,
                    source_id,
                    &locator,
                    &h.index.to_string(),
                ),
            };

            while let Some((lvl, _)) = level_stack.last() {
                if *lvl >= h.level {
                    level_stack.pop();
                } else {
                    break;
                }
            }

            let mut props = h.properties.clone();
            if let Some((_, parent_ref)) = level_stack.last() {
                props.insert("PARENT_REF".to_string(), parent_ref.to_string());
            }

            level_stack.push((h.level, h_ref));
            heading_refs.push(h_ref);

            let heading_hash = content_hash_of_bytes(h.title.as_bytes());
            resources.push(Resource {
                primary_source_id: String::new(),
                r#ref: h_ref,
                kind: h_ref.kind(),
                title: h.title.clone(),
                revision: revision.clone(),
                source_id: source_id.to_string(),
                locator: locator.clone(),
                properties: props,
                object_id: derived_object_id(&heading_hash, &locator, &format!("h:{}", h.index)),
            });
        }

        // Parse links — extract all forms
        let mut links = Vec::new();
        let mut link_occurrences = Vec::new();
        let mut current_source_ref = doc_ref;
        let mut heading_idx = 0;

        for (line_num, line) in raw.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('*') && trimmed.contains(' ') {
                let stars = trimmed.chars().take_while(|c| *c == '*').count();
                if stars > 0
                    && trimmed[stars..].starts_with(' ')
                    && heading_idx + 1 < resources.len()
                {
                    heading_idx += 1;
                    current_source_ref = resources[heading_idx].r#ref;
                }
            }

            let extracted = extract_org_links(line, line_num + 1, current_source_ref);
            for occ in extracted {
                // Backward compat: if this is an ID link, produce old-style ResourceRelation
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
        }

        Ok(ScannedDocument {
            raw,
            resources,
            links,
            link_occurrences,
        })
    }
}
fn parse_keyword_line(line: &str) -> Option<(&str, &str)> {
    let s = line.strip_prefix("#+")?;
    let (key, val) = s.split_once(':')?;
    Some((key.trim(), val.trim()))
}

fn parse_property_line(line: &str) -> Option<(&str, &str)> {
    let s = line.strip_prefix(':')?;
    let (key, val) = s.split_once(':')?;
    Some((key.trim(), val.trim()))
}

fn parse_heading_title(text: &str) -> (Option<String>, String) {
    let mut parts = text.split_whitespace();
    if let Some(first) = parts.next()
        && TODO_KEYWORDS.contains(&first)
    {
        let rest = text[first.len()..].trim().to_string();
        return (Some(first.to_string()), rest);
    }
    (None, text.to_string())
}

fn parse_id_val(
    val: &str,
    expected_kind: ResourceKind,
    path: &str,
    line: usize,
    column: usize,
) -> Result<ResourceRef, DocumentError> {
    let s = val.trim();
    if s.contains(':') {
        ResourceRef::parse(s).map_err(|e| DocumentError::MalformedId {
            path: path.to_string(),
            line,
            column,
            message: e.to_string(),
        })
    } else {
        let full = format!("{}:{}", expected_kind.as_str(), s);
        ResourceRef::parse(&full).map_err(|e| DocumentError::MalformedId {
            path: path.to_string(),
            line,
            column,
            message: e.to_string(),
        })
    }
}

/// Extract all Org-mode links from a line, returning `LinkOccurrence` for each.
///
/// Supported forms:
/// - `[[id:VALUE]]` or `[[id:VALUE][DESCRIPTION]]` → `LinkTarget::Id`
/// - `[[file:PATH]]` or `[[file:PATH::HEADING]]` → `LinkTarget::File`
/// - `[[file:PATH][DESC]]` → `LinkTarget::File`
/// - `[[https://URL]]` or `[[http://URL][DESC]]` → `LinkTarget::Url`
/// - `[[SCHEME:VALUE]]` → `LinkTarget::Custom` for unknown schemes
/// - `[[TITLE]]` → `LinkTarget::Title` (bare wikilink without scheme)
fn extract_org_links(line: &str, line_num: usize, source_ref: ResourceRef) -> Vec<LinkOccurrence> {
    let mut results = Vec::new();
    let mut rest = line;
    let mut offset = 0;

    while let Some(start) = rest.find("[[") {
        let col_start = offset + start;
        let after = &rest[start + 2..];

        // Find the matching ]] — may have ][desc] in between
        let end = if let Some(desc_start) = after.find("][") {
            let after_desc = &after[desc_start + 2..];
            if let Some(close) = after_desc.find("]]") {
                // [[target][description]]
                desc_start + 2 + close + 2
            } else if let Some(close) = after.find("]]") {
                close + 2
            } else {
                break;
            }
        } else if let Some(close) = after.find("]]") {
            close + 2
        } else {
            break;
        };

        let full_raw = &rest[start..start + 2 + end];
        let col_end = col_start + full_raw.len();

        // Parse inner: [[target][description]] or [[target]]
        let inner = &after[..end - 2]; // strip trailing ]]
        let (target_part, display_text) = if let Some(desc_pos) = inner.find("][") {
            (&inner[..desc_pos], Some(inner[desc_pos + 2..].to_string()))
        } else {
            (inner, None)
        };

        let link_target = parse_org_link_target(target_part);

        results.push(LinkOccurrence {
            source_ref,
            target: link_target,
            raw: full_raw.to_string(),
            display_text,
            span: TextSpan {
                line: line_num,
                col_start: col_start + 1, // 1-indexed
                col_end: col_end + 1,
            },
        });

        rest = &rest[start + 2 + end..];
        offset = col_end;
    }

    results
}

/// Parse the target part of an Org link (inside `[[...]]` before any `][`).
fn parse_org_link_target(target: &str) -> LinkTarget {
    // Check for scheme:value pattern
    if let Some((scheme, value)) = target.split_once(':') {
        let scheme_lower = scheme.to_lowercase();
        match scheme_lower.as_str() {
            "id" => {
                let value = value.trim();
                // Try to determine kind hint
                if value.contains(':') {
                    // Already has kind:ulid format
                    if let Some((kind_str, ulid_part)) = value.split_once(':') {
                        let kind_hint = match kind_str {
                            "document" => Some(ResourceKind::Document),
                            "heading" => Some(ResourceKind::Heading),
                            "block" => Some(ResourceKind::Block),
                            "attachment" => Some(ResourceKind::Attachment),
                            _ => None,
                        };
                        LinkTarget::id(ulid_part, kind_hint)
                    } else {
                        LinkTarget::id(value, None)
                    }
                } else {
                    LinkTarget::id(value, None)
                }
            }
            "file" => {
                // file:path or file:path::heading
                if let Some((path, fragment)) = value.split_once("::") {
                    LinkTarget::file(path, Some(fragment.to_string()))
                } else {
                    LinkTarget::file(value, None)
                }
            }
            "http" | "https" => LinkTarget::url(target),
            _ => {
                // Custom scheme
                if let Some((val, fragment)) = value.split_once("::") {
                    LinkTarget::custom(scheme, val, Some(fragment.to_string()))
                } else {
                    LinkTarget::custom(scheme, value, None)
                }
            }
        }
    } else {
        // No scheme — treat as title/wikilink
        LinkTarget::title(target, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_org_id_link() {
        let t = parse_org_link_target("id:01J00000000000000000000001");
        assert!(matches!(t, LinkTarget::Id { .. }));
        if let LinkTarget::Id { value, kind_hint } = &t {
            assert_eq!(value, "01J00000000000000000000001");
            assert_eq!(*kind_hint, None);
        }
    }

    #[test]
    fn parse_org_file_link() {
        let t = parse_org_link_target("file:notes/todo.org::heading");
        assert!(matches!(t, LinkTarget::File { .. }));
        if let LinkTarget::File { path, fragment } = &t {
            assert_eq!(path, "notes/todo.org");
            assert_eq!(fragment.as_deref(), Some("heading"));
        }
    }

    #[test]
    fn parse_org_url_link() {
        let t = parse_org_link_target("https://example.com/page");
        assert!(matches!(t, LinkTarget::Url { .. }));
    }

    #[test]
    fn parse_org_title_link() {
        let t = parse_org_link_target("Some Heading");
        assert!(matches!(t, LinkTarget::Title { .. }));
        if let LinkTarget::Title { title, .. } = &t {
            assert_eq!(title, "Some Heading");
        }
    }

    #[test]
    fn parse_org_custom_link() {
        let t = parse_org_link_target("zotero:key123");
        assert!(matches!(t, LinkTarget::Custom { .. }));
    }

    #[test]
    fn extract_multiple_links() {
        let ref_id = ResourceRef::parse("document:01J00000000000000000000001").unwrap();
        let line = "See [[id:01J00000000000000000000002][target]] and [[file:notes.org]].";
        let occs = extract_org_links(line, 1, ref_id);
        assert_eq!(occs.len(), 2);
        assert!(matches!(occs[0].target, LinkTarget::Id { .. }));
        assert_eq!(occs[0].display_text.as_deref(), Some("target"));
        assert!(matches!(occs[1].target, LinkTarget::File { .. }));
        assert!(occs[1].display_text.is_none());
    }
}
