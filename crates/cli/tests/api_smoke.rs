//! Comprehensive smoke test contract for every top-level `notez` CLI
//! command. Each test exercises the binary end-to-end against a temporary
//! space and asserts the exit code together with a key stdout / stderr
//! substring.
//!
//! The goal is to lock in the public CLI surface so that future refactors
//! cannot quietly drop a command or change its meaning without these
//! tests going red.
//!
//! Conventions:
//! - All paths live in `tempfile::TempDir` instances; no state survives
//!   between tests.
//! - Org-mode fixtures are used as the canonical native source; a single
//!   markdown fixture is included for cross-adapter coverage.
//! - The binary under test is the `notez` crate binary resolved through
//!   `assert_cmd::Command::cargo_bin`.
//! - Per the task contract, the per-crate runner is
//!   `cargo test -p cli --test api_smoke`. Do not run the entire
//!   workspace from this test file.

use assert_cmd::Command;
use predicates::str;
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn notez() -> Command {
    Command::cargo_bin("notez").unwrap()
}

/// Write the canonical Org-mode + Markdown fixtures into `space`.
fn seed_space(space: &Path) {
    fs::write(
        space.join("alpha.org"),
        "#+title: Alpha Architecture\n#+ID: 01J000000000000000000000A1\n\
         * NEXT Draft alpha section\n\
         :PROPERTIES:\n\
         :ID: 01J000000000000000000000A2\n\
         :TODO: NEXT\n\
         :SCHEDULED: <2026-07-22 Wed>\n\
         :END:\n\
         body of alpha.\n",
    )
    .unwrap();
    fs::write(
        space.join("beta.org"),
        "#+title: Beta Architecture\n#+ID: 01J000000000000000000000B1\n\
         * DONE Finalize beta\n\
         :PROPERTIES:\n\
         :ID: 01J000000000000000000000B2\n\
         :TODO: DONE\n\
         :END:\n",
    )
    .unwrap();
    fs::write(
        space.join("gamma.md"),
        "---\nid: 01J000000000000000000000C1\ntitle: Gamma Notes\n---\n\
         # Gamma Notes\n\
         Some markdown body.\n",
    )
    .unwrap();
}

/// Scan once so subsequent commands have a populated projection.
fn scan(space: &Path) {
    notez()
        .arg("--space")
        .arg(space)
        .arg("scan")
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Public read-path commands — these must always succeed once the space has
// been scanned.
// ---------------------------------------------------------------------------

#[test]
fn smoke_scan_reports_scanned_files() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("scan")
        .assert()
        .success()
        .stdout(str::contains("\"scanned_files\":3"));
}

#[test]
fn smoke_query_filters_by_kind_and_title() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("query")
        .arg("--kind")
        .arg("heading")
        .arg("--title-contains")
        .arg("Draft")
        .assert()
        .success()
        .stdout(str::contains("Draft alpha section"));
}

#[test]
fn smoke_resolve_returns_resource_ref() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("resolve")
        .arg("heading:01J000000000000000000000A2")
        .assert()
        .success()
        .stdout(str::contains("heading:01J000000000000000000000A2"));
}

#[test]
fn smoke_read_returns_full_resource() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("read")
        .arg("heading:01J000000000000000000000A2")
        .assert()
        .success()
        .stdout(str::contains("\"title\":\"Draft alpha section\""));
}

#[test]
fn smoke_inspect_returns_resource_details() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("inspect")
        .arg("heading:01J000000000000000000000A2")
        .assert()
        .success()
        .stdout(str::contains("heading:01J000000000000000000000A2"));
}

#[test]
fn smoke_inspect_rules_returns_classified_type() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("inspect")
        .arg("heading:01J000000000000000000000A2")
        .arg("--rules")
        .assert()
        .success()
        .stdout(str::contains("\"classified_type\""));
}

#[test]
fn smoke_agenda_returns_items() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("agenda")
        .assert()
        .success()
        .stdout(str::contains("Draft alpha section"));
}

#[test]
fn smoke_recent_returns_recent_items() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("recent")
        .arg("--limit")
        .arg("10")
        .assert()
        .success()
        .stdout(str::contains("\"ref\""));
}

// ---------------------------------------------------------------------------
// Resource mutation commands.
// ---------------------------------------------------------------------------

