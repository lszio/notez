use assert_cmd::Command;
use std::fs;

fn notez() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn config_cli_loads_registered_space() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tmp.path().join("config");
    let space = tmp.path().join("notes");
    
    fs::create_dir_all(&space).unwrap();
    fs::create_dir_all(xdg.join("notez")).unwrap();
    fs::write(
        xdg.join("notez/config.toml"),
        format!(
            "version = 1\ndefault_space = \"test\"\n[spaces.test]\npath = \"{}\"\n",
            space.display()
        )
    ).unwrap();
    
    // Explicit name resolution.
    notez()
        .env("XDG_CONFIG_HOME", xdg.as_os_str())
        .arg("--space")
        .arg("test")
        .arg("query")
        .arg("--json")
        .assert()
        .success();
        
    // Default space resolution.
    notez()
        .env("XDG_CONFIG_HOME", xdg.as_os_str())
        .arg("query")
        .arg("--json")
        .assert()
        .success();
        
    // Upward resolution.
    fs::write(space.join("notez.toml"), "version = 1\n[space]\nname = \"local\"\n").unwrap();
    fs::create_dir_all(space.join("deep/dir")).unwrap();
    notez()
        .env("XDG_CONFIG_HOME", xdg.as_os_str())
        .current_dir(space.join("deep/dir"))
        .arg("query")
        .arg("--json")
        .assert()
        .success();
}

#[test]
fn config_cli_rejects_invalid_config_before_db_open() {
    let tmp = tempfile::tempdir().unwrap();
    let xdg = tmp.path().join("config");
    let space = tmp.path().join("notes");
    
    fs::create_dir_all(&space).unwrap();
    fs::create_dir_all(xdg.join("notez")).unwrap();
    fs::write(
        xdg.join("notez/config.toml"),
        "version = 1\n[spaces.x]\npath = \"/nonexistent\"\nunknown = 1\n"
    ).unwrap();
    
    notez()
        .env("XDG_CONFIG_HOME", xdg.as_os_str())
        .arg("--space")
        .arg("x")
        .arg("query")
        .assert()
        .failure()
        .code(2);
        
    // Assert no .notez was created locally
    assert!(!space.join(".notez").exists());
}
