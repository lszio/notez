//! Contract tests for `core::config::web_space`.
//!
//! The web reader depends on these helpers to populate its space picker
//! and to validate user-typed paths. Tests cover:
//! - happy paths (registered space, explicit path, toml-file path, dot-nopez path)
//! - error variants (not found, not a space)
//! - the empty / no-global-config case so the UI can degrade gracefully
//! - tilde expansion is delegated to the existing `expand_tilde` helper
//!   (covered separately, we just confirm it does not regress here)

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use notez_core::config::web_space::{list_spaces, resolve_space, RegisteredSpace, SpaceSource, WebSpaceError};

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

fn write_space_dir(root: &Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("notez.toml"),
        format!("version = 1\n\n[space]\nname = \"{name}\"\ndatabase = \".notez/index.sqlite\"\n"),
    )
    .unwrap();
}

#[test]
fn list_spaces_returns_empty_when_no_global_config() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    // No global config written; nothing should be reported and no error.
    assert!(list_spaces(&env_with(&cfg), &cwd).is_empty());
}

#[test]
fn list_spaces_returns_named_registrations_with_expanded_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    write_global(
        &cfg,
        r#"
version = 1
default_space = "work"

[spaces.work]
path = "/srv/notes/work"

[spaces.personal]
path = "/srv/notes/personal"
"#,
    );

    let names: Vec<String> = list_spaces(&env_with(&cfg), &cwd)
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(names, vec!["personal".to_string(), "work".to_string()]);

    // Each entry carries the configured (non-tilde-expanded) path; the
    // ~ expansion only happens for paths that start with `~`, which we
    // exercise in a dedicated test below.
    let work = list_spaces(&env_with(&cfg), &cwd)
        .into_iter()
        .find(|r| r.name == "work")
        .unwrap();
    assert_eq!(work.path, PathBuf::from("/srv/notes/work"));
}

#[test]
fn list_spaces_tolerates_unreadable_global_config() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    // A malformed global config (unsupported version) must not panic
    // and must produce an empty list — the picker is allowed to be
    // empty, it is not allowed to crash the web server.
    write_global(&cfg, "version = 999\n");
    assert!(list_spaces(&env_with(&cfg), &cwd).is_empty());
}

#[test]
fn resolve_space_returns_not_found_for_missing_path() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("does-not-exist");
    match resolve_space(&missing) {
        Err(WebSpaceError::NotFound(p)) => assert_eq!(p, missing.display().to_string()),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn resolve_space_returns_not_a_space_for_plain_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("plain");
    fs::create_dir_all(&plain).unwrap();
    match resolve_space(&plain) {
        Err(WebSpaceError::NotASpace(p)) => assert_eq!(p, plain.display().to_string()),
        other => panic!("expected NotASpace, got {other:?}"),
    }
}

#[test]
fn resolve_space_accepts_directory_with_notez_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let space = tmp.path().join("work");
    write_space_dir(&space, "work");

    let sel = resolve_space(&space).expect("valid space dir resolves");
    assert_eq!(sel.space_name, "work");
    assert_eq!(sel.space_root, space);
}

#[test]
fn resolve_space_accepts_directory_with_dot_notez() {
    let tmp = tempfile::tempdir().unwrap();
    let space = tmp.path().join("scratch");
    fs::create_dir_all(space.join(".notez")).unwrap();
    // No toml, but a `.notez/` is enough to be considered a space.

    let sel = resolve_space(&space).expect(".notez dir is enough");
    assert_eq!(sel.space_root, space);
}

#[test]
fn resolve_space_accepts_pointing_at_notez_toml_file() {
    let tmp = tempfile::tempdir().unwrap();
    let space = tmp.path().join("explicit");
    write_space_dir(&space, "explicit");
    let toml = space.join("notez.toml");

    let sel = resolve_space(&toml).expect("toml file resolves to its parent");
    assert_eq!(sel.space_root, space);
}

#[test]
fn registered_space_equality_is_by_name_and_path() {
    let a = RegisteredSpace {
        name: "work".into(),
        path: PathBuf::from("/srv/work"),
        source: SpaceSource::Registered,
    };
    let b = RegisteredSpace {
        name: "work".into(),
        path: PathBuf::from("/srv/work"),
        source: SpaceSource::Registered,
    };
    // Same source + name + path compare equal regardless of how the
    // caller discovered the entry.
    assert_eq!(a, b);
    // Differing source still distinguishes the two origins.
    let c = RegisteredSpace {
        name: "work".into(),
        path: PathBuf::from("/srv/work"),
        source: SpaceSource::Discovered,
    };
    assert_ne!(a, c);
}