#[test]
fn smoke_resource_upsert_then_delete_via_json_file() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    // Upsert a fresh heading directly into the projection.
    let payload = json!({
        "ref": "heading:01J000000000000000000000D1",
        "kind": "heading",
        "title": "Injected Heading",
        "revision": "rev1",
        "source_id": "native",
        "locator": "/injected.org",
        "properties": {
            "TODO": "TODO",
        },
    });
    let payload_path = tmp.path().join("resource.json");
    fs::write(&payload_path, serde_json::to_vec_pretty(&payload).unwrap()).unwrap();

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("resource")
        .arg("upsert")
        .arg("--from")
        .arg(&payload_path)
        .assert()
        .success()
        .stdout(str::contains("heading:01J000000000000000000000D1"));

    // The projection must now contain the injected heading.
    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("query")
        .arg("--exact-ref")
        .arg("heading:01J000000000000000000000D1")
        .assert()
        .success()
        .stdout(str::contains("\"title\":\"Injected Heading\""));

    // Delete it again via the typed ref CLI command.
    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("resource")
        .arg("delete")
        .arg("heading:01J000000000000000000000D1")
        .assert()
        .success();

    // Deleting a missing ref is an idempotent no-op (no failure expected).
    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("resource")
        .arg("delete")
        .arg("heading:01J000000000000000000000D1")
        .assert()
        .success();
}

#[test]
fn smoke_resource_ls_lists_by_source() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("resource")
        .arg("ls")
        .arg("native")
        .assert()
        .success()
        .stdout(str::contains("\"ref\""));
}

// ---------------------------------------------------------------------------
// Link subcommands — all currently implemented, must surface success.
// ---------------------------------------------------------------------------

#[test]
fn smoke_link_list_returns_occurrences() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("note.org"),
        "#+title: Source Doc\n#+ID: 01J000000000000000000000E1\n\
         * Section link\n\
         :PROPERTIES:\n\
         :ID: 01J000000000000000000000E2\n\
         :END:\n\
         See [[Target Doc]] for details.\n",
    )
    .unwrap();
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("link")
        .arg("list")
        .arg("heading:01J000000000000000000000E2")
        .assert()
        .success()
        .stdout(str::contains("\"source_ref\""));
}

#[test]
fn smoke_link_resolve_returns_relations() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("note.org"),
        "#+title: Source Doc\n#+ID: 01J000000000000000000000F1\n\
         * Section link\n\
         :PROPERTIES:\n\
         :ID: 01J000000000000000000000F2\n\
         :END:\n\
         See [[Target Doc]] for details.\n",
    )
    .unwrap();
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("link")
        .arg("resolved")
        .arg("heading:01J000000000000000000000F2")
        .assert()
        .success()
        .stdout(str::contains("["));
}

#[test]
fn smoke_link_diagnose_returns_diagnostics() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join("note.org"),
        "#+title: Source Doc\n#+ID: 01J000000000000000000000G1\n\
         * Section link\n\
         :PROPERTIES:\n\
         :ID: 01J000000000000000000000G2\n\
         :END:\n\
         See [[Target Doc]] for details.\n",
    )
    .unwrap();
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("link")
        .arg("diagnose")
        .arg("heading:01J000000000000000000000G2")
        .assert()
        .success()
        .stdout(str::contains("["));
}

#[test]
fn smoke_link_reindex_returns_report() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("link")
        .arg("reindex")
        .assert()
        .success()
        .stdout(str::contains("\"scanned\""));
}

// ---------------------------------------------------------------------------
// Task subcommands.
// ---------------------------------------------------------------------------

#[test]
fn smoke_task_agenda_returns_items() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("task")
        .arg("agenda")
        .assert()
        .success()
        .stdout(str::contains("Draft alpha section"));
}

#[test]
fn smoke_task_list_groups_by_state() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("task")
        .arg("list")
        .assert()
        .success()
        .stdout(str::contains("items"));
}

#[test]
fn smoke_task_detail_reads_heading() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("task")
        .arg("detail")
        .arg("heading:01J000000000000000000000A2")
        .assert()
        .success()
        .stdout(str::contains("Draft alpha section"));
}

#[test]
fn smoke_task_para_overview_succeeds() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("task")
        .arg("para")
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Config subcommands.
// ---------------------------------------------------------------------------

