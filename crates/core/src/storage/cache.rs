//! JSON-backed project-local caches.
//!
//! Two small per-source caches (`sources.json`, `communities.json`)
//! were historically read/written directly by `application/` modules.
//! Their filesystem effects belong in the infrastructure layer, so
//! the persistence logic lives here and the application modules hold
//! only typed data.
//!
//! Both functions are silent on malformed input — the legacy behavior
//! a corrupted cache silently disables that cache entry without
//! surfacing a hard error. This is documented as a follow-up under
//! the federation reliability spec but is intentionally not changed
//! here.

use crate::config::SourceInstanceConfig;
use crate::domain::community::Community;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpaceCommunitiesConfig {
    pub communities: Vec<Community>,
}

impl SpaceCommunitiesConfig {
    pub fn load(source_root: &Path) -> Result<Self, io::Error> {
        let path = source_root.join(".notez/communities.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    pub fn save(&self, source_root: &Path) -> Result<(), io::Error> {
        let dot_notez = source_root.join(".notez");
        if !dot_notez.exists() {
            fs::create_dir_all(&dot_notez)?;
        }
        let path = dot_notez.join("communities.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

/// Per-source persistent cache of registered source instances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SourceInstancesCache {
    pub sources: Vec<SourceInstanceConfig>,
}

impl SourceInstancesCache {
    pub fn load(source_root: &Path) -> Result<Self, io::Error> {
        let config_path = source_root.join(".notez/sources.json");
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(config_path)?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    pub fn save(&self, source_root: &Path) -> Result<(), io::Error> {
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