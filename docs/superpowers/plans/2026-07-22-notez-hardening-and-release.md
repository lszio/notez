# Notez Hardening and Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Space Doctor (`notez space doctor`), Job Manager (`notez job list`), Artifact Freshness Checker (`notez artifact stale`), Security Sanity & Path Traversal Guards, comprehensive contract/golden tests, and full release acceptance verification (`scripts/acceptance-release.sh`).

**Architecture:** Add `doctor`, `job_manager`, and `security` modules in `application` / `document`. Expose `space doctor`, `job list`, `artifact stale` on CLI and MCP stdio server. Finalize end-to-end release acceptance suite covering all 5 acceptance scripts.

**Tech Stack:** Rust 1.94, Cargo workspace, serde, thiserror, ulid, rusqlite, clap, walkdir.

---

### Task 1: Space Doctor and Integrity Diagnostics

**Files:**
- Create: `crates/application/src/doctor.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/doctor.rs`

**Interfaces:**
- Produces: `ApplicationService::space_doctor`, `DoctorReport`, `DoctorIssue`.

- [ ] **Step 1: Write failing space doctor test**

Create `crates/application/tests/doctor.rs`: test detecting missing files, broken ID links, corrupt sqlite index entries, and returning structured `DoctorReport`.

- [ ] **Step 2: Run test and verify missing doctor failure**

Run: `cargo test -p application --test doctor`
Expected: FAIL because `space_doctor` is missing.

- [ ] **Step 3: Implement Space Doctor**

Implement `space_doctor` verifying file existence, broken link references, unindexed documents, and database integrity checks.

- [ ] **Step 4: Run application tests**

Run: `cargo test -p application`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: add space doctor integrity diagnostics"
```

---

### Task 2: Persistent Job Manager and Artifact Freshness Checker

**Files:**
- Create: `crates/application/src/job_manager.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/job_manager.rs`

**Interfaces:**
- Produces: `ApplicationService::list_jobs`, `ApplicationService::check_artifact_freshness`, `ArtifactStaleReport`.

- [ ] **Step 1: Write failing job manager & freshness test**

Create `crates/application/tests/job_manager.rs`: test listing extraction/scan background jobs and checking whether derived artifacts are stale compared to raw files.

- [ ] **Step 2: Run job manager test**

Run: `cargo test -p application --test job_manager`
Expected: FAIL because `list_jobs` or `check_artifact_freshness` is missing.

- [ ] **Step 3: Implement Job Manager and Artifact Freshness Checker**

Implement `JobRecord` tracker, `list_jobs`, and `check_artifact_freshness` comparing document revisions to derived artifact revisions.

- [ ] **Step 4: Run application tests**

Run: `cargo test -p application`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: implement job manager and artifact freshness checker"
```

---

### Task 3: Security Guards and Path Traversal Protections

**Files:**
- Create: `crates/document/src/security.rs`
- Modify: `crates/document/src/lib.rs`
- Test: `crates/document/tests/security.rs`

**Interfaces:**
- Produces: `SecurityGuard::sanitize_path`, `SecurityGuard::validate_attachment_size`.

- [ ] **Step 1: Write failing security guard test**

Create `crates/document/tests/security.rs`: test path traversal prevention (e.g. `../../etc/passwd`), symlink escape protection, and attachment size limits.

- [ ] **Step 2: Run security test**

Run: `cargo test -p document --test security`
Expected: FAIL because `SecurityGuard` is missing.

- [ ] **Step 3: Implement SecurityGuard**

Implement `SecurityGuard` checking path canonicalization within space boundaries, rejecting path traversal attempts, and enforcing 100MB max attachment size limit.

- [ ] **Step 4: Run document tests**

Run: `cargo test -p document`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/document
git commit -m "feat: implement security guards for path traversal and attachment limits"
```

---

### Task 4: CLI and MCP Extensions for Doctor, Jobs, and Artifact Freshness

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/cli/tests/doctor_cli.rs`
- Test: `crates/mcp/tests/doctor_mcp.rs`

**Interfaces:**
- Produces: CLI commands `space doctor`, `job list`, `artifact stale`; MCP tools `space_doctor`, `job_list`, `artifact_stale`.

- [ ] **Step 1: Write failing CLI and MCP doctor tests**

Test CLI `notez space doctor`, `notez job list`, `notez artifact stale`; test corresponding MCP tools.

- [ ] **Step 2: Run CLI and MCP doctor tests**

Run: `cargo test -p cli --test doctor_cli` and `cargo test -p mcp --test doctor_mcp`
Expected: FAIL because doctor/job subcommands/tools are missing.

- [ ] **Step 3: Implement CLI subcommands and MCP tools**

Add clap `space doctor`, `job list`, `artifact stale` subcommands; register `space_doctor`, `job_list`, `artifact_stale` tools in MCP stdio server.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli crates/mcp
git commit -m "feat: expose space doctor, job list, and artifact stale over cli and mcp"
```

---

### Task 5: Comprehensive Workspace Golden and Property Test Suite

**Files:**
- Create: `tests/golden_suite.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Produces: Golden test suite verifying Org/Markdown round-trip formatting locality, selector compositions, and ID stability across all features.

- [ ] **Step 1: Write golden and property test suite**

Create `tests/golden_suite.rs`: test lossless Org & Markdown round-trips, selector compositions, and error code mappings.

- [ ] **Step 2: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests
git commit -m "test: add golden and property test suite for workspace"
```

---

### Task 6: Release Acceptance Script, Documentation, and Final Release Cut

**Files:**
- Create: `scripts/acceptance-release.sh`
- Modify: `README.md`

**Interfaces:**
- Produces: `scripts/acceptance-release.sh` running all 5 acceptance test scripts and verifying clean zero-warning builds.

- [ ] **Step 1: Write release acceptance script**

The script executes `acceptance-core.sh`, `acceptance-rules.sh`, `acceptance-federation.sh`, `acceptance-attachments.sh`, `acceptance-artifacts.sh`, and `acceptance-sync.sh`, verifying `release acceptance: PASS`.

- [ ] **Step 2: Run release acceptance script**

Run: `bash scripts/acceptance-release.sh`
Expected: PASS (`release acceptance: PASS`).

- [ ] **Step 3: Update README.md**

Finalize user-facing documentation covering all features, CLI commands, MCP tools, exit codes, and operational usage.

- [ ] **Step 4: Final verification checks**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace`
Run: `bash scripts/acceptance-release.sh`
Expected: All exit 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts docs
git commit -m "release: cut notez 0.1.0 baseline with full acceptance test suite"
```
