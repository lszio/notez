//! Contract tests for `core::config::web_space`.
//!
//! The web reader depends on these helpers to populate its source picker
//! and to validate user-typed paths. Tests cover:
//! - happy paths (registered source, explicit path, toml-file path, dot-nopez path)
//! - error variants (not found, not a source)
//! - the empty / no-global-config case so the UI can degrade gracefully

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use notez_core::config::model::SourceRegistration;
use notez_core::config::web_space::{
    ListedSource, SourceOrigin, WebSourceError, list_sources, resolve_source,
};

fn env_with(config_home: &Path) -> BTreeMap<String, OsString> {
    BTreeMap::from([(
        "XDG_CONFIG_HOME".to_string(),
        config_home.as_os_str().to_os_string(),
    )])
}

fn write_global(config_home: &Path, body: &str) {
    let dir = config_home.join("notez");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.toml"), body).unwrap();
}

fn write_source_dir(source_root: &Path, name: &str) {
    fs::create_dir_all(source_root).unwrap();
    fs::write(
        source_root.join("notez.toml"),
        format!("version = 2\n\n[source]\nname = \"{name}\"\ndatabase = \".notez/index.sqlite\"\n"),
    )
    .unwrap();
}

#[test]
fn list_sources_returns_empty_when_no_global_config() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    assert!(list_sources(&env_with(&cfg), &cwd).unwrap().is_empty());
}

#[test]
fn list_sources_returns_named_registrations_with_expanded_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    write_global(
        &cfg,
        r#"
version = 2
default_source = "work"

[sources.work]
path = "/srv/notes/work"

[sources.personal]
path = "/srv/notes/personal"
"#,
    );

    let names: Vec<String> = list_sources(&env_with(&cfg), &cwd)
        .unwrap()
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(names, vec!["personal".to_string(), "work".to_string()]);

    let work = list_sources(&env_with(&cfg), &cwd)
        .unwrap()
        .into_iter()
        .find(|r| r.name == "work")
        .unwrap();
    assert_eq!(work.root, PathBuf::from("/srv/notes/work"));
    assert!(matches!(work.origin, SourceOrigin::Registered(_)));
}

#[test]
fn list_sources_tolerates_unreadable_global_config() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    write_global(&cfg, "version = 999\n");
    assert!(list_sources(&env_with(&cfg), &cwd).unwrap().is_empty());
}

#[test]
fn resolve_source_returns_not_found_for_missing_path() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("does-not-exist");
    match resolve_source(&missing) {
        Err(WebSourceError::NotFound(p)) => assert_eq!(p, missing.display().to_string()),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn resolve_source_accepts_plain_directory_as_fresh_workspace() {
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("plain");
    fs::create_dir_all(&plain).unwrap();
    let sel = resolve_source(&plain).expect("plain directory resolves as a fresh source");
    assert_eq!(sel.source_name, "plain");
    assert_eq!(sel.root, plain);
}

#[test]
fn resolve_source_accepts_directory_with_notez_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("work");
    write_source_dir(&source, "work");

    let sel = resolve_source(&source).expect("valid source dir resolves");
    assert_eq!(sel.source_name, "work");
    assert_eq!(sel.root, source);
}

#[test]
fn registered_source_equality_is_by_origin() {
    let reg = SourceRegistration {
        path: PathBuf::from("/srv/work"),
        config: None,
    };
    let s1 = ListedSource {
        name: "work".into(),
        root: PathBuf::from("/srv/work"),
        origin: SourceOrigin::Registered(reg),
    };
    let s2 = ListedSource {
        name: "work".into(),
        root: PathBuf::from("/srv/work"),
        origin: SourceOrigin::Discovered,
    };
    assert_ne!(s1, s2, "different origins must not compare equal");
    let s3 = s1.clone();
    assert_eq!(s1, s3);
}
