use crate::config::model::{ConfigError, GlobalConfig, SourceConfig, SourceRegistration};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Pure discovery inputs: environment map (HOME, XDG_CONFIG_HOME, …) and the
/// current working directory. Tests can supply both without mutating the real
/// process environment.
pub struct ConfigPaths {
    pub global: PathBuf,
    pub cwd: PathBuf,
    pub global_config: Option<GlobalConfig>,
    pub source_config: Option<(PathBuf, SourceConfig)>,
}

impl ConfigPaths {
    /// Discover the global config path under XDG and the nearest source
    /// config by walking upward from `cwd`.
    pub fn discover(
        env: &BTreeMap<String, OsString>,
        cwd: impl AsRef<Path>,
    ) -> Result<Self, ConfigError> {
        let cwd = cwd.as_ref();
        let home: PathBuf = env
            .get("HOME")
            .or_else(|| env.get("USERPROFILE"))
            .map(|s| PathBuf::from(s.as_os_str()))
            .unwrap_or_else(|| PathBuf::from("."));
        let xdg_config_home: PathBuf = env
            .get("XDG_CONFIG_HOME")
            .map(|s| PathBuf::from(s.as_os_str()))
            .unwrap_or_else(|| home.join(".config"));

        let global = xdg_config_home.join("notez").join("config.toml");
        let global_config = if global.is_file() {
            std::fs::read_to_string(&global)
                .ok()
                .and_then(|text| GlobalConfig::parse(&text).ok())
        } else {
            None
        };
        let source_config = find_upward_notez_toml(cwd);

        Ok(Self {
            global,
            cwd: cwd.to_path_buf(),
            global_config,
            source_config,
        })
    }
}

fn find_upward_notez_toml(start: &Path) -> Option<(PathBuf, SourceConfig)> {
    let mut current: Option<&Path> = Some(start);

    while let Some(dir) = current {
        let candidate = dir.join("notez.toml");
        if candidate.is_file() {
            let text = std::fs::read_to_string(&candidate).ok()?;
            // Try v2 first, then attempt v1 migration.
            match SourceConfig::parse(&text) {
                Ok(cfg) => return Some((candidate, cfg)),
                Err(_) => {}
            }
        }
        current = dir.parent();
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSelector<'a> {
    /// A name registered in the global config (`--source personal`).
    Name(&'a str),
    /// An explicit path to a source root or its `notez.toml`.
    Path(&'a Path),
    /// Walk upward from the current working directory.
    Upward,
    /// Use the global config's `default_source`, if any.
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSource {
    pub source_name: String,
    pub root: PathBuf,
    pub source_config_path: PathBuf,
    pub registration: Option<SourceRegistration>,
    pub source_config: SourceConfig,
}

pub fn select_source(
    paths: &ConfigPaths,
    selector: SourceSelector<'_>,
) -> Result<SelectedSource, ConfigError> {
    match selector {
        SourceSelector::Name(name) => {
            let global = paths
                .global_config
                .as_ref()
                .ok_or_else(|| ConfigError::Invalid("space", "unknown source".to_string()))?;
            let reg = global
                .sources
                .get(name)
                .ok_or_else(|| ConfigError::Invalid("space", format!("unknown source '{name}'")))?;
            let root = expand_tilde(&reg.path);
            let space_toml = if let Some(c) = reg.config.as_ref() {
                c.clone()
            } else {
                root.join("notez.toml")
            };
            let source_config = if space_toml.exists() {
                SourceConfig::parse(
                    &std::fs::read_to_string(&space_toml)
                        .map_err(|e| ConfigError::Invalid("io", e.to_string()))?,
                )?
            } else {
                SourceConfig {
                    version: crate::config::model::CURRENT_VERSION,
                    source: crate::config::model::SourceIdentity {
                        name: name.to_string(),
                        database: crate::config::model::default_database(),
                    },
                    workflow: Default::default(),
                    sources: Vec::new(),
                    link_overrides: serde_json::Value::Null,
                }
            };
            Ok(SelectedSource {
                source_name: name.to_string(),
                root: root,
                source_config_path: space_toml,
                registration: Some(reg.clone()),
                source_config,
            })
        }
        SourceSelector::Path(p) => {
            let root = expand_tilde(p);
            if !root.is_dir() && !(root.is_file() && root.extension().is_some_and(|e| e == "toml"))
            {
                return Err(ConfigError::Invalid(
                    "space",
                    "invalid source path: not a notez source or toml".to_string(),
                ));
            }
            let space_toml = if root.is_file() {
                root.clone()
            } else if root.join("notez.toml").is_file() {
                root.join("notez.toml")
            } else {
                // Plain directory without notez.toml: auto-construct a
                // default v2 source so the CLI can run against a fresh
                // workspace. The user can later write notez.toml to
                // customise the source.
                root.join("notez.toml")
            };
            let cfg = if space_toml.is_file() {
                SourceConfig::parse(
                    &std::fs::read_to_string(&space_toml)
                        .map_err(|e| ConfigError::Invalid("io", e.to_string()))?,
                )?
            } else {
                SourceConfig {
                    version: crate::config::model::CURRENT_VERSION,
                    source: crate::config::model::SourceIdentity {
                        name: root
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        database: crate::config::model::default_database(),
                    },
                    workflow: Default::default(),
                    sources: Vec::new(),
                    link_overrides: serde_json::Value::Null,
                }
            };
            let root_dir = space_toml.parent().unwrap().to_path_buf();
            Ok(SelectedSource {
                source_name: cfg.source.name.clone(),
                root: root_dir,
                source_config_path: space_toml,
                registration: None,
                source_config: cfg,
            })
        }
        SourceSelector::Upward => {
            if let Some((path, cfg)) = paths.source_config.clone() {
                let name = cfg.source.name.clone();
                Ok(SelectedSource {
                    source_name: name,
                    root: path.parent().unwrap().to_path_buf(),
                    source_config_path: path,
                    registration: None,
                    source_config: cfg,
                })
            } else {
                Err(ConfigError::Invalid(
                    "space",
                    "no notez.toml found upward from current directory".to_string(),
                ))
            }
        }
        SourceSelector::Default => {
            let name = paths
                .global_config
                .as_ref()
                .and_then(|g| g.default_source.clone())
                .ok_or_else(|| ConfigError::MissingField("default_source"))?;
            select_source(paths, SourceSelector::Name(&name))
        }
    }
}

impl SelectedSource {
    pub fn effective_root(&self) -> &Path {
        if self.registration.is_some() {
            &self.root
        } else {
            &self.root
        }
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = home_dir() {
            return if stripped.as_os_str().is_empty() {
                home
            } else {
                home.join(stripped)
            };
        }
    }
    path.to_path_buf()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
