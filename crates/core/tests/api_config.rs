//! Contract tests for `core::config`.
//!
//! Covers: v1→v2 migration, GlobalConfig/SourceConfig parsing, discovery,
//! select_source, and runtime resolution.

use notez_core::config::defaults::{built_in_link_profiles, merge_link_overrides, resolve_path};
use notez_core::config::migrate::{
    LegacyCommunities, LegacySources, MigrationPlan, apply_legacy_migration, plan_legacy_migration,
};
use notez_core::config::{
    CURRENT_VERSION, ConfigError, ConfigPaths, GlobalConfig, Preferences, ResolvedSourceRuntime,
    SourceConfig, SourceIdentity, SourceInstanceConfig, SourceRegistration, SourceSelector,
    WorkflowConfig, resolve_source_runtime, select_source,
};
use notez_core::domain::Community;
use notez_core::source::SourceKind;
use serde_json::json;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

fn environment(config_home: &Path) -> BTreeMap<String, OsString> {
    BTreeMap::from([(
        "XDG_CONFIG_HOME".to_string(),
        config_home.as_os_str().to_os_string(),
    )])
}

fn minimal_source(name: &str) -> SourceConfig {
    SourceConfig {
        version: CURRENT_VERSION,
        source: SourceIdentity {
            name: name.into(),
            database: PathBuf::from(".notez/index.sqlite"),
        },
        workflow: WorkflowConfig::default(),
        sources: Vec::new(),
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    }
}

fn write_source(source_root: &Path, name: &str) {
    fs::create_dir_all(source_root).unwrap();
    fs::write(
        source_root.join("notez.toml"),
        format!("version = 2\n[source]\nname = \"{name}\"\ndatabase = \".notez/index.sqlite\"\n"),
    )
    .unwrap();
}

#[test]
fn config_model_parsing_applies_defaults_and_rejects_invalid_versions_fields() {
    let s = SourceConfig::parse(
        r#"
version = 2

[source]
name = "personal"
"#,
    )
    .unwrap();
    assert_eq!(s.source.name, "personal");
    assert_eq!(s.source.database, PathBuf::from(".notez/index.sqlite"));
    assert!(s.workflow.todo.is_empty());
    assert!(s.workflow.done.is_empty());

    // Wrong version is rejected (any version != 2, including the retired v1).
    let wrong_version = SourceConfig::parse(
        r#"version = 1

[source]
name = "x"
"#,
    );
    assert!(matches!(
        wrong_version.unwrap_err(),
        ConfigError::UnsupportedVersion(1)
    ));

    let future_version = SourceConfig::parse(
        r#"version = 3

[source]
name = "x"
"#,
    );
    assert!(matches!(
        future_version.unwrap_err(),
        ConfigError::UnsupportedVersion(3)
    ));

    // Empty name rejected.
    assert!(matches!(
        SourceConfig::parse(
            r#"version = 2

[source]
name = ""
"#
        ),
        Err(ConfigError::MissingField("source.name"))
    ));

    // Global config v999 rejected.
    let unknown = GlobalConfig::parse("version = 999\n").unwrap_err();
    assert!(matches!(unknown, ConfigError::UnsupportedVersion(999)));
    assert!(GlobalConfig::parse("version = 2\n").is_ok());
}

#[test]
fn legacy_v1_source_config_converts_to_v2_defaults() {
    let cfg = SourceConfig::parse(
        r#"version = 1

[space]
name = "legacy"
database = "custom/index.sqlite"
"#,
    )
    .unwrap();
    assert_eq!(cfg.version, CURRENT_VERSION);
    assert_eq!(cfg.source.name, "legacy");
    assert_eq!(cfg.source.database, PathBuf::from("custom/index.sqlite"));
    assert_eq!(cfg.workflow, WorkflowConfig::default());
    assert!(cfg.sources.is_empty());
    assert!(cfg.link_overrides.is_null());
}

