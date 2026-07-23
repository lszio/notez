use crate::org::DocumentError;
use domain::{Resource, ResourceKind, ResourceRef, ResourceRelation};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use ulid::Ulid;

pub struct ScannedDocument {
    pub raw: Arc<str>,
    pub resources: Vec<Resource>,
    pub links: Vec<ResourceRelation>,
}

pub struct MarkdownScanner;

impl MarkdownScanner {
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
            doc_id_opt.unwrap_or_else(|| ResourceRef::new(ResourceKind::Document, Ulid::new()));

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
        let mut links = Vec::new();
        let mut current_source_ref = doc_ref;

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

                    let h_ref = heading_id_opt
                        .unwrap_or_else(|| ResourceRef::new(ResourceKind::Heading, Ulid::new()));
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
                        });
                        current_source_ref = b_ref;
                    }
                }
            }

            for target_ref in extract_markdown_links(line) {
                links.push(ResourceRelation {
                    source_ref: current_source_ref,
                    relation: "id_link".to_string(),
                    target_ref,
                });
            }

            line_idx += 1;
        }

        Ok(ScannedDocument {
            raw,
            resources,
            links,
        })
    }
}

fn extract_markdown_links(line: &str) -> Vec<ResourceRef> {
    let mut results = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let inner = &after[..end];
            let inner_trimmed = inner.trim();

            if let Some(id_part) = inner_trimmed.strip_prefix("id:") {
                let target_str = if let Some((target, _)) = id_part.split_once("][") {
                    target
                } else {
                    id_part
                };
                let target_str = target_str.trim();
                let parsed = if target_str.contains(':') {
                    ResourceRef::parse(target_str).ok()
                } else {
                    ResourceRef::parse(&format!("heading:{target_str}")).ok()
                };
                if let Some(r) = parsed {
                    results.push(r);
                }
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    results
}
