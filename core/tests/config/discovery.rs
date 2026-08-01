use crate::config::{ConfigPaths, SpaceSelector, select_space};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

fn env_with(home: &str) -> BTreeMap<String, OsString> {
    let mut env = BTreeMap::new();
    env.insert("HOME".into(), OsString::from(home));
    env.insert(
        "XDG_CONFIG_HOME".into(),
        OsString::from(format!("{home}/.config")),
    );
    env
}

#[test]
fn explicit_registered_name_is_preferred() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join("notes/personal")).unwrap();
    std::fs::write(
        root.join("notes/personal/note.org"),
        "#+title: note\n#+ID: 01J000000000000000000000A1\n",
    )
    .unwrap();

    let global_dir = root.join(".config/notez");
    std::fs::create_dir_all(&global_dir).unwrap();
    std::fs::write(
        global_dir.join("config.toml"),
        format!(
            "version = 1\ndefault_space = \"personal\"\n\n[spaces.personal]\npath = \"{}\"\n",
            root.join("notes/personal").display()
        ),
    )
    .unwrap();

    let paths = ConfigPaths::discover(&env_with(root.to_str().unwrap()), root.join("anywhere"))
        .unwrap();
    let space = select_space(&paths, SpaceSelector::Name("personal")).unwrap();
    assert_eq!(space.space_name, "personal");
    assert_eq!(
        space.space_root.canonicalize().unwrap(),
        root.join("notes/personal").canonicalize().unwrap()
    );
}


#[test]
fn upward_search_finds_nearest_notez_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let nested = root.join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();
    let target = root.join("a/b/c/d/e");
    std::fs::write(
        nested.join("notez.toml"),
        "version = 1\n[space]\nname = \"nested\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(&target).unwrap();

    let paths = ConfigPaths::discover(
        &env_with(root.to_str().unwrap()),
        nested.join("d/e"),
    )
    .unwrap();
    let space = select_space(&paths, SpaceSelector::Upward).unwrap();
    assert_eq!(space.space_root, nested);
    assert_eq!(space.space_name, "nested");
}

#[test]
fn missing_space_returns_clear_error() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let paths = ConfigPaths::discover(
        &env_with(root.to_str().unwrap()),
        root.join("anywhere"),
    )
    .unwrap();
    let err = select_space(&paths, SpaceSelector::Default).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("no space selected"), "msg = {msg}");
}
