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
//! - `dashboard` — notez card ordering + hidden ids for the
//!   document-driven home dashboard.
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

/// One dashboard layout (ordered + hidden card ids) for the
/// notez-driven home page. Lives in the same `web.toml` as the legacy
/// widget fields so the persistence boundary stays one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DashboardLayout {
    /// All known card ids in display order (visible + hidden).
    #[serde(default)]
    pub ordered_card_ids: Vec<String>,
    /// Card ids currently hidden from the dashboard.
    #[serde(default)]
    pub hidden_card_ids: Vec<String>,
}

impl DashboardLayout {
    /// Visible card ids in display order: ordered minus hidden.
    pub fn visible_card_ids(&self) -> Vec<String> {
        self.ordered_card_ids
            .iter()
            .filter(|id| !self.hidden_card_ids.iter().any(|h| h == *id))
            .cloned()
            .collect()
    }

    /// Move `card_id` one slot up or down in the ordered list.
    /// Unknown ids are ignored.
    pub fn move_card(&mut self, card_id: &str, up: bool) {
        let Some(pos) = self.ordered_card_ids.iter().position(|c| c == card_id) else {
            return;
        };
        let target = if up { pos.checked_sub(1) } else { Some(pos + 1) };
        if let Some(target) = target {
            if target < self.ordered_card_ids.len() {
                self.ordered_card_ids.swap(pos, target);
            }
        }
    }

    /// Hide or unhide a card id. Unknown ids are still recorded so
    /// hide/show cycles survive reordering.
    pub fn set_card_hidden(&mut self, card_id: &str, hidden: bool) {
        if hidden {
            if !self.hidden_card_ids.iter().any(|c| c == card_id) {
                self.hidden_card_ids.push(card_id.to_string());
            }
        } else {
            self.hidden_card_ids.retain(|c| c != card_id);
        }
    }

    /// Replace the ordered + hidden lists wholesale (the
    /// `update_dashboard` operation does this).
    pub fn replace_with(&mut self, ordered_card_ids: Vec<String>, hidden_card_ids: Vec<String>) {
        self.ordered_card_ids = ordered_card_ids;
        self.hidden_card_ids = hidden_card_ids;
    }
}

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
    /// Dashboard card layout (ordered + hidden ids).
    #[serde(default)]
    pub dashboard: DashboardLayout,
}

impl Default for WebUiConfig {
    fn default() -> Self {
        Self {
            version: 1,
            home_widgets: WIDGET_IDS.iter().map(|s| s.to_string()).collect(),
            hidden_home_widgets: Vec::new(),
            landing: BTreeMap::new(),
            starred: BTreeMap::new(),
            dashboard: DashboardLayout::default(),
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
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    notez_core::config::ConfigPaths::discover(&env, &cwd)
        .map_err(|e| e.to_string())
        .map(|p| p.global.parent().unwrap_or(&p.global).join("web.toml"))
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
    let serialized = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let parent = path.parent().ok_or("invalid web config path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join("web.toml.tmp");
    std::fs::write(&tmp, serialized).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Process-wide dashboard layout accessor.
pub fn load_dashboard_layout() -> DashboardLayout {
    load_web_ui_config_from(web_ui_config_path().ok().as_deref()).dashboard
}

/// Persist a freshly mutated dashboard layout.
pub fn store_dashboard_layout(layout: &DashboardLayout) -> Result<(), String> {
    let mut cfg = load_web_ui_config_from(web_ui_config_path().ok().as_deref());
    cfg.dashboard = layout.clone();
    store_web_ui_config(&cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_minimal_web_toml(dir: &std::path::Path) {
        fs::write(
            dir.join("web.toml"),
            "version = 1\n[dashboard]\nordered_card_ids = []\nhidden_card_ids = []\n",
        )
        .unwrap();
    }

    #[test]
    fn dashboard_layout_round_trips_and_orders() {
        let dir = tempdir().unwrap();
        write_minimal_web_toml(dir.path());
        let mut layout = DashboardLayout::default();
        layout.replace_with(
            vec!["a".to_string(), "b".to_string()],
            vec!["b".to_string()],
        );
        assert_eq!(layout.visible_card_ids(), vec!["a".to_string()]);
        layout.move_card("a", false);
        assert_eq!(layout.ordered_card_ids, vec!["b".to_string(), "a".to_string()]);
        layout.set_card_hidden("a", true);
        assert_eq!(layout.hidden_card_ids, vec!["b".to_string(), "a".to_string()]);
        let cfg = WebUiConfig { version: 1, dashboard: layout.clone(), ..WebUiConfig::default() };
        store_web_ui_config_to(&cfg, &dir.path().join("web.toml")).unwrap();
        let loaded = load_web_ui_config_from(Some(dir.path().join("web.toml").as_path()));
        assert_eq!(loaded.dashboard.ordered_card_ids, layout.ordered_card_ids);
        assert_eq!(loaded.dashboard.hidden_card_ids, layout.hidden_card_ids);
    }
}