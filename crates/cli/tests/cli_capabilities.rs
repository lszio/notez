//! Equivalence contract for the public capability listing at the CLI
//! transport boundary.
//!
//! The CLI `list-capabilities` subcommand and the MCP `list_capabilities`
//! tool must both surface the same JSON payload for a given facade.
//! This test drives the CLI subcommand end-to-end and asserts the
//! payload shape that the MCP tool mirrors.

use assert_cmd::Command;
use serde_json::Value;

fn notez() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn list_capabilities_subcommand_emits_builtins_json() {
    let output = notez()
        .arg("list-capabilities")
        .output()
        .expect("spawn notez");
    assert!(
        output.status.success(),
        "list-capabilities exit code: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let payload: Value = serde_json::from_str(stdout.trim()).expect("list-capabilities emits JSON");
    let entries = payload
        .as_array()
        .expect("top-level JSON value is an array");
    assert_eq!(entries.len(), 9, "nine built-in use-case capabilities");

    let mut ids: Vec<&str> = entries
        .iter()
        .map(|e| e["id"].as_str().expect("id is string"))
        .collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![
            "artifact",
            "attachment",
            "community",
            "inspect",
            "link",
            "resource",
            "scan",
            "sync",
            "task",
        ]
    );

    for entry in entries {
        let obj = entry.as_object().expect("entry is an object");
        assert_eq!(obj.len(), 3, "each entry has exactly three keys");
        let mutability = obj["mutability"].as_str().expect("mutability is string");
        assert!(
            mutability == "read" || mutability == "write",
            "unexpected mutability `{mutability}`"
        );
    }
}
