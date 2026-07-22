use domain::community::Community;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SpaceCommunitiesConfig {
    pub communities: Vec<Community>,
}

impl SpaceCommunitiesConfig {
    pub fn load(space_root: &Path) -> Result<Self, io::Error> {
        let path = space_root.join(".notez/communities.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)?;
        let cfg = serde_json::from_str(&content).unwrap_or_default();
        Ok(cfg)
    }

    pub fn save(&self, space_root: &Path) -> Result<(), io::Error> {
        let dot_notez = space_root.join(".notez");
        if !dot_notez.exists() {
            fs::create_dir_all(&dot_notez)?;
        }
        let path = dot_notez.join("communities.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