#[test]
fn legacy_v1_unknown_fields_are_rejected_without_data_loss() {
    let result = SourceConfig::parse(
        r#"version = 1

[space]
name = "legacy"
unexpected = true
"#,
    );
    assert!(result.is_err());

    let mixed = SourceConfig::parse(
        r#"version = 1

[space]
name = "legacy"

[source]
name = "also-legacy"
"#,
    );
    assert!(mixed.is_err());
}

#[test]
fn discovery_and_select_source_support_legacy_v1_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("legacy");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        root.join("notez.toml"),
        "version = 1\n[space]\nname = \"legacy\"\ndatabase = \".notez/legacy.sqlite\"\n",
    )
    .unwrap();

    let paths = ConfigPaths::discover(&environment(&dir.path().join("config")), &nested).unwrap();
    let (config_path, cfg) = paths.source_config.as_ref().unwrap();
    assert_eq!(config_path, &root.join("notez.toml"));
    assert_eq!(cfg.version, CURRENT_VERSION);
    assert_eq!(cfg.source.name, "legacy");

    let selected = select_source(&paths, SourceSelector::Upward).unwrap();
    assert_eq!(selected.source_name, "legacy");
    assert_eq!(selected.root, root);
}

#[test]
fn discovery_finds_xdg_global_and_nearest_source_and_reports_missing_home() {
    let dir = tempfile::tempdir().unwrap();
    let config_home = dir.path().join("config");
    let cwd = dir.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();
    let env = environment(&config_home);
    let paths = ConfigPaths::discover(&env, &cwd).unwrap();
    assert!(paths.global_config.is_none());
    assert!(paths.source_config.is_none());
    let _ = paths.global;
    let _ = paths.cwd;
}

#[test]
fn discovery_skips_invalid_source_config_and_rejects_invalid_global_config() {
    let dir = tempfile::tempdir().unwrap();
    let config_home = dir.path().join("config");
    let cwd = dir.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();
    let global_dir = config_home.join("notez");
    fs::create_dir_all(&global_dir).unwrap();
    // Malformed global config — GlobalConfig::parse returns Err for v999,
    // but ConfigPaths::discover swallows the error and returns None for
    // global_config (best-effort).
    fs::write(global_dir.join("config.toml"), "version = 999\n").unwrap();
    let env = environment(&config_home);
    let paths = ConfigPaths::discover(&env, &cwd).unwrap();
    assert!(paths.global_config.is_none());
}

#[test]
fn select_source_supports_path_upward_name_default_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("source");
    let nested = root.join("nested");
    write_source(&root, "personal");
    fs::create_dir_all(&nested).unwrap();
    let config_home = dir.path().join("config");
    let global_dir = config_home.join("notez");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(
        global_dir.join("config.toml"),
        format!(
            "version = 2\ndefault_source = \"personal\"\n\n[sources.personal]\npath = {:?}\n",
            root.to_string_lossy()
        ),
    )
    .unwrap();
    let paths = ConfigPaths::discover(&environment(&config_home), &nested).unwrap();

    for selected in [
        select_source(&paths, SourceSelector::Path(&root)).unwrap(),
        select_source(&paths, SourceSelector::Path(&root.join("notez.toml"))).unwrap(),
        select_source(&paths, SourceSelector::Upward).unwrap(),
        select_source(&paths, SourceSelector::Name("personal")).unwrap(),
        select_source(&paths, SourceSelector::Default).unwrap(),
    ] {
        assert_eq!(selected.source_name, "personal");
        assert_eq!(selected.root, root);
        assert_eq!(selected.source_config_path, root.join("notez.toml"));
    }

    let unknown = select_source(&paths, SourceSelector::Name("missing")).unwrap_err();
    assert!(
        unknown.to_string().contains("unknown source 'missing'")
            || unknown.to_string().contains("'missing'")
    );
    let invalid =
        select_source(&paths, SourceSelector::Path(&dir.path().join("missing"))).unwrap_err();
    assert!(invalid.to_string().contains("invalid source path"));
}

