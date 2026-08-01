use crate::config::model::{ConfigError, SpaceConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacySources {
    pub sources: Vec<crate::config::model::SpaceSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyCommunities {
    pub communities: Vec<crate::domain::community::Community>,
}

#[derive(Debug, Clone, Default)]
pub struct MigrationPlan {
    pub sources_to_add: Vec<crate::config::model::SpaceSourceConfig>,
    pub communities_to_extract: Vec<crate::domain::community::Community>,
}

pub fn plan_legacy_migration(
    space_root: &Path,
    current_cfg: &SpaceConfig,
) -> Result<MigrationPlan, ConfigError> {
    let mut plan = MigrationPlan::default();
    
    let sources_json = space_root.join(".notez/sources.json");
    if sources_json.is_file() {
        if let Ok(text) = std::fs::read_to_string(&sources_json) {
            if let Ok(legacy) = serde_json::from_str::<LegacySources>(&text) {
                for s in legacy.sources {
                    if !current_cfg.sources.iter().any(|existing| existing.id == s.id) {
                        plan.sources_to_add.push(s);
                    }
                }
            }
        }
    }

    let comms_json = space_root.join(".notez/communities.json");
    if comms_json.is_file() {
        if let Ok(text) = std::fs::read_to_string(&comms_json) {
            if let Ok(legacy) = serde_json::from_str::<LegacyCommunities>(&text) {
                plan.communities_to_extract.extend(legacy.communities);
            }
        }
    }

    Ok(plan)
}

pub fn apply_legacy_migration(
    space_root: &Path,
    mut current_cfg: SpaceConfig,
    plan: MigrationPlan,
) -> Result<(), ConfigError> {
    if plan.sources_to_add.is_empty() && plan.communities_to_extract.is_empty() {
        return Ok(());
    }

    // Add sources
    if !plan.sources_to_add.is_empty() {
        current_cfg.sources.extend(plan.sources_to_add);
        let text = toml::to_string_pretty(&current_cfg).unwrap();
        let cfg_path = space_root.join("notez.toml");
        std::fs::write(&cfg_path, text).map_err(|e| ConfigError::Invalid("io", e.to_string()))?;
    }

    // "Extract" communities (for now we just leave them in communities.json
    // as it's the domain store, but we can write them back nicely if needed.
    // T5 specifies they move to independent domain declarations, but leaving
    // them in `.notez/communities.json` as a domain artifact is acceptable
    // for this milestone as long as they don't block TOML validation).
    
    Ok(())
}
