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

/// The small, deliberately conservative edit surface supported for Org files.
/// `line` and columns are one-based; `col_end` is exclusive.  The fragment is
/// part of the precondition, so a patch cannot silently target a moved field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgTextEdit {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
    pub expected_fragment: String,
    pub replacement: String,
    pub field: OrgEditField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrgEditField { HeadlineTitle, TodoKeyword, Priority, Property, PlanningDate }

#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrgPatchError {
    #[error("document revision mismatch: expected {expected}, actual {actual}")]
    RevisionConflict { expected: String, actual: String },
    #[error("invalid Org patch: {0}")]
    InvalidPatch(String),
    #[error("unsupported Org edit: {0}")]
    Unsupported(String),
}

pub fn restricted_org_patches(
    content: &str,
    expected_revision: &str,
    edits: &[OrgTextEdit],
) -> Result<Vec<crate::source::writer::TextPatch>, OrgPatchError> {
    let mut patches = Vec::with_capacity(edits.len());
    for edit in edits {
        let patch = restricted_org_patch(content, expected_revision, edit)?;
        if patches.iter().any(|p: &crate::source::writer::TextPatch| {
            p.line_no == patch.line_no && p.line_no_end == patch.line_no_end
        }) {
            return Err(OrgPatchError::InvalidPatch("multiple edits target the same span".into()));
        }
        patches.push(patch);
    }
    Ok(patches)
}

pub fn restricted_org_patch(
    content: &str,
    expected_revision: &str,
    edit: &OrgTextEdit,
) -> Result<crate::source::writer::TextPatch, OrgPatchError> {
    let actual = format!("{:x}", Sha256::digest(content.as_bytes()));
    if actual != expected_revision {
        return Err(OrgPatchError::RevisionConflict { expected: expected_revision.to_string(), actual });
    }
    if edit.line == 0 || edit.col_start == 0 || edit.col_end < edit.col_start {
        return Err(OrgPatchError::InvalidPatch("invalid line or span".into()));
    }
    let lines: Vec<&str> = content.lines().collect();
    let line = lines.get(edit.line - 1).ok_or_else(|| OrgPatchError::InvalidPatch(format!("line {} is out of range", edit.line)))?;
    let start = edit.col_start - 1;
    let end = edit.col_end - 1;
    let fragment = line.get(start..end).ok_or_else(|| OrgPatchError::InvalidPatch("span is not a valid UTF-8 span".into()))?;
    if fragment != edit.expected_fragment { return Err(OrgPatchError::InvalidPatch("expected fragment does not match document".into())); }
    let trimmed = line.trim_start();
    let base = line.len() - trimmed.len();
    let recognized = match edit.field {
        OrgEditField::HeadlineTitle => is_org_heading(trimmed) && title_span(line, start, end),
        OrgEditField::TodoKeyword => todo_span(line, start, end, fragment),
        OrgEditField::Priority => priority_span(line, start, end, fragment),
        OrgEditField::Property => property_span(line, start, end),
        OrgEditField::PlanningDate => planning_span(line, start, end),
    };
    let _ = base;
    if !recognized { return Err(OrgPatchError::Unsupported(format!("{:?} is not recognized at this span", edit.field))); }
    if matches!(edit.field, OrgEditField::TodoKeyword) && !TODO_KEYWORDS.contains(&edit.replacement.as_str()) {
        return Err(OrgPatchError::InvalidPatch("replacement is not a recognized TODO keyword".into()));
    }
    if matches!(edit.field, OrgEditField::Priority) && !(edit.replacement.is_empty() || (edit.replacement.starts_with("[#") && edit.replacement.ends_with(']') && edit.replacement.len() == 4)) {
        return Err(OrgPatchError::InvalidPatch("replacement is not an Org priority".into()));
    }
    let mut replacement = String::with_capacity(line.len() - fragment.len() + edit.replacement.len());
    replacement.push_str(&line[..start]); replacement.push_str(&edit.replacement); replacement.push_str(&line[end..]);
    Ok(crate::source::writer::TextPatch::replace_line(edit.line, (*line).to_string(), replacement))
}