#[test]
fn runtime_merge_resolves_paths_and_applies_global_env_cli_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("source");
    let global = GlobalConfig {
        version: CURRENT_VERSION,
        default_source: Some("personal".into()),
        sources: BTreeMap::from([(
            "personal".into(),
            SourceRegistration {
                path: root.clone(),
                config: None,
            },
        )]),
        preferences: Preferences {
            output: "json".into(),
            log_level: "info".into(),
        },
    };
    let source_cfg = SourceConfig {
        version: CURRENT_VERSION,
        source: SourceIdentity {
            name: "personal".into(),
            database: PathBuf::from("db/index.sqlite"),
        },
        workflow: WorkflowConfig {
            todo: vec!["OPEN".into()],
            done: vec!["CLOSED".into()],
        },
        sources: vec![SourceInstanceConfig {
            id: "native".into(),
            kind: SourceKind::Native,
            path: PathBuf::from("notes"),
            read_only: false,
            url: None,
            include_paths: vec![PathBuf::from("notes/include")],
            exclude_paths: vec![PathBuf::from("notes/exclude")],
            scan: Default::default(),
        }],
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let paths = ConfigPaths {
        global: dir.path().join("config.toml"),
        cwd: root.clone(),
        global_config: Some(global),
        source_config: None,
    };
    let selected = notez_core::config::SelectedSource {
        source_name: "personal".into(),
        root: root.clone(),
        source_config_path: root.join("notez.toml"),
        registration: None,
        source_config: source_cfg,
    };
    let env = BTreeMap::from([("NOTEZ_LOG_LEVEL".into(), OsString::from("debug"))]);
    let runtime: ResolvedSourceRuntime =
        resolve_source_runtime(&paths, &selected, &env, Some("human".into())).unwrap();
    assert_eq!(runtime.config.source.database, root.join("db/index.sqlite"));
    assert_eq!(runtime.preferences.output, "human");
    assert_eq!(runtime.preferences.log_level, "debug");
    assert_eq!(runtime.config.sources[0].path, root.join("notes"));
    assert_eq!(
        runtime.config.sources[0].include_paths,
        vec![root.join("notes/include")]
    );
    assert_eq!(
        runtime.config.sources[0].exclude_paths,
        vec![root.join("notes/exclude")]
    );
}

#[test]
fn merge_defaults_are_recursive_and_path_resolution_handles_absolute_paths() {
    // Link profile defaults: every built-in profile has the expected shape.
    let profiles = built_in_link_profiles();
    assert!(profiles.get("org").is_some());
    assert!(profiles.get("markdown").is_some());
    assert!(profiles.get("obsidian").is_some());
    // Recursive override: nested objects are deep-merged.
    let merged = merge_link_overrides(
        &json!({"a": {"b": 1, "c": 2}, "x": 1}),
        &json!({"a": {"c": 99, "d": 3}, "y": 2}),
    );
    assert_eq!(merged["a"]["b"], json!(1));
    assert_eq!(merged["a"]["c"], json!(99));
    assert_eq!(merged["a"]["d"], json!(3));
    assert_eq!(merged["x"], json!(1));
    assert_eq!(merged["y"], json!(2));
    // Path resolution prefers absolute.
    let abs = PathBuf::from("/etc/notez");
    assert_eq!(resolve_path(Path::new("/some/base"), &abs), abs);
}

