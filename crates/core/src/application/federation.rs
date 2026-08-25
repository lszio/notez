use crate::config::SourceInstanceConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Per-source persistent cache of registered source instances, written
/// to `<source_root>/.notez/sources.json`. Used by `notez source add/list/remove`
/// to track source bindings without rewriting the parent `notez.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SourceInstancesCache {
    pub sources: Vec<SourceInstanceConfig>,
}

impl SourceInstancesCache {
    pub fn load(source_root: &Path) -> Result<Self, std::io::Error> {
        let config_path = source_root.join(".notez/sources.json");
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(config_path)?;
        let cfg = serde_json::from_str(&content).unwrap_or_default();
        Ok(cfg)
    }

    pub fn save(&self, source_root: &Path) -> Result<(), std::io::Error> {
        let dot_notez = source_root.join(".notez");
        if !dot_notez.exists() {
            fs::create_dir_all(&dot_notez)?;
        }
        let config_path = dot_notez.join("sources.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }
}