fn is_org_heading(line: &str) -> bool { let n = line.chars().take_while(|c| *c == '*').count(); n > 0 && line.as_bytes().get(n) == Some(&b' ') }
fn title_span(line: &str, start: usize, end: usize) -> bool {
    let t = line.trim_start(); let base = line.len() - t.len();
    let stars = t.chars().take_while(|c| *c == '*').count(); let mut rest = &t[stars + 1..];
    if let Some(k) = TODO_KEYWORDS.iter().find(|k| rest.starts_with(&format!("{k} "))) { rest = &rest[k.len() + 1..]; }
    if rest.starts_with("[#") { if let Some(i) = rest.find(']') { rest = rest[i + 1..].trim_start(); } }
    let title_start = base + line[base..].len() - rest.len(); start >= title_start && start < end
}
fn todo_span(line: &str, start: usize, end: usize, fragment: &str) -> bool { let n = line.chars().take_while(|c| *c == '*').count(); start == n + 1 && end == start + fragment.len() && TODO_KEYWORDS.contains(&fragment) }
fn priority_span(line: &str, start: usize, end: usize, fragment: &str) -> bool { let Some(i) = line.find("[#") else { return false }; start == i && end == i + fragment.len() && fragment.len() == 4 && fragment.ends_with(']') }
fn property_span(line: &str, start: usize, end: usize) -> bool {
    let base = line.len() - line.trim_start().len();
    let t = line.trim_start();
    let Some(rest) = t.strip_prefix(':') else { return false };
    let Some((key, _)) = rest.split_once(':') else { return false };
    ["ID", "CUSTOM_ID", "CATEGORY", "DESCRIPTION", "CREATED", "EFFORT", "OWNER", "ROAM_REFS", "URL"].contains(&key.to_uppercase().as_str()) && start >= base && start < end
}
fn planning_span(line: &str, start: usize, end: usize) -> bool {
    let base = line.len() - line.trim_start().len();
    let t = line.trim_start();
    ["SCHEDULED:", "DEADLINE:", "CLOSED:"].iter().any(|p| t.starts_with(p)) && start >= base + t.find(':').unwrap() + 1 && start < end
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

#[cfg(test)]
mod patch_tests {
    use super::*;
    use crate::source::writer::FsSpanWriter;
    use sha2::{Digest, Sha256};

    fn revision(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }
    #[test]
    fn title_patch_preserves_unknown_bytes() {
        let text = "#+CUSTOM: untouched\n* TODO [#A] Old title\n:UNKNOWN: keep\n";
        let edit = OrgTextEdit { line: 2, col_start: 13, col_end: 22, expected_fragment: "Old title".into(), replacement: "New title".into(), field: OrgEditField::HeadlineTitle };
        let patch = restricted_org_patch(text, &revision(text), &edit).unwrap();
        let out = crate::source::writer::apply_to_lines(text.lines().map(str::to_string).collect(), &[patch]).unwrap().join("\n") + "\n";
        assert_eq!(out, "#+CUSTOM: untouched\n* TODO [#A] New title\n:UNKNOWN: keep\n");
    }

    #[test]
    fn unsupported_and_stale_patches_are_rejected() {
        let text = "* TODO title\nbody\n";
        let unsupported = OrgTextEdit { line: 2, col_start: 1, col_end: 5, expected_fragment: "body".into(), replacement: "x".into(), field: OrgEditField::HeadlineTitle };
        assert!(matches!(restricted_org_patch(text, &revision(text), &unsupported), Err(OrgPatchError::Unsupported(_))));
        let stale = OrgTextEdit { line: 1, col_start: 8, col_end: 13, expected_fragment: "title".into(), replacement: "new".into(), field: OrgEditField::HeadlineTitle };
        assert!(matches!(restricted_org_patch("* TODO changed\nbody\n", &revision(text), &stale), Err(OrgPatchError::RevisionConflict { .. })));
    }

    #[test]
    fn failed_filesystem_patch_leaves_file_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.org");
        let text = "* TODO title\nbody\n";
        std::fs::write(&path, text).unwrap();
        let bad = crate::source::writer::TextPatch::replace_line(1, "* TODO wrong", "* DONE changed");
        assert!(FsSpanWriter::apply(&path, &[bad]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
}
