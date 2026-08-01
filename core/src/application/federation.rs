use serde::{Deserialize, Serialize};
use crate::source::SourceConfig;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpaceSourcesConfig {
    pub sources: Vec<SourceConfig>,
}

impl SpaceSourcesConfig {
    pub fn load(space_root: &Path) -> Result<Self, std::io::Error> {
        let config_path = space_root.join(".notez/sources.json");
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(config_path)?;
        let cfg = serde_json::from_str(&content).unwrap_or_default();
        Ok(cfg)
    }

    pub fn save(&self, space_root: &Path) -> Result<(), std::io::Error> {
        let dot_notez = space_root.join(".notez");
        if !dot_notez.exists() {
            fs::create_dir_all(&dot_notez)?;
        }
        let config_path = dot_notez.join("sources.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }
}
