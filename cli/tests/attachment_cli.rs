use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_attachment_add_extract_segments() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    let sample_file = space.join("sample.txt");
    fs::write(
        &sample_file,
        "Attachment file text content for CLI extraction testing.",
    )
    .unwrap();

    let mut cmd_add = notez_cmd();
    let output = cmd_add
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("attachment")
        .arg("add")
        .arg("--path")
        .arg(&sample_file)
        .assert()
        .success();

    let stdout_str = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json_res: serde_json::Value = serde_json::from_str(&stdout_str).unwrap();
    let att_ref = json_res["ref"].as_str().unwrap();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("attachment")
        .arg("extract")
        .arg(att_ref)
        .assert()
        .success()
        .stdout(predicates::str::contains("\"segments_count\":"));

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("attachment")
        .arg("segments")
        .arg(att_ref)
        .assert()
        .success()
        .stdout(predicates::str::contains("CLI extraction testing"));
}
