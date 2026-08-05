use crate::config::{GlobalConfig, SpaceConfig};

#[test]
fn unknown_space_field_is_rejected() {
    let err = SpaceConfig::parse("version=1\n[space]\nname='x'\nnaem='typo'").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("naem"), "msg = {msg}");
}
#[test]
fn valid_global_config_parses() {
    let text = r#"
version = 1
default_space = "personal"

[spaces.personal]
path = "~/Notes/personal"
"#;
    let cfg = GlobalConfig::parse(text).unwrap();
    assert_eq!(cfg.default_space.as_deref(), Some("personal"));
    assert!(cfg.spaces.contains_key("personal"));
}

#[test]
fn missing_version_is_rejected() {
    let err = GlobalConfig::parse("[spaces.x]\npath='/n'").unwrap_err();
    assert!(err.to_string().contains("version"));
}

#[test]
fn space_config_workflow_round_trip() {
    let text = r#"
version = 1
[space]
name = "personal"
database = ".notez/index.sqlite"
[workflow]
todo = ["TODO", "NEXT"]
done = ["DONE"]
[[sources]]
id = "vault"
kind = "obsidian"
path = "../vault"
read_only = true
"#;
    let cfg = SpaceConfig::parse(text).unwrap();
    assert_eq!(cfg.workflow.todo, vec!["TODO", "NEXT"]);
    assert_eq!(cfg.workflow.done, vec!["DONE"]);
    assert_eq!(cfg.sources.len(), 1);
    assert!(cfg.sources[0].read_only);
}
