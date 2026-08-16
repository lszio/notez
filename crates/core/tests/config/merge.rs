use crate::config::model::ConfigError;
use crate::config::{ConfigPaths, SelectedSpace, RuntimeConfig, SpaceConfig, GlobalConfig};
use std::collections::BTreeMap;
use std::ffi::OsString;

// We need an origin enum inside the crate. We can test effective values via
// RuntimeConfig fields since they are built via merge.

fn env_with(vars: &[(&str, &str)]) -> BTreeMap<String, OsString> {
    vars.iter()
        .map(|(k, v)| (k.to_string(), OsString::from(v)))
        .collect()
}

#[test]
fn runtime_config_merges_global_and_space() {
    let global_text = r#"
version = 1
default_space = "personal"

[preferences]
output = "json"
log_level = "info"
"#;
    let space_text = r#"
version = 1
[space]
name = "personal"
database = ".notez/index.sqlite"
[workflow]
todo = ["A"]
done = ["B"]
"#;
    let global = GlobalConfig::parse(global_text).unwrap();
    let space = SpaceConfig::parse(space_text).unwrap();
    
    let mut paths = ConfigPaths {
        global: "/config".into(),
        cwd: "/cwd".into(),
        global_config: Some(global),
        space_config: None,
    };
    
    let sel = SelectedSpace {
        space_name: "personal".into(),
        space_root: "/space".into(),
        space_config_path: "/space/notez.toml".into(),
        registration: None,
        space_config: space,
    };
    
    // We expect a merge function that takes global, space, env, and cli.
    // For now we test via load_runtime_config (to be implemented).
    let env = env_with(&[("NOTEZ_LOG_LEVEL", "debug")]);
    let cli_output = Some("human".to_string());
    
    let rc = config::load_runtime_config(&paths, &sel, &env, cli_output).unwrap();
    
    // Preferences: output overridden by CLI, log_level overridden by env
    assert_eq!(rc.preferences.output, "human");
    assert_eq!(rc.preferences.log_level, "debug");
    
    // Database resolved relative to space_root
    assert_eq!(rc.database, std::path::PathBuf::from("/space/.notez/index.sqlite"));
    
    // Workflow from space
    assert_eq!(rc.workflow.todo, vec!["A"]);
}
