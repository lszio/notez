use std::path::{Path, PathBuf};
use serde_json::Value as JsonValue;
use serde_json::json;

pub fn merge_link_overrides(base: &JsonValue, override_val: &JsonValue) -> JsonValue {
    if override_val.is_null() {
        return base.clone();
    }
    match (base, override_val) {
        (JsonValue::Object(map_base), JsonValue::Object(map_override)) => {
            let mut result = map_base.clone();
            for (k, v) in map_override {
                let merged = merge_link_overrides(
                    map_base.get(k).unwrap_or(&JsonValue::Null),
                    v,
                );
                result.insert(k.clone(), merged);
            }
            JsonValue::Object(result)
        }
        (_, other) => other.clone(),
    }
}

pub fn built_in_link_profiles() -> JsonValue {
    json!({
        "org": {
            "extract_schemes": ["id", "file", "http", "https"],
            "allow_custom": true,
            "resolution_order": ["exact_ref", "path", "alias", "title"]
        },
        "markdown": {
            "extract_schemes": ["file", "http", "https"],
            "allow_custom": false,
            "resolution_order": ["path", "title"]
        },
        "obsidian": {
            "extract_schemes": ["file", "http", "https", "wikilink"],
            "allow_custom": false,
            "resolution_order": ["path", "alias", "basename", "title"]
        }
    })
}

pub fn resolve_path(base: &Path, rel: &Path) -> PathBuf {
    if rel.is_absolute() {
        rel.to_path_buf()
    } else {
        base.join(rel)
    }
}
