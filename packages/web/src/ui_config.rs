//! Web-owned UI configuration (`web.toml`).
//!
//! The web client keeps its own small config file next to the global
//! `config.toml` (`~/.config/notez/web.toml` by default). It stores
//! presentation-only state that no other surface shares:
//!
//! - `home_widgets` — ordered list of dashboard widget ids. Hidden
//!   widgets stay in the list so reordering survives a hide/show
//!   cycle; `hidden_home_widgets` holds the ids currently not shown.
//! - `landing` — per-source landing mode (`journal` | `index` |
//!   `files`) used by the space home page.
//! - `starred` — per-source list of starred resource refs, rendered
//!   as the star toggle on the note page.
//!
//! The file is deliberately *not* part of `notez_core::config`: the
//! core parser rejects unknown fields, and this state is a web-surface
//! concern. Unknown fields here are ignored (forward compatible), and
//! `home_widgets` self-heals: unknown ids are dropped, missing known
//! ids are appended in canonical order.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Widget ids the home dashboard understands, in canonical (default)
/// display order.
pub const WIDGET_IDS: [&str; 5] = ["journal", "recent", "activity", "spaces", "graph"];

/// Landing modes a space home can use.
pub const LANDING_MODES: [&str; 3] = ["journal", "index", "files"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebUiConfig {
    pub version: u8,
    #[serde(default)]
    pub home_widgets: Vec<String>,
    #[serde(default)]
    pub hidden_home_widgets: Vec<String>,
    /// Absolute source root → landing mode.
    #[serde(default)]
    pub landing: BTreeMap<String, String>,
    /// Absolute source root → starred resource refs.
    #[serde(default)]
    pub starred: BTreeMap<String, Vec<String>>,
}

impl Default for WebUiConfig {
    fn default() -> Self {
        Self {
            version: 1,
            home_widgets: WIDGET_IDS.iter().map(|s| s.to_string()).collect(),
            hidden_home_widgets: Vec::new(),
            landing: BTreeMap::new(),
            starred: BTreeMap::new(),
        }
    }
}

impl WebUiConfig {
    /// Visible home widgets in display order. Self-heals the stored
    /// list: drops unknown ids, appends missing known ids in canonical
    /// order, then subtracts the hidden set.
    pub fn visible_home_widgets(&self) -> Vec<String> {
        let mut ordered: Vec<String> = self
            .home_widgets
            .iter()
            .filter(|w| WIDGET_IDS.contains(&w.as_str()))
            .cloned()
            .collect();
        for id in WIDGET_IDS {
            if !ordered.iter().any(|w| w == id) {
                ordered.push(id.to_string());
            }
        }
        ordered
            .into_iter()
            .filter(|w| !self.hidden_home_widgets.iter().any(|h| h == w))
            .collect()
    }

    /// Landing mode for `source_root`, defaulting to `index`.
    pub fn landing_for(&self, source_root: &str) -> &'static str {
        match self.landing.get(source_root).map(|s| s.as_str()) {
            Some("journal") => "journal",
            Some("files") => "files",
            _ => "index",
        }
    }

    /// Whether `ref_str` is starred for `source_root`.
    pub fn is_starred(&self, source_root: &str, ref_str: &str) -> bool {
        self.starred
            .get(source_root)
            .is_some_and(|list| list.iter().any(|r| r == ref_str))
    }

    /// Toggle the star for `(source_root, ref_str)`. Returns the new
    /// state (`true` = starred after the call).
    pub fn toggle_star(&mut self, source_root: &str, ref_str: &str) -> bool {
        let entry = self.starred.entry(source_root.to_string()).or_default();
        if let Some(pos) = entry.iter().position(|r| r == ref_str) {
            entry.remove(pos);
            if entry.is_empty() {
                self.starred.remove(source_root);
            }
            false
        } else {
            entry.push(ref_str.to_string());
            true
        }
    }

    /// Move `widget` one slot up or down in the (full) widget order.
    /// Hidden state is untouched. Unknown ids are ignored.
    pub fn move_widget(&mut self, widget: &str, up: bool) {
        if !WIDGET_IDS.contains(&widget) {
            return;
        }
        if !self.home_widgets.iter().any(|w| w == widget) {
            self.home_widgets = WIDGET_IDS.iter().map(|s| s.to_string()).collect();
        }
        let Some(pos) = self.home_widgets.iter().position(|w| w == widget) else {
            return;
        };
        let target = if up { pos.checked_sub(1) } else { Some(pos + 1) };
        if let Some(target) = target {
            if target < self.home_widgets.len() {
                self.home_widgets.swap(pos, target);
            }
        }
    }

    /// Hide or unhide a home widget. Unknown ids are ignored.
    pub fn set_widget_hidden(&mut self, widget: &str, hidden: bool) {
        if !WIDGET_IDS.contains(&widget) {
            return;
        }
        if hidden {
            if !self.hidden_home_widgets.iter().any(|w| w == widget) {
                self.hidden_home_widgets.push(widget.to_string());
            }
        } else {
            self.hidden_home_widgets.retain(|w| w != widget);
        }
    }
}

