use crate::document::{MarkdownScanner, OrgScanner};
use crate::source::adapter::{ScannedSource, SourceAdapter, SourceConfig, SourceError};
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct GitSourceAdapter {
    config: SourceConfig,
}

impl GitSourceAdapter {
    pub fn new(config: SourceConfig) -> Self {
        Self { config }
    }
}

impl SourceAdapter for GitSourceAdapter {
    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        let mut entries: Vec<PathBuf> = Vec::new();
        for entry in WalkDir::new(&self.config.path)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            let rel_path = path.strip_prefix(&self.config.path).unwrap_or(path);
            if rel_path.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s == ".git" || (s.starts_with('.') && s != ".")
            }) {
                continue;
            }
            if path.is_file()
                && let Some(ext) = path.extension().and_then(|e| e.to_str())
                && (ext == "org" || ext == "md")
            {
                entries.push(path.to_path_buf());
            }
        }

        entries.sort();

        let mut resources = Vec::new();
        let mut relations = Vec::new();
        let mut link_occurrences = Vec::new();
        for path in entries {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            // M4 v1: inline shim around the pure parser. The adapter
            // crate owns the public `parse_path` for third-party
            // callers; core transports stay self-contained to avoid
            // a `notez-core -> adapter -> notez-core` cycle.
            let doc = if ext == "org" {
                let bytes = std::fs::read(&path).map_err(SourceError::Io)?;
                OrgScanner::parse_bytes(&bytes, &self.config.id, &path.to_string_lossy())
                    .map_err(SourceError::Document)?
            } else if ext == "md" {
                let bytes = std::fs::read(&path).map_err(SourceError::Io)?;
                MarkdownScanner::parse_bytes(&bytes, &self.config.id, &path.to_string_lossy())
                    .map_err(SourceError::MarkdownDocument)?
            } else {
                continue;
            };
            resources.extend(doc.resources);
            relations.extend(doc.links);
            link_occurrences.extend(doc.link_occurrences);
        }
        Ok(ScannedSource {
            source_id: self.config.id.clone(),
            resources,
            relations,
            link_occurrences,
        })
    }
}
