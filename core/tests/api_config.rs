use notez_core::config::defaults::{built_in_link_profiles, merge_link_overrides, resolve_path};
use notez_core::config::migrate::{
    apply_legacy_migration, plan_legacy_migration, LegacyCommunities, LegacySources, MigrationPlan,
};
use notez_core::config::{
    load_runtime_config, select_space, ConfigError, ConfigPaths, GlobalConfig, Preferences,
    RuntimeConfig, SelectedSpace, SpaceConfig, SpaceIdentity, SpaceRegistration, SpaceSelector,
    SpaceSourceConfig, WorkflowConfig,
};
use notez_core::domain::{Community, Selector};
use notez_core::source::{SourceKind};
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

fn minimal_space(name: &str) -> SpaceConfig {
    SpaceConfig {
        version: 1,
        space: SpaceIdentity {
            name: name.into(),
            database: PathBuf::from(".notez/index.sqlite"),
        },
        workflow: WorkflowConfig::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
    }
}

fn write_space(root: &Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("notez.toml"),
        format!("version = 1\n\n[space]\nname = \"{name}\"\ndatabase = \".notez/index.sqlite\"\n"),
    )
    .unwrap();
}

#[test]
fn config_model_parsing_applies_defaults_and_rejects_invalid_versions_fields() {
    let global = GlobalConfig::parse(
        "version = 1\ndefault_space = \"personal\"\n\n[spaces.personal]\npath = \"/notes\"\n",
    )
    .unwrap();
    assert_eq!(global.default_space.as_deref(), Some("personal"));
    assert_eq!(global.preferences, Preferences::default());
    assert_eq!(global.spaces["personal"].path, PathBuf::from("/notes"));

    let space = SpaceConfig::parse("version = 1\n\n[space]\nname = \"personal\"\n").unwrap();
    assert_eq!(space.space.database, PathBuf::from(".notez/index.sqlite"));
    assert!(space.workflow.todo.is_empty());
    assert!(space.workflow.done.is_empty());

    assert!(matches!(
        GlobalConfig::parse("version = 2"),
        Err(ConfigError::UnsupportedVersion(2))
    ));
    assert!(matches!(
        SpaceConfig::parse("version = 1\n\n[space]\nname = \"\""),
        Err(ConfigError::MissingField("space.name"))
    ));
    let unknown = GlobalConfig::parse("version = 1\nunexpected = true").unwrap_err();
    assert!(unknown.to_string().contains("unknown field") || unknown.to_string().contains("unexpected"));
}

