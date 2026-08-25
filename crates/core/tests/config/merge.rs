use notez_core::config::merge::resolve_source_runtime;
use notez_core::config::{ConfigPaths, GlobalConfig, SelectedSource, SourceConfig};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

fn env_with(vars: &[(&str, &str)]) -> BTreeMap<String, OsString> {
    vars.iter()
        .map(|(k, v)| (k.to_string(), OsString::from(v)))
        .collect()
}

#[test]
fn runtime_config_merges_global_and_source() {
    let global = GlobalConfig {
        version: 2,
        default_source: Some("personal".into()),
        sources: Default::default(),
        preferences: notez_core::config::Preferences {
            output: "json".into(),
            log_level: "info".into(),
        },
    };
    let source_cfg_text = r#"
version = 2
[source]
name = "personal"
database = ".notez/index.sqlite"
[workflow]
todo = ["A"]
done = ["B"]
"#;
    let source_cfg = SourceConfig::parse(source_cfg_text).unwrap();

    let paths = ConfigPaths {
        global: PathBuf::from("/config"),
        cwd: PathBuf::from("/cwd"),
        global_config: Some(global),
        source_config: None,
    };

    let sel = SelectedSource {
        source_name: "personal".into(),
        root: PathBuf::from("/source"),
        source_config_path: PathBuf::from("/source/notez.toml"),
        registration: None,
        source_config: source_cfg,
    };

    let env = env_with(&[("NOTEZ_LOG_LEVEL", "debug")]);
    let cli_output = Some("human".to_string());

    let rc = resolve_source_runtime(&paths, &sel, &env, cli_output).unwrap();

    // Preferences: output overridden by CLI, log_level overridden by env.
    assert_eq!(rc.preferences.output, "human");
    assert_eq!(rc.preferences.log_level, "debug");

    // Database resolved relative to the source root.
    assert_eq!(
        rc.config.source.database,
        PathBuf::from("/source/.notez/index.sqlite")
    );

    // Workflow from the on-disk source config.
    assert_eq!(rc.config.workflow.todo, vec!["A"]);
}