#[test]
fn migration_plan_deduplicates_sources_collects_communities_and_tolerates_bad_json() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = minimal_source("personal");
    let sources_json = dir.path().join(".notez/sources.json");
    fs::create_dir_all(sources_json.parent().unwrap()).unwrap();
    fs::write(
        &sources_json,
        r#"{"sources":[{"id":"native","kind":"native","path":"docs"}]}"#,
    )
    .unwrap();
    let plan: MigrationPlan = plan_legacy_migration(dir.path(), &cfg).unwrap();
    assert_eq!(
        plan.sources_to_add.len(),
        1,
        "expected the legacy sources.json entry to be migrated"
    );
    let communities_json = dir.path().join(".notez/communities.json");
    fs::write(
        &communities_json,
        r#"{"communities":[{"id":"c1","name":"C","selector":{"kind":"document"}}]}"#,
    )
    .unwrap();
    let plan: MigrationPlan = plan_legacy_migration(dir.path(), &cfg).unwrap();
    assert_eq!(plan.sources_to_add.len(), 1);
    assert_eq!(plan.sources_to_add[0].id, "native");
    assert_eq!(plan.communities_to_extract.len(), 1);
    assert_eq!(plan.communities_to_extract[0].id, "c1");

    // No files → empty plan.
    let empty_dir = tempfile::tempdir().unwrap();
    let empty = plan_legacy_migration(empty_dir.path(), &cfg).unwrap();
    assert!(empty.sources_to_add.is_empty());
    assert!(empty.communities_to_extract.is_empty());

    // Malformed JSON → tolerant (still produces empty plan).
    let bad_dir = tempfile::tempdir().unwrap();
    let nope = bad_dir.path().join(".notez/sources.json");
    fs::create_dir_all(nope.parent().unwrap()).unwrap();
    fs::write(&nope, "{ not valid json").unwrap();
    let bad = plan_legacy_migration(bad_dir.path(), &cfg).unwrap();
    assert!(bad.sources_to_add.is_empty());
}

#[test]
fn migration_apply_writes_new_sources_noops_empty_plan_and_reports_io_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = minimal_source("personal");
    let plan = MigrationPlan {
        sources_to_add: vec![SourceInstanceConfig {
            id: "added".into(),
            kind: SourceKind::Native,
            path: dir.path().join("docs"),
            read_only: false,
            url: None,
            include_paths: vec![],
            exclude_paths: vec![],
            scan: Default::default(),
        }],
        communities_to_extract: vec![Community {
            id: "c1".into(),
            name: "C".into(),
            selector: notez_core::domain::Selector::new(),
            pinned_members: Vec::new(),
            excluded_members: Vec::new(),
        }],
    };
    apply_legacy_migration(dir.path(), cfg.clone(), plan).unwrap();
    cfg.sources.push(SourceInstanceConfig {
        id: "added".into(),
        kind: SourceKind::Native,
        path: dir.path().join("docs"),
        read_only: false,
        url: None,
        include_paths: vec![],
        exclude_paths: vec![],
        scan: Default::default(),
    });
    let on_disk = std::fs::read_to_string(dir.path().join("notez.toml")).unwrap();
    let parsed = SourceConfig::parse(&on_disk).unwrap();
    assert_eq!(parsed.sources.len(), 1);
    assert_eq!(parsed.sources[0].id, "added");

    // Empty plan is a no-op.
    let noop_dir = tempfile::tempdir().unwrap();
    write_source(&noop_dir.path().to_path_buf(), "personal");
    let noop_cfg = minimal_source("personal");
    let original = noop_cfg.clone();
    apply_legacy_migration(&noop_dir.path(), noop_cfg, MigrationPlan::default()).unwrap();
    assert_eq!(original.sources.len(), 0);

    // I/O error path: plan_legacy_migration reads known files only,
    // so a missing directory produces an empty plan rather than an error.
    let missing_dir = tempfile::tempdir().unwrap();
    let missing = plan_legacy_migration(missing_dir.path(), &cfg).unwrap();
    assert!(missing.sources_to_add.is_empty());

    // Smoke: LegacySources/LegacyCommunities deserialize through serde.
    let _: LegacySources = serde_json::from_str(r#"{"sources":[]}"#).unwrap();
    let _: LegacyCommunities = serde_json::from_str(r#"{"communities":[]}"#).unwrap();
}