#[test]
fn discovery_finds_xdg_global_and_nearest_space_and_reports_missing_home() {
    let dir = tempfile::tempdir().unwrap();
    let config_home = dir.path().join("config");
    let global_dir = config_home.join("notez");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(
        global_dir.join("config.toml"),
        "version = 1\ndefault_space = \"personal\"\n\n[spaces.personal]\npath = \"/notes\"\n",
    )
    .unwrap();
    let space_root = dir.path().join("space");
    let nested = space_root.join("a/b");
    write_space(&space_root, "personal");
    fs::create_dir_all(&nested).unwrap();

    let paths = ConfigPaths::discover(&environment(&config_home), &nested).unwrap();
    assert_eq!(paths.global, global_dir.join("config.toml"));
    assert_eq!(paths.cwd, nested);
    assert_eq!(paths.global_config.unwrap().default_space.as_deref(), Some("personal"));
    assert_eq!(paths.space_config.unwrap().0, space_root.join("notez.toml"));

    let error = match ConfigPaths::discover(&BTreeMap::new(), dir.path()) {
        Ok(_) => panic!("discovery without XDG_CONFIG_HOME or HOME must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, ConfigError::MissingField("XDG_CONFIG_HOME / HOME")));
}

#[test]
fn discovery_skips_invalid_space_config_and_rejects_invalid_global_config() {
    let dir = tempfile::tempdir().unwrap();
    let space = dir.path().join("space");
    fs::create_dir_all(&space).unwrap();
    fs::write(space.join("notez.toml"), "version = 9\n[space]\nname = \"bad\"").unwrap();
    let config_home = dir.path().join("config");
    let paths = ConfigPaths::discover(&environment(&config_home), &space).unwrap();
    assert!(paths.space_config.is_none());

    let global_dir = config_home.join("notez");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(global_dir.join("config.toml"), "version = 9").unwrap();
    let error = match ConfigPaths::discover(&environment(&config_home), &space) {
        Ok(_) => panic!("unsupported global config version must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, ConfigError::UnsupportedVersion(9)));
}

#[test]
fn select_space_supports_path_upward_name_default_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("space");
    let nested = root.join("nested");
    write_space(&root, "personal");
    fs::create_dir_all(&nested).unwrap();
    let config_home = dir.path().join("config");
    let global_dir = config_home.join("notez");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(
        global_dir.join("config.toml"),
        format!(
            "version = 1\ndefault_space = \"personal\"\n\n[spaces.personal]\npath = {:?}\n",
            root.to_string_lossy()
        ),
    )
    .unwrap();
    let paths = ConfigPaths::discover(&environment(&config_home), &nested).unwrap();

    for selected in [
        select_space(&paths, SpaceSelector::Path(&root)).unwrap(),
        select_space(&paths, SpaceSelector::Path(&root.join("notez.toml"))).unwrap(),
        select_space(&paths, SpaceSelector::Upward).unwrap(),
        select_space(&paths, SpaceSelector::Name("personal")).unwrap(),
        select_space(&paths, SpaceSelector::Default).unwrap(),
    ] {
        assert_eq!(selected.space_name, "personal");
        assert_eq!(selected.space_root, root);
        assert_eq!(selected.space_config_path, root.join("notez.toml"));
    }

    let unknown = select_space(&paths, SpaceSelector::Name("missing")).unwrap_err();
    assert!(unknown.to_string().contains("unknown space 'missing'"));
    let invalid = select_space(&paths, SpaceSelector::Path(&dir.path().join("missing"))).unwrap_err();
    assert!(invalid.to_string().contains("invalid space path"));

    let empty_paths = ConfigPaths {
        global: dir.path().join("none.toml"),
        cwd: dir.path().into(),
        global_config: None,
        space_config: None,
    };
    assert!(select_space(&empty_paths, SpaceSelector::Upward).unwrap_err().to_string().contains("no notez.toml"));
    assert!(select_space(&empty_paths, SpaceSelector::Default).unwrap_err().to_string().contains("no space selected"));
}

#[test]
fn runtime_merge_resolves_paths_and_applies_global_env_cli_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("space");
    let global = GlobalConfig {
        version: 1,
        default_space: Some("personal".into()),
        spaces: BTreeMap::from([(
            "personal".into(),
            SpaceRegistration { path: root.clone(), config: None },
        )]),
        preferences: Preferences { output: "json".into(), log_level: "info".into() },
    };
    let space_cfg = SpaceConfig {
        version: 1,
        space: SpaceIdentity { name: "personal".into(), database: PathBuf::from("db/index.sqlite") },
        workflow: WorkflowConfig { todo: vec!["OPEN".into()], done: vec!["CLOSED".into()] },
        sources: vec![SpaceSourceConfig {
            id: "native".into(),
            kind: SourceKind::Native,
            path: PathBuf::from("notes"),
            read_only: false,
            include_paths: vec![PathBuf::from("notes/include")],
            exclude_paths: vec![PathBuf::from("notes/exclude")],
        }],
        link_overrides: serde_json::Value::Null,
    };
    let paths = ConfigPaths {
        global: dir.path().join("config.toml"),
        cwd: root.clone(),
        global_config: Some(global),
        space_config: None,
    };
    let selected = SelectedSpace {
        space_name: "personal".into(),
        space_root: root.clone(),
        space_config_path: root.join("notez.toml"),
        registration: None,
        space_config: space_cfg,
    };
    let env = BTreeMap::from([("NOTEZ_LOG_LEVEL".into(), OsString::from("debug"))]);
    let runtime = load_runtime_config(&paths, &selected, &env, Some("human".into())).unwrap();
    assert_eq!(runtime.space_root, root);
    assert_eq!(runtime.space_name, "personal");
    assert_eq!(runtime.database, runtime.space_root.join("db/index.sqlite"));
    assert_eq!(runtime.preferences.output, "human");
    assert_eq!(runtime.preferences.log_level, "debug");
    assert_eq!(runtime.sources[0].path, runtime.space_root.join("notes"));
    assert_eq!(runtime.sources[0].include_paths, vec![runtime.space_root.join("notes/include")]);
    assert_eq!(runtime.sources[0].exclude_paths, vec![runtime.space_root.join("notes/exclude")]);
}

#[test]
fn merge_defaults_are_recursive_and_path_resolution_handles_absolute_paths() {
    let base = json!({"org": {"allow_custom": true, "order": ["id"]}, "keep": 1});
    let override_value = json!({"org": {"allow_custom": false}, "add": 2});
    assert_eq!(
        merge_link_overrides(&base, &override_value),
        json!({"org": {"allow_custom": false, "order": ["id"]}, "keep": 1, "add": 2})
    );
    assert_eq!(merge_link_overrides(&base, &serde_json::Value::Null), base);
    assert_eq!(merge_link_overrides(&json!({"x": 1}), &json!([1, 2])), json!([1, 2]));
    assert_eq!(built_in_link_profiles()["obsidian"]["resolution_order"][2], "basename");
    assert_eq!(resolve_path(Path::new("/space"), Path::new("notes")), PathBuf::from("/space/notes"));
    assert_eq!(resolve_path(Path::new("/space"), Path::new("/other")), PathBuf::from("/other"));
}

#[test]
fn migration_plan_deduplicates_sources_collects_communities_and_tolerates_bad_json() {
    let dir = tempfile::tempdir().unwrap();
    let dot_notez = dir.path().join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let existing = SpaceSourceConfig {
        id: "existing".into(),
        kind: SourceKind::Native,
        path: PathBuf::from("existing"),
        read_only: false,
        include_paths: vec![],
        exclude_paths: vec![],
    };
    let added = SpaceSourceConfig { id: "added".into(), path: PathBuf::from("added"), ..existing.clone() };
    fs::write(
        dot_notez.join("sources.json"),
        serde_json::to_vec(&LegacySources { sources: vec![existing.clone(), added.clone()] }).unwrap(),
    )
    .unwrap();
    let community = Community {
        id: "all".into(),
        name: "All".into(),
        selector: Selector::new(),
        pinned_members: vec![],
        excluded_members: vec![],
    };
    fs::write(
        dot_notez.join("communities.json"),
        serde_json::to_vec(&LegacyCommunities { communities: vec![community.clone()] }).unwrap(),
    )
    .unwrap();
    let current = SpaceConfig { sources: vec![existing], ..minimal_space("personal") };
    let plan = plan_legacy_migration(dir.path(), &current).unwrap();
    assert_eq!(plan.sources_to_add.len(), 1);
    assert_eq!(plan.sources_to_add[0].id, "added");
    assert_eq!(plan.communities_to_extract, vec![community]);

    fs::write(dot_notez.join("sources.json"), "not json").unwrap();
    fs::write(dot_notez.join("communities.json"), "not json").unwrap();
    let empty = plan_legacy_migration(dir.path(), &current).unwrap();
    assert!(empty.sources_to_add.is_empty());
    assert!(empty.communities_to_extract.is_empty());
}

#[test]
fn migration_apply_writes_new_sources_noops_empty_plan_and_reports_io_errors() {
    let dir = tempfile::tempdir().unwrap();
    let config = minimal_space("personal");
    apply_legacy_migration(dir.path(), config.clone(), MigrationPlan::default()).unwrap();
    assert!(!dir.path().join("notez.toml").exists());

    let source = SpaceSourceConfig {
        id: "native".into(),
        kind: SourceKind::Native,
        path: PathBuf::from("notes"),
        read_only: false,
        include_paths: vec![],
        exclude_paths: vec![],
    };
    apply_legacy_migration(
        dir.path(),
        config.clone(),
        MigrationPlan { sources_to_add: vec![source], communities_to_extract: vec![] },
    )
    .unwrap();
    let written = SpaceConfig::parse(&fs::read_to_string(dir.path().join("notez.toml")).unwrap()).unwrap();
    assert_eq!(written.sources[0].id, "native");

    let blocker = dir.path().join("blocker");
    fs::write(&blocker, b"file").unwrap();
    let error = apply_legacy_migration(
        &blocker,
        config,
        MigrationPlan {
            sources_to_add: vec![SpaceSourceConfig {
                id: "bad".into(),
                kind: SourceKind::Native,
                path: PathBuf::from("bad"),
                read_only: false,
                include_paths: vec![],
                exclude_paths: vec![],
            }],
            communities_to_extract: vec![],
        },
    )
    .unwrap_err();
    assert!(matches!(error, ConfigError::Invalid("io", _)));
}
