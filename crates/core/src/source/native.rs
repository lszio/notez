use crate::document::{MarkdownScanner, OrgScanner};
use crate::source::adapter::{ScannedSource, SourceAdapter, SourceConfig, SourceError};
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::source::adapter::ComposedSourceAdapter;
use crate::source::protocol::{
    FormatParser, ParsedEntity, ParserError, RawEntity, SourceTransport, TransportError,
};
use std::fs;

pub struct NativeSourceAdapter {
    inner: ComposedSourceAdapter,
    base_path: PathBuf,
}

pub struct NativeTransport {
    include_paths: Vec<PathBuf>,
    exclude_paths: Vec<PathBuf>,
    base_path: PathBuf,
}

impl SourceTransport for NativeTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        let include_roots: Vec<PathBuf> = if self.include_paths.is_empty() {
            vec![self.base_path.clone()]
        } else {
            self.include_paths.clone()
        };

        let mut entities = Vec::new();
        for include_root in &include_roots {
            for entry in WalkDir::new(include_root)
                .into_iter()
                .filter_map(Result::ok)
            {
                let path = entry.path();
                if self.exclude_paths.iter().any(|ex| path.starts_with(ex)) {
                    continue;
                }
                let rel_path = path.strip_prefix(include_root).unwrap_or(path);
                if rel_path
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
                {
                    continue;
                }
                if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or_default();
                    let mime_type = if ext == "org" {
                        "text/org"
                    } else if ext == "md" {
                        "text/markdown"
                    } else {
                        continue;
                    };

                    let payload = fs::read(path).map_err(TransportError::Io)?;
                    entities.push(RawEntity {
                        // Use the path relative to the include_root so the
                        // locator stays stable across sources (spec §3.1):
                        // two Spaces pointing at the same physical
                        // directory must agree on a per-file locator
                        // regardless of where the include_root lives on
                        // disk.
                        locator: rel_path.to_string_lossy().to_string(),
                        mime_type: mime_type.to_string(),
                        payload,
                    });
                }
            }
        }

        // Maintain deterministic order
        entities.sort_by(|a, b| a.locator.cmp(&b.locator));
        Ok(entities)
    }
}

pub struct OrgParser;
impl FormatParser for OrgParser {
    fn supports(&self, mime_type: &str) -> bool {
        mime_type == "text/org"
    }

    fn parse(&self, entity: &RawEntity, source_id: &str) -> Result<ParsedEntity, ParserError> {
        // The transport already loaded the bytes; pass them through so
        // the scanner doesn't re-read from disk using the (relative)
        // locator as a path.
        let doc = OrgScanner::parse_bytes(&entity.payload, source_id, &entity.locator)
            .map_err(|e| ParserError::Other(e.to_string()))?;
        Ok(ParsedEntity {
            resources: doc.resources,
            relations: doc.links,
            link_occurrences: doc.link_occurrences,
        })
    }
}

pub struct MarkdownParser;
impl FormatParser for MarkdownParser {
    fn supports(&self, mime_type: &str) -> bool {
        mime_type == "text/markdown"
    }

    fn parse(&self, entity: &RawEntity, source_id: &str) -> Result<ParsedEntity, ParserError> {
        // See OrgParser::parse above.
        let doc = MarkdownScanner::parse_bytes(&entity.payload, source_id, &entity.locator)
            .map_err(|e| ParserError::Other(e.to_string()))?;
        Ok(ParsedEntity {
            resources: doc.resources,
            relations: doc.links,
            link_occurrences: doc.link_occurrences,
        })
    }
}

impl NativeSourceAdapter {
    pub fn new(config: SourceConfig) -> Self {
        let base_path = config.path.clone();
        let transport = NativeTransport {
            include_paths: config.include_paths.clone(),
            exclude_paths: config.exclude_paths.clone(),
            base_path: config.path.clone(),
        };

        let parsers: Vec<Box<dyn FormatParser>> =
            vec![Box::new(OrgParser), Box::new(MarkdownParser)];

        let inner = ComposedSourceAdapter::new(config, Box::new(transport), parsers);
        Self { inner, base_path }
    }
}

impl SourceAdapter for NativeSourceAdapter {
    fn config(&self) -> &SourceConfig {
        self.inner.config()
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        self.inner.scan()
    }

    fn recognizes_path(&self, path: &str) -> bool {
        path.starts_with("notez://object/")
            || path.starts_with("file://")
            || path.ends_with(".md")
            || path.ends_with(".org")
    }

    fn resolve_path(
        &self,
        path: &str,
    ) -> Result<Option<crate::domain::ObjectIdentity>, SourceError> {
        if let Some(rest) = path.strip_prefix("notez://object/") {
            return match crate::domain::ObjectIdentity::parse(rest) {
                Ok(id) if id.authority == "local" => Ok(Some(id)),
                _ => Ok(None),
            };
        }
        let fs_path = if let Some(rest) = path.strip_prefix("file://") {
            std::path::PathBuf::from(rest)
        } else if path.ends_with(".md") || path.ends_with(".org") {
            std::path::PathBuf::from(path)
        } else {
            return Ok(None);
        };
        let content = std::fs::read(&fs_path).map_err(SourceError::Io)?;
        let hash = crate::document::content_hash_of_bytes(&content);
        // Normalize to a source-relative locator when under base_path so the
        // identity matches what `scan` produces; otherwise use the path as-is.
        let locator = fs_path
            .strip_prefix(&self.base_path)
            .map(|rel| rel.to_string_lossy().to_string())
            .unwrap_or_else(|_| fs_path.to_string_lossy().to_string());
        Ok(Some(crate::domain::derived_object_id(&hash, &locator, "")))
    }

    fn render_path(&self, identity: &crate::domain::ObjectIdentity) -> Option<String> {
        if identity.authority == "local" {
            Some(identity.to_notez_uri())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NativeSourceAdapter;
    use crate::source::SourceKind;
    use crate::source::adapter::{SourceAdapter, SourceConfig};

    fn adapter() -> NativeSourceAdapter {
        NativeSourceAdapter::new(SourceConfig::new(
            "native",
            SourceKind::Native,
            std::path::PathBuf::from("/tmp"),
            false,
        ))
    }

    #[test]
    fn resolves_local_object_uri_and_renders_back() {
        let a = adapter();
        let id = a
            .resolve_path("notez://object/local::01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .unwrap()
            .unwrap();
        assert_eq!(id.authority, "local");
        assert_eq!(id.native_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(
            a.render_path(&id).unwrap(),
            "notez://object/local::01ARZ3NDEKTSV4RRFFQ69G5FAV"
        );
    }

    #[test]
    fn rejects_non_local_authority() {
        let a = adapter();
        assert!(
            a.resolve_path("notez://object/notion::x")
                .unwrap()
                .is_none()
        );
        assert!(
            a.render_path(&crate::domain::ObjectIdentity::new("notion", "x"))
                .is_none()
        );
    }

    #[test]
    fn recognizes_own_path_forms() {
        let a = adapter();
        assert!(a.recognizes_path("notez://object/local::x"));
        assert!(a.recognizes_path("notes/foo.md"));
        assert!(a.recognizes_path("file:///tmp/foo.org"));
        assert!(!a.recognizes_path("https://notion.so/x"));
    }
}
