//! Legacy data migration helpers.
//!
//! `plan_legacy_migration` / `apply_legacy_migration` ingest the legacy
//! JSON-based sources/communities caches under `<root>/.notez/` that
//! 0.4-era workspaces accumulated. The on-disk config schema is v2 only —
//! there is no v1 → v2 TOML upgrade path (v1 predates the first release).

use crate::config::model::{ConfigError, SourceConfig, SourceInstanceConfig};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacySources {
    pub sources: Vec<SourceInstanceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyCommunities {
    pub communities: Vec<crate::domain::community::Community>,
}

#[derive(Debug, Clone, Default)]
pub struct MigrationPlan {
    pub sources_to_add: Vec<SourceInstanceConfig>,
    pub communities_to_extract: Vec<crate::domain::community::Community>,
}

pub fn plan_legacy_migration(
    source_root: &Path,
    current_cfg: &SourceConfig,
) -> Result<MigrationPlan, ConfigError> {
    let mut plan = MigrationPlan::default();

    let sources_json = source_root.join(".notez/sources.json");
    if sources_json.is_file() {
        if let Ok(text) = std::fs::read_to_string(&sources_json) {
            if let Ok(legacy) = serde_json::from_str::<LegacySources>(&text) {
                for s in legacy.sources {
                    if !current_cfg
                        .sources
                        .iter()
                        .any(|existing| existing.id == s.id)
                    {
                        plan.sources_to_add.push(s);
                    }
                }
            }
        }
    }

    let comms_json = source_root.join(".notez/communities.json");
    if comms_json.is_file() {
        if let Ok(text) = std::fs::read_to_string(&comms_json) {
            match serde_json::from_str::<LegacyCommunities>(&text) {
                Ok(legacy) => plan.communities_to_extract.extend(legacy.communities),
                Err(_) => {}
            }
        }
    }

    Ok(plan)
}

pub fn apply_legacy_migration(
    source_root: &Path,
    mut current_cfg: SourceConfig,
    plan: MigrationPlan,
) -> Result<(), ConfigError> {
    if plan.sources_to_add.is_empty() && plan.communities_to_extract.is_empty() {
        return Ok(());
    }

    if !plan.sources_to_add.is_empty() {
        current_cfg.sources.extend(plan.sources_to_add);
        let text = toml::to_string_pretty(&current_cfg).unwrap();
        let cfg_path = source_root.join("notez.toml");
        std::fs::write(&cfg_path, text).map_err(|e| ConfigError::Invalid("io", e.to_string()))?;
    }

    Ok(())
}
