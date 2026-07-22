# Notez Folder Synchronization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement folder synchronization (`crates/sync`) with manifests, content-addressed objects, heads tracking, tombstones, 3-way merge engine, conflict inspection, and CLI/MCP sync commands.

**Architecture:** Create `crates/sync` with `Manifest`, `ObjectStore`, `Tombstone`, `ThreeWayMerger`, and `FolderTransport`. Update `ApplicationService` to manage folder sync pushes/pulls, merge concurrent changes, detect conflicts, and expose CLI/MCP sync endpoints.

**Tech Stack:** Rust 1.94, Cargo workspace, serde, sha2, thiserror, ulid, rusqlite, clap, walkdir.

---

### Task 1: Sync Crate, Manifest, and Object Store

**Files:**
- Create: `crates/sync/Cargo.toml`
- Create: `crates/sync/src/lib.rs`
- Create: `crates/sync/src/manifest.rs`
- Create: `crates/sync/src/object.rs`
- Test: `crates/sync/tests/manifest.rs`

**Interfaces:**
- Produces: `Manifest`, `SyncObject`, `ObjectStore`, `TombstoneRecord`.

- [ ] **Step 1: Write failing manifest and object store test**

Create `crates/sync/tests/manifest.rs`: test creating a `Manifest` with space ID, actor ID, parent snapshots, logical path, and content hash; test storing/retrieving encrypted objects and tombstones.

- [ ] **Step 2: Run test and verify missing sync crate failure**

Run: `cargo test -p sync --test manifest`
Expected: FAIL because `sync` crate is missing.

- [ ] **Step 3: Implement Manifest and ObjectStore**

Create `crates/sync/Cargo.toml` and implement `Manifest`, `SyncObject`, `ObjectStore`, and `TombstoneRecord`.

- [ ] **Step 4: Run sync tests**

Run: `cargo test -p sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/sync
git commit -m "feat: add sync crate with manifest and object store"
```

---

### Task 2: Heads Tracking and 3-Way Merge Engine

**Files:**
- Create: `crates/sync/src/merge.rs`
- Modify: `crates/sync/src/lib.rs`
- Test: `crates/sync/tests/merge.rs`

**Interfaces:**
- Produces: `ThreeWayMerger`, `MergeResult`, `ConflictRecord`, `HeadsTracker`.

- [ ] **Step 1: Write failing 3-way merge test**

Create `crates/sync/tests/merge.rs`: test fast-forward merge, non-overlapping 3-way text merge, and detecting concurrent conflict records on overlapping edits.

- [ ] **Step 2: Run merge test**

Run: `cargo test -p sync --test merge`
Expected: FAIL because `ThreeWayMerger` is missing.

- [ ] **Step 3: Implement ThreeWayMerger and HeadsTracker**

Implement fast-forward checks, line-level 3-way text merging, conflict marker generation, and `HeadsTracker` (`heads/<actor_id>`).

- [ ] **Step 4: Run sync tests**

Run: `cargo test -p sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sync
git commit -m "feat: implement heads tracking and 3-way merge engine"
```

---

### Task 3: Shared Folder Transport and Sync Engine

**Files:**
- Create: `crates/sync/src/transport.rs`
- Create: `crates/sync/src/engine.rs`
- Modify: `crates/sync/src/lib.rs`
- Test: `crates/sync/tests/folder_sync.rs`

**Interfaces:**
- Produces: `FolderTransport`, `SyncEngine::push`, `SyncEngine::pull`, `SyncEngine::sync`.

- [ ] **Step 1: Write failing folder sync test**

Create `crates/sync/tests/folder_sync.rs`: test syncing between two isolated device folders via a shared folder transport.

- [ ] **Step 2: Run folder sync test**

Run: `cargo test -p sync --test folder_sync`
Expected: FAIL because `FolderTransport` or `SyncEngine` is missing.

- [ ] **Step 3: Implement FolderTransport and SyncEngine**

Implement directory layout (`heads/`, `manifests/`, `objects/`, `tombstones/`), `FolderTransport` read/write operations, and `SyncEngine` push/pull routines.

- [ ] **Step 4: Run sync tests**

Run: `cargo test -p sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sync
git commit -m "feat: implement folder transport and sync engine"
```

---

### Task 4: Sync Application Services in ApplicationService

**Files:**
- Create: `crates/application/src/sync_app.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/sync_app.rs`

**Interfaces:**
- Produces: `ApplicationService::sync_push`, `ApplicationService::sync_pull`, `ApplicationService::list_conflicts`.

- [ ] **Step 1: Write failing sync application test**

Create `crates/application/tests/sync_app.rs`: test `sync_push` to shared folder, `sync_pull` on second space, and inspecting active conflicts.

- [ ] **Step 2: Run sync application test**

Run: `cargo test -p application --test sync_app`
Expected: FAIL because `sync_push` is missing.

- [ ] **Step 3: Implement Sync Application Services**

Add `sync = { path = "../sync" }` to `crates/application/Cargo.toml`; implement `sync_push`, `sync_pull`, and `list_conflicts`.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: integrate sync push, pull, and conflict inspection in application service"
```

---

### Task 5: CLI and MCP Extensions for Sync Management

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/cli/tests/sync_cli.rs`
- Test: `crates/mcp/tests/sync_mcp.rs`

**Interfaces:**
- Produces: CLI commands `sync push`, `sync pull`, `sync conflicts`; MCP tools `sync_push`, `sync_pull`, `sync_conflicts`.

- [ ] **Step 1: Write failing CLI and MCP sync tests**

Test CLI `notez sync push --folder /shared`, `notez sync pull --folder /shared`, `notez sync conflicts`; test corresponding MCP tools.

- [ ] **Step 2: Run CLI and MCP sync tests**

Run: `cargo test -p cli --test sync_cli` and `cargo test -p mcp --test sync_mcp`
Expected: FAIL because `sync` subcommands/tools are missing.

- [ ] **Step 3: Implement CLI subcommands and MCP tools**

Add clap `sync` subcommand family; register `sync_push`, `sync_pull`, `sync_conflicts` in MCP stdio server.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli crates/mcp
git commit -m "feat: expose sync push, pull, and conflicts over cli and mcp"
```

---

### Task 6: Acceptance Script, Documentation, and Final Verification

**Files:**
- Create: `scripts/acceptance-sync.sh`
- Modify: `README.md`

**Interfaces:**
- Produces: `scripts/acceptance-sync.sh` script testing folder sync between two devices, 3-way merge, rebuild, and conflict listing.

- [ ] **Step 1: Write sync acceptance script**

The script sets up two device spaces and one shared folder, executes push/pull sync, modifies files concurrently, inspects conflicts, rebuilds, and verifies output consistency.

- [ ] **Step 2: Run sync acceptance script**

Run: `bash scripts/acceptance-sync.sh`
Expected: PASS (`sync acceptance: PASS`).

- [ ] **Step 3: Update README.md**

Document Folder Sync architecture (`heads/`, `manifests/`, `objects/`, `tombstones/`), 3-way merge rules, and Sync CLI/MCP commands.

- [ ] **Step 4: Final verification checks**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace`
Run: `bash scripts/acceptance-core.sh`
Run: `bash scripts/acceptance-rules.sh`
Run: `bash scripts/acceptance-federation.sh`
Run: `bash scripts/acceptance-attachments.sh`
Run: `bash scripts/acceptance-artifacts.sh`
Run: `bash scripts/acceptance-sync.sh`
Expected: All exit 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts docs
git commit -m "test: add sync acceptance script and update documentation"
```
