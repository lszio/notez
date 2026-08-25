use notez_core::config::{GlobalConfig, SourceConfig};

#[test]
fn unknown_field_is_rejected() {
    let err = SourceConfig::parse("version=2\n[source]\nname='x'\nnaem='typo'").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("naem"), "msg = {msg}");
}
#[test]
fn valid_global_config_parses() {
    let text = r#"
version = 2
default_source = "personal"

[sources.personal]
path = "~/Notes/personal"
"#;
    let cfg = GlobalConfig::parse(text).unwrap();
    assert_eq!(cfg.default_source.as_deref(), Some("personal"));
    assert!(cfg.sources.contains_key("personal"));
}

#[test]
fn missing_version_is_rejected() {
    let err = GlobalConfig::parse("[sources.x]\npath='/n'").unwrap_err();
    assert!(err.to_string().contains("version"));
}

#[test]
fn source_config_workflow_round_trip() {
    let text = r#"
version = 2
[source]
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
    let cfg = SourceConfig::parse(text).unwrap();
    assert_eq!(cfg.workflow.todo, vec!["TODO", "NEXT"]);
    assert_eq!(cfg.workflow.done, vec!["DONE"]);
    assert_eq!(cfg.sources.len(), 1);
    assert!(cfg.sources[0].read_only);
}