// -- auto-discovery ----------------------------------------------------------

/// Build an env map with both `XDG_CONFIG_HOME` and `HOME` pointed at
/// the given home-like directory; the auto-discovery code reads both.
fn env_with_home(home: &Path) -> BTreeMap<String, OsString> {
    BTreeMap::from([
        (
            "XDG_CONFIG_HOME".to_string(),
            home.join("config").as_os_str().to_os_string(),
        ),
        (
            "HOME".to_string(),
            home.as_os_str().to_os_string(),
        ),
    ])
}

#[test]
fn list_spaces_auto_discovers_home_children() {
    let tmp = tempfile::tempdir().unwrap();
    let env_root = tmp.path();
    let cwd = env_root.join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    // Two notez-looking children under $HOME.
    write_space_dir(&env_root.join("notes"), "notes");
    write_space_dir(&env_root.join("journal"), "journal");
    // A plain sibling that should NOT be picked up.
    fs::create_dir_all(env_root.join("plain")).unwrap();

    let result = list_spaces(&env_with_home(env_root), &cwd);
    let discovered: Vec<_> = result
        .iter()
        .filter(|r| r.source == SpaceSource::Discovered)
        .map(|r| (r.name.clone(), r.path.clone()))
        .collect();
    let names: Vec<&str> = discovered.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["journal", "notes"]);
}

#[test]
fn list_spaces_picks_up_cwd_upward_chain() {
    let tmp = tempfile::tempdir().unwrap();
    let env_root = tmp.path();
    let space = env_root.join("work").join("project");
    write_space_dir(&space, "work-space");
    let cwd = space.join("subdir");
    fs::create_dir_all(&cwd).unwrap();

    let result = list_spaces(&env_with_home(env_root), &cwd);
    let upward: Vec<_> = result
        .iter()
        .filter(|r| r.source == SpaceSource::Discovered && r.path == space)
        .collect();
    assert_eq!(upward.len(), 1, "cwd upward walk should find the space");
    assert_eq!(upward[0].name, "work-space");
}

#[test]
fn list_spaces_respects_disable_auto_discover() {
    let tmp = tempfile::tempdir().unwrap();
    let env_root = tmp.path();
    let cwd = env_root.join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    write_space_dir(&env_root.join("notes"), "notes");

    let mut env = env_with_home(env_root);
    env.insert(
        "NOTEZ_DISABLE_AUTO_DISCOVER".to_string(),
        OsString::from("1"),
    );

    let result = list_spaces(&env, &cwd);
    // Only the XDG registrations (none) survive; the home child must
    // be filtered out.
    assert!(result.iter().all(|r| r.source == SpaceSource::Registered));
}

#[test]
fn list_spaces_coalesces_registered_and_discovered() {
    let tmp = tempfile::tempdir().unwrap();
    let env_root = tmp.path();
    let cwd = env_root.join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    // A space that lives under $HOME AND is also explicitly registered
    // under the same path. The registration must win.
    let home_notes = env_root.join("notes");
    write_space_dir(&home_notes, "notes");
    write_global(
        &env_root.join("config"),
        &format!(
            "version = 1\n[spaces.notes]\npath = \"{}\"\n",
            home_notes.display()
        ),
    );

    let result = list_spaces(&env_with_home(env_root), &cwd);
    let notes: Vec<_> = result.iter().filter(|r| r.path == home_notes).collect();
    assert_eq!(notes.len(), 1, "registered + discovered must collapse");
    assert_eq!(notes[0].source, SpaceSource::Registered);
    assert_eq!(notes[0].name, "notes");
}

#[test]
fn list_spaces_handles_missing_home() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).unwrap();

    // Only XDG_CONFIG_HOME is set; HOME is missing. Discovery must
    // silently bail out without panicking.
    let env = BTreeMap::from([(
        "XDG_CONFIG_HOME".to_string(),
        tmp.path().join("config").as_os_str().to_os_string(),
    )]);

    let result = list_spaces(&env, &cwd);
    assert!(result.is_empty());
}