/// Resolve the web config path: a `web.toml` sibling of the global
/// `config.toml` discovered through the standard XDG lookup.
pub fn web_ui_config_path() -> Result<PathBuf, String> {
    let env: BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v))
        .collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let paths = notez_core::config::ConfigPaths::discover(&env, &cwd)
        .map_err(|e| format!("config discovery: {e}"))?;
    Ok(paths.global.with_file_name("web.toml"))
}

/// Load the web UI config, falling back to defaults when the file is
/// missing or unreadable. A corrupt file never breaks the UI.
pub fn load_web_ui_config() -> WebUiConfig {
    load_web_ui_config_from(web_ui_config_path().ok().as_deref())
}

/// Store the web UI config atomically (tmp file + rename).
pub fn store_web_ui_config(cfg: &WebUiConfig) -> Result<(), String> {
    let path = web_ui_config_path()?;
    store_web_ui_config_to(cfg, &path)
}

pub fn load_web_ui_config_from(path: Option<&std::path::Path>) -> WebUiConfig {
    let Some(path) = path else {
        return WebUiConfig::default();
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return WebUiConfig::default(),
    };
    toml::from_str(&text).unwrap_or_default()
}

pub fn store_web_ui_config_to(cfg: &WebUiConfig, path: &std::path::Path) -> Result<(), String> {
    let text = toml::to_string(cfg).map_err(|e| format!("serialize web.toml: {e}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write web.toml: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename web.toml: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_widget_in_canonical_order() {
        let cfg = WebUiConfig::default();
        assert_eq!(cfg.visible_home_widgets(), ["journal", "recent", "activity", "spaces", "graph"]);
    }

    #[test]
    fn visible_drops_unknown_and_appends_missing() {
        let cfg = WebUiConfig {
            home_widgets: vec!["graph".into(), "bogus".into()],
            ..WebUiConfig::default()
        };
        assert_eq!(
            cfg.visible_home_widgets(),
            ["graph", "journal", "recent", "activity", "spaces"]
        );
    }

    #[test]
    fn hidden_widgets_disappear_but_keep_order() {
        let mut cfg = WebUiConfig::default();
        cfg.set_widget_hidden("recent", true);
        assert_eq!(cfg.visible_home_widgets(), ["journal", "activity", "spaces", "graph"]);
        cfg.set_widget_hidden("recent", false);
        assert_eq!(cfg.visible_home_widgets(), ["journal", "recent", "activity", "spaces", "graph"]);
    }

    #[test]
    fn move_widget_swaps_adjacent_positions() {
        let mut cfg = WebUiConfig::default();
        cfg.move_widget("recent", true);
        assert_eq!(cfg.home_widgets, vec!["recent", "journal", "activity", "spaces", "graph"]);
        cfg.move_widget("recent", false);
        assert_eq!(cfg.home_widgets, vec!["journal", "recent", "activity", "spaces", "graph"]);
        // Moving the first widget up is a no-op.
        cfg.move_widget("journal", true);
        assert_eq!(cfg.home_widgets, vec!["journal", "recent", "activity", "spaces", "graph"]);
    }

    #[test]
    fn move_widget_self_heals_truncated_list() {
        let mut cfg = WebUiConfig {
            home_widgets: vec!["graph".into()],
            ..WebUiConfig::default()
        };
        cfg.move_widget("journal", true);
        assert_eq!(cfg.home_widgets.len(), WIDGET_IDS.len());
    }

    #[test]
    fn star_toggle_round_trips() {
        let mut cfg = WebUiConfig::default();
        assert!(!cfg.is_starred("/s", "doc:1"));
        assert!(cfg.toggle_star("/s", "doc:1"));
        assert!(cfg.is_starred("/s", "doc:1"));
        assert!(!cfg.is_starred("/other", "doc:1"));
        assert!(!cfg.toggle_star("/s", "doc:1"));
        assert!(cfg.starred.get("/s").is_none(), "empty list is pruned");
    }

    #[test]
    fn landing_defaults_to_index_and_accepts_known_modes() {
        let mut cfg = WebUiConfig::default();
        assert_eq!(cfg.landing_for("/s"), "index");
        cfg.landing.insert("/s".into(), "journal".into());
        assert_eq!(cfg.landing_for("/s"), "journal");
        cfg.landing.insert("/s".into(), "weird".into());
        assert_eq!(cfg.landing_for("/s"), "index", "unknown modes fall back");
    }

    #[test]
    fn toml_round_trip_preserves_state() {
        let mut cfg = WebUiConfig::default();
        cfg.move_widget("graph", true);
        cfg.set_widget_hidden("spaces", true);
        cfg.toggle_star("/notes", "doc:abc");
        cfg.landing.insert("/notes".into(), "journal".into());

        let dir = std::env::temp_dir().join(format!("notez-webcfg-test-{}", std::process::id()));
        let path = dir.join("web.toml");
        store_web_ui_config_to(&cfg, &path).unwrap();
        let loaded = load_web_ui_config_from(Some(&path));
        assert_eq!(loaded, cfg);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("notez-webcfg-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("web.toml");
        std::fs::write(&path, "!!! not toml !!!").unwrap();
        assert_eq!(load_web_ui_config_from(Some(&path)), WebUiConfig::default());
        std::fs::remove_dir_all(dir).ok();
    }
}
