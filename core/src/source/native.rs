use crate::source::adapter::{ScannedSource, SourceAdapter, SourceConfig, SourceError};
use crate::document::{MarkdownScanner, OrgScanner};
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::source::protocol::{FormatParser, ParsedEntity, RawEntity, SourceTransport, TransportError, ParserError};
use crate::source::adapter::ComposedSourceAdapter;
use std::fs;

pub struct NativeSourceAdapter {
    inner: ComposedSourceAdapter,
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
            for entry in WalkDir::new(include_root).into_iter().filter_map(Result::ok) {
                let path = entry.path();
                if self.exclude_paths.iter().any(|ex| path.starts_with(ex)) {
                    continue;
                }
                let rel_path = path.strip_prefix(include_root).unwrap_or(path);
                if rel_path.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
                    continue;
                }
                if path.is_file() {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
                    let mime_type = if ext == "org" {
                        "text/org"
                    } else if ext == "md" {
                        "text/markdown"
                    } else {
                        continue;
                    };
                    
                    let payload = fs::read(path).map_err(TransportError::Io)?;
                    entities.push(RawEntity {
                        locator: path.to_string_lossy().to_string(),
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
        let path = PathBuf::from(&entity.locator);
        let doc = OrgScanner::scan(&path, source_id)
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
        let path = PathBuf::from(&entity.locator);
        let doc = MarkdownScanner::scan(&path, source_id)
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
        let transport = NativeTransport {
            include_paths: config.include_paths.clone(),
            exclude_paths: config.exclude_paths.clone(),
            base_path: config.path.clone(),
        };
        
        let parsers: Vec<Box<dyn FormatParser>> = vec![
            Box::new(OrgParser),
            Box::new(MarkdownParser),
        ];

        let inner = ComposedSourceAdapter::new(config, Box::new(transport), parsers);
        Self { inner }
    }
}

impl SourceAdapter for NativeSourceAdapter {
    fn config(&self) -> &SourceConfig {
        self.inner.config()
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        self.inner.scan()
    }
}