#[test]
fn smoke_config_show_returns_runtime_config() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    let output = notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(stdout).unwrap();
    let source_name = json
        .get("source")
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");
    assert!(
        !source_name.is_empty(),
        "config show must include source.name"
    );
}

#[test]
fn smoke_config_validate_returns_valid() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("config")
        .arg("validate")
        .assert()
        .success()
        .stdout(str::contains("\"valid\":true"));
}

#[test]
fn smoke_config_migrate_preview_runs() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    // No legacy JSON files present; migration plan should still produce a
    // valid preview (zero counts) without touching the filesystem.
    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("config")
        .arg("migrate")
        .assert()
        .success()
        .stdout(str::contains("sources_to_add"));
}

// ---------------------------------------------------------------------------
// Source subcommands.
// ---------------------------------------------------------------------------

#[test]
fn smoke_source_add_then_list() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("source")
        .arg("add")
        .arg("--id")
        .arg("vault_src")
        .arg("--kind")
        .arg("obsidian")
        .arg("--path")
        .arg(tmp.path())
        .arg("--read-only")
        .assert()
        .success();

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("source")
        .arg("list")
        .assert()
        .success()
        .stdout(str::contains("vault_src"));
}

#[test]
fn smoke_source_sync_returns_report() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("source")
        .arg("add")
        .arg("--id")
        .arg("vault_src")
        .arg("--kind")
        .arg("obsidian")
        .arg("--path")
        .arg(tmp.path())
        .arg("--read-only")
        .assert()
        .success();

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("source")
        .arg("sync")
        .assert()
        .success()
        .stdout(str::contains("\"scanned_resources\""));
}

// ---------------------------------------------------------------------------
// Attachment subcommands.
// ---------------------------------------------------------------------------

#[test]
fn smoke_attachment_add_extract_segments() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    let sample = tmp.path().join("sample.txt");
    fs::write(
        &sample,
        "Attachment text content for api_smoke extraction testing.",
    )
    .unwrap();

    let output = notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("attachment")
        .arg("add")
        .arg("--path")
        .arg(&sample)
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let att_ref = parsed["ref"]
        .as_str()
        .expect("attachment add must return a ref");

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("attachment")
        .arg("extract")
        .arg(att_ref)
        .assert()
        .success()
        .stdout(str::contains("\"segments_count\""));

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("attachment")
        .arg("segments")
        .arg(att_ref)
        .assert()
        .success()
        .stdout(str::contains("api_smoke extraction testing"));
}

// ---------------------------------------------------------------------------
// Community, derive, skill subcommands.
// ---------------------------------------------------------------------------

#[test]
fn smoke_community_create_derive_skill() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("community")
        .arg("create")
        .arg("--id")
        .arg("api_comm")
        .arg("--name")
        .arg("API Comm")
        .arg("--title-contains")
        .arg("Architecture")
        .assert()
        .success();

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("community")
        .arg("list")
        .assert()
        .success()
        .stdout(str::contains("api_comm"));

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("derive")
        .arg("--community")
        .arg("api_comm")
        .arg("--recipe")
        .arg("summary")
        .assert()
        .success();

    let skill_out = tmp.path().join("skill_out");
    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("skill")
        .arg("export")
        .arg("--community")
        .arg("api_comm")
        .arg("--description")
        .arg("api smoke skill")
        .arg("--out")
        .arg(&skill_out)
        .assert()
        .success();

    assert!(skill_out.join("SKILL.md").exists());
}

// ---------------------------------------------------------------------------
// Sync subcommands. `push`/`pull` must succeed on a real folder exchange;
// `conflicts` lists persisted conflict records (empty on a fresh space).
// ---------------------------------------------------------------------------

