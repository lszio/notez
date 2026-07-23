use domain::{Resource, ResourceKind, ResourceRef, ResourceRelation};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use ulid::Ulid;

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("malformed ID at {path}:{line}:{column}: {message}")]
    MalformedId {
        path: String,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("document error: {0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub struct ScannedDocument {
    pub raw: Arc<str>,
    pub resources: Vec<Resource>,
    pub links: Vec<ResourceRelation>,
}

pub struct OrgScanner;

const TODO_KEYWORDS: &[&str] = &["TODO", "NEXT", "PEND", "WAIT", "DONE", "QUIT"];

impl OrgScanner {
    pub fn scan(path: &Path, source_id: &str) -> Result<ScannedDocument, DocumentError> {
        let path_str = path.to_string_lossy().to_string();
        let content_bytes = fs::read(path).map_err(|e| DocumentError::Io {
            path: path_str.clone(),
            source: e,
        })?;
        let content_str = String::from_utf8(content_bytes)
            .map_err(|e| DocumentError::Other(format!("invalid UTF-8 in {path_str}: {e}")))?;

        let mut hasher = Sha256::new();
        hasher.update(content_str.as_bytes());
        let revision = format!("{:x}", hasher.finalize());

        let raw: Arc<str> = Arc::from(content_str.as_str());

        let mut doc_title = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut doc_id_opt: Option<(ResourceRef, usize, usize)> = None;
        let mut doc_properties = BTreeMap::new();

        struct PendingHeading {
            level: usize,
            title: String,
            properties: BTreeMap<String, String>,
            id_opt: Option<(ResourceRef, usize, usize)>,
        }
        let mut headings: Vec<PendingHeading> = Vec::new();

        let lines: Vec<&str> = raw.lines().collect();
        let mut line_idx = 0;

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
                        });
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
                    });
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

        let doc_ref = match doc_id_opt {
            Some((r, _, _)) => r,
            None => ResourceRef::new(ResourceKind::Document, Ulid::new()),
        };

        let doc_resource = Resource {
            r#ref: doc_ref,
            kind: ResourceKind::Document,
            title: doc_title,
            revision: revision.clone(),
            source_id: source_id.to_string(),
            locator: path_str.clone(),
            properties: doc_properties,
        };

        let mut resources = vec![doc_resource];

        // Process headings stack to set parent ref
        let mut heading_refs = Vec::new();
        let mut level_stack: Vec<(usize, ResourceRef)> = Vec::new();

        for h in headings {
            let h_ref = match h.id_opt {
                Some((r, _, _)) => r,
                None => ResourceRef::new(ResourceKind::Heading, Ulid::new()),
            };

            while let Some((lvl, _)) = level_stack.last() {
                if *lvl >= h.level {
                    level_stack.pop();
                } else {
                    break;
                }
            }

            let mut props = h.properties;
            if let Some((_, parent_ref)) = level_stack.last() {
                props.insert("PARENT_REF".to_string(), parent_ref.to_string());
            }

            level_stack.push((h.level, h_ref));
            heading_refs.push(h_ref);

            resources.push(Resource {
                r#ref: h_ref,
                kind: h_ref.kind(),
                title: h.title,
                revision: revision.clone(),
                source_id: source_id.to_string(),
                locator: path_str.clone(),
                properties: props,
            });
        }

        // Parse links
        let mut links = Vec::new();
        let mut current_source_ref = doc_ref;
        let mut heading_idx = 0;

        for line in raw.lines() {
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

            for target_ref in extract_id_links(line) {
                links.push(ResourceRelation {
                    source_ref: current_source_ref,
                    relation: "id_link".to_string(),
                    target_ref,
                });
            }
        }

        Ok(ScannedDocument {
            raw,
            resources,
            links,
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

fn extract_id_links(line: &str) -> Vec<ResourceRef> {
    let mut results = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("[[id:") {
        let after = &rest[start + 5..];
        if let Some(end) = after.find(']') {
            let target_str = &after[..end];
            let target_str = target_str.trim();
            let parsed = if target_str.contains(':') {
                ResourceRef::parse(target_str).ok()
            } else {
                ResourceRef::parse(&format!("heading:{target_str}")).ok()
            };
            if let Some(r) = parsed {
                results.push(r);
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    results
}
