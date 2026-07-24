use crate::model::{ConfigError, GlobalConfig, SpaceConfig, SpaceRegistration};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Pure discovery inputs: environment map (HOME, XDG_CONFIG_HOME, …) and the
/// current working directory. Tests can supply both without mutating the
/// real process environment.
pub struct ConfigPaths {
    pub global: PathBuf,
    pub cwd: PathBuf,
    pub global_config: Option<GlobalConfig>,
    pub space_config: Option<(PathBuf, SpaceConfig)>,
}

impl ConfigPaths {
    /// Discover the global config path under XDG and the nearest space
    /// config by walking upward from `cwd`.
    pub fn discover(
        env: &BTreeMap<String, OsString>,
        cwd: impl AsRef<Path>,
    ) -> Result<Self, ConfigError> {
        let cwd = cwd.as_ref().to_path_buf();
        let xdg_config_home = env
            .get("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env.get("HOME").map(|h| PathBuf::from(h).join(".config")))
            .ok_or(ConfigError::MissingField("XDG_CONFIG_HOME / HOME"))?;
        let global = xdg_config_home.join("notez").join("config.toml");

        let global_config = if global.exists() {
            let text = std::fs::read_to_string(&global).map_err(|e| ConfigError::Invalid("io", e.to_string()))?;
            Some(GlobalConfig::parse(&text)?)
        } else {
            None
        };

        let space_config = find_upward_notez_toml(&cwd);

        Ok(Self {
            global,
            cwd,
            global_config,
            space_config,
        })
    }
}

fn find_upward_notez_toml(start: &Path) -> Option<(PathBuf, SpaceConfig)> {
    let mut current: Option<&Path> = Some(start);

    while let Some(dir) = current {
        let candidate = dir.join("notez.toml");

        if candidate.is_file() {
            let text = std::fs::read_to_string(&candidate).unwrap();
            match SpaceConfig::parse(&text) {
                Ok(cfg) => return Some((candidate, cfg)),
                Err(_) => {} // skip invalid notes.toml
            }
        }
        current = dir.parent();
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaceSelector<'a> {
    /// A name registered in the global config (`--space personal`).
    Name(&'a str),
    /// An explicit path to a space root or its `notez.toml`.
    Path(&'a Path),
    /// Walk upward from the cwd to find the nearest `notez.toml`.
    Upward,
    /// Use the global `default_space` entry.
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSpace {
    pub space_name: String,
    pub space_root: PathBuf,
    pub space_config_path: PathBuf,
    pub registration: Option<SpaceRegistration>,
    pub space_config: SpaceConfig,
}
pub fn select_space(
    paths: &ConfigPaths,
    selector: SpaceSelector<'_>,
) -> Result<SelectedSpace, ConfigError> {
    match selector {
        SpaceSelector::Name(name) => {
            let global = paths
                .global_config
                .as_ref()
                .ok_or(ConfigError::MissingField("global config"))?;
            let reg = global
                .spaces
                .get(name)
                .ok_or_else(|| ConfigError::Invalid("space", format!("unknown space '{name}'")))?
                .clone();
            let root = expand_tilde(&reg.path);
            let space_toml = root.join("notez.toml");
            let space_config = if space_toml.exists() {
                SpaceConfig::parse(&std::fs::read_to_string(&space_toml).map_err(|e| ConfigError::Invalid("io", e.to_string()))?)?
            } else {
                SpaceConfig {
                    version: 1,
                    space: crate::model::SpaceIdentity { name: name.to_string(), database: crate::model::default_database() },
                    workflow: Default::default(),
                    sources: Vec::new(),
                    link_overrides: serde_json::Value::Null,
                }
            };
            Ok(SelectedSpace {
                space_name: name.to_string(),
                space_root: root,
                space_config_path: space_toml,
                registration: Some(reg),
space_config: space_config.clone(),
            })
            .and_then(|sel| sel.with_space(&space_config))
        }
        SpaceSelector::Path(p) => {
            let root = expand_tilde(p);
            let space_toml = if root.join("notez.toml").is_file() {
                root.join("notez.toml")
            } else if root.is_file() {
                root.clone()
            } else if root.is_dir() {
                root.join("notez.toml")
            } else {
                return Err(ConfigError::Invalid(
                    "space",
                    format!("invalid space path {}", root.display()),
                ));
            };
            
            let cfg = if space_toml.exists() {
                SpaceConfig::parse(
                    &std::fs::read_to_string(&space_toml).map_err(|e| ConfigError::Invalid("io", e.to_string()))?,
                )?
            } else {
                SpaceConfig {
                    version: 1,
                    space: crate::model::SpaceIdentity { name: root.file_name().unwrap_or_default().to_string_lossy().to_string(), database: crate::model::default_database() },
                    workflow: Default::default(),
                    sources: Vec::new(),
                    link_overrides: serde_json::Value::Null,
                }
            };
            let name = cfg.space.name.clone();
            let root_dir = space_toml.parent().unwrap().to_path_buf();
            Ok(SelectedSpace {
                space_name: name,
                space_root: root_dir,
                space_config_path: space_toml,
                registration: None,
space_config: cfg.clone(),
            })
            .and_then(|sel| sel.with_space(&cfg))
        }
        SpaceSelector::Upward => {
            let (path, cfg) = paths
                .space_config
                .clone()
                .ok_or_else(|| ConfigError::Invalid(
                    "space",
                    "no notez.toml found above cwd".to_string(),
                ))?;
            let name = cfg.space.name.clone();
            let root = path.parent().unwrap().to_path_buf();
            Ok(SelectedSpace {
                space_name: name,
                space_root: root,
                space_config_path: path,
                registration: None,
space_config: cfg.clone(),
            })
            .and_then(|sel| sel.with_space(&cfg))
        }
        SpaceSelector::Default => {
            if paths.global_config.is_none() {
                return Err(ConfigError::Invalid(
                    "space",
                    "no space selected (missing global config and no upward notez.toml)"
                        .to_string(),
                ));
            }
            let global = paths.global_config.as_ref().unwrap();
            let name = global
                .default_space
                .as_ref()
                .ok_or_else(|| ConfigError::MissingField("default_space"))?;
            select_space(paths, SpaceSelector::Name(name))
        }
    }
}

impl SelectedSpace {
    fn with_space(self, cfg: &SpaceConfig) -> Result<Self, ConfigError> {
        // If the global registration pointed at a different `notez.toml` we
        // keep the one inside the space; this method exists so callers
        // receive a fully populated `SelectedSpace`.
        let _ = cfg;
        Ok(self)
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    } else if s == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    path.to_path_buf()
}


