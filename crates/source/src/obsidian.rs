use crate::adapter::{ScannedSource, SourceAdapter, SourceConfig, SourceError};
use document::MarkdownScanner;
use std::path::PathBuf;
use walkdir::WalkDir;

pub struct ObsidianSourceAdapter {
    config: SourceConfig,
}

impl ObsidianSourceAdapter {
    pub fn new(config: SourceConfig) -> Self {
        Self { config }
    }
}

impl SourceAdapter for ObsidianSourceAdapter {
    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        let mut entries: Vec<PathBuf> = Vec::new();
        for entry in WalkDir::new(&self.config.path).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            let rel_path = path.strip_prefix(&self.config.path).unwrap_or(path);
            if rel_path.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
                continue;
            }
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "md" {
                        entries.push(path.to_path_buf());
                    }
                }
            }
        }

        entries.sort();

        let mut resources = Vec::new();
        let mut relations = Vec::new();

        for path in entries {
            let doc = MarkdownScanner::scan(&path, &self.config.id)?;
            resources.extend(doc.resources);
            relations.extend(doc.links);
        }

        Ok(ScannedSource {
            source_id: self.config.id.clone(),
            resources,
            relations,
        })
    }
}
