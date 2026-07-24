use std::process::Command;

fn notez() -> Command {
    Command::new(env!("CARGO_BIN_EXE_notez"))
}

fn fixture(tmp: &std::path::Path) {
    std::fs::write(
        tmp.join("first.org"),
        "* Heading\n:PROPERTIES:\n:ID: 01J0000000000000000000000A\n:END:\n\nbody\n\n[[Second Note]] and [[https://example.com]] after.\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("second.md"),
        "---\nid: 01J0000000000000000000000B\n---\n# Second Note\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("second_alias.md"),
        "---\nid: 01J0000000000000000000000C\n---\n# Second Note\n",
    )
    .unwrap();
}

#[test]
fn cli_link_diagnose_returns_diagnostics_json() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let scan = notez()
        .current_dir(tmp.path())
        .arg("scan")
        .output()
        .unwrap();
    assert!(scan.status.success(), "scan failed: {scan:?}");

    let out = notez()
        .current_dir(tmp.path())
        .arg("link")
        .arg("diagnose")
        .arg("heading:01J0000000000000000000000A")
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "diagnose failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let arr = value.as_array().expect("diagnose output is an array");
    assert!(!arr.is_empty(), "expected at least one diagnostic");
}

#[test]
fn cli_link_reindex_returns_counts_json() {
    let tmp = tempfile::tempdir().unwrap();
    fixture(tmp.path());
    let scan = notez()
        .current_dir(tmp.path())
        .arg("scan")
        .output()
        .unwrap();
    assert!(scan.status.success(), "scan failed: {scan:?}");
    let out = notez()
        .current_dir(tmp.path())
        .arg("link")
        .arg("reindex")
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .output()
        .unwrap();
    assert!(out.status.success(), "reindex failed: {out:?}");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(value.get("scanned").is_some());
    assert!(value.get("external").is_some());
}