#[test]
fn smoke_sync_push_and_pull() {
    let tmp = TempDir::new().unwrap();
    let space_a = tmp.path().join("a");
    let space_b = tmp.path().join("b");
    let shared = tmp.path().join("shared");
    fs::create_dir_all(&space_a).unwrap();
    fs::create_dir_all(&space_b).unwrap();
    fs::create_dir_all(&shared).unwrap();

    fs::write(
        space_a.join("shared.org"),
        "#+title: Shared\n#+ID: 01J000000000000000000000H1\n",
    )
    .unwrap();

    notez()
        .arg("--space")
        .arg(&space_a)
        .arg("--json")
        .arg("sync")
        .arg("push")
        .arg("--actor")
        .arg("actor_a")
        .arg("--folder")
        .arg(&shared)
        .assert()
        .success()
        .stdout(str::contains("\"pushed_files\":1"));

    notez()
        .arg("--space")
        .arg(&space_b)
        .arg("--json")
        .arg("sync")
        .arg("pull")
        .arg("--actor")
        .arg("actor_b")
        .arg("--folder")
        .arg(&shared)
        .assert()
        .success()
        .stdout(str::contains("\"pulled_files\":1"));
}

// ---------------------------------------------------------------------------
// Space administrative subcommands. `doctor` is an unsupported capability
// stub; `register`/`unregister`/`list`/`rebuild` must succeed.
// ---------------------------------------------------------------------------

#[test]
fn smoke_space_rebuild_succeeds() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("workspace")
        .arg("rebuild")
        .assert()
        .success()
        .stdout(str::contains("\"rebuilt\":true"));
}

#[test]
fn smoke_space_list_register_unregister() {
    let tmp = TempDir::new().unwrap();
    let space = tmp.path().join("notes");
    fs::create_dir_all(&space).unwrap();

    // Use an isolated XDG_CONFIG_HOME so we don't pollute the real one.
    let xdg = tmp.path().join("xdg");
    fs::create_dir_all(xdg.join("notez")).unwrap();

    notez()
        .env("XDG_CONFIG_HOME", xdg.as_os_str())
        .arg("--space")
        .arg(&space)
        .arg("--json")
        .arg("workspace")
        .arg("register")
        .arg("api_space")
        .arg("--path")
        .arg(&space)
        .assert()
        .success()
        .stdout(str::contains("\"registered\":\"api_space\""));

    notez()
        .env("XDG_CONFIG_HOME", xdg.as_os_str())
        .arg("--space")
        .arg(&space)
        .arg("--json")
        .arg("workspace")
        .arg("list")
        .assert()
        .success()
        .stdout(str::contains("api_space"));

    notez()
        .env("XDG_CONFIG_HOME", xdg.as_os_str())
        .arg("--space")
        .arg(&space)
        .arg("--json")
        .arg("workspace")
        .arg("unregister")
        .arg("api_space")
        .assert()
        .success()
        .stdout(str::contains("\"unregistered\":\"api_space\""));
}

// ---------------------------------------------------------------------------
// Unsupported capability stubs. These commands are intentionally wired to
// `ApplicationError::Unsupported` and must surface a non-zero exit code
// with a recognisable stderr substring.
// ---------------------------------------------------------------------------

#[test]
fn smoke_unsupported_space_doctor() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("workspace")
        .arg("doctor")
        .assert()
        .failure()
        .stderr(str::contains("unsupported"));
}

#[test]
fn smoke_unsupported_task_jobs() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("task")
        .arg("jobs")
        .assert()
        .failure()
        .stderr(str::contains("unsupported"));
}

#[test]
fn smoke_unsupported_artifact_stale() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("artifact")
        .arg("stale")
        .assert()
        .failure()
        .stderr(str::contains("unsupported"));
}

#[test]
fn smoke_unsupported_sync_relay() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("sync")
        .arg("relay")
        .arg("--id")
        .arg("any_src")
        .assert()
        .failure()
        .stderr(str::contains("unsupported"));
}

#[test]
fn smoke_sync_conflicts_lists_persisted_records() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());
    scan(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("sync")
        .arg("conflicts")
        .assert()
        .success()
        .stdout(str::contains("[]"));
}

#[test]
fn smoke_source_writeback_rejects_unregistered_source() {
    let tmp = TempDir::new().unwrap();
    seed_space(tmp.path());

    notez()
        .arg("--space")
        .arg(tmp.path())
        .arg("--json")
        .arg("source")
        .arg("writeback")
        .arg("--id")
        .arg("missing_src")
        .arg("--r-ref")
        .arg("heading:01J000000000000000000000A2")
        .arg("--payload")
        .arg("Updated title")
        .assert()
        .failure()
        .stderr(str::contains("no sources registered"));
}
