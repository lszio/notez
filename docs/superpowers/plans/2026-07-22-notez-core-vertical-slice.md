# Notez Core Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a runnable Rust `notez` binary that scans Org documents and headings into a rebuildable SQLite projection and exposes the same resolve/query/read operations through CLI and MCP stdio.

**Architecture:** A Cargo workspace separates domain, document, storage, application, MCP, and CLI packages under `crates/`. Files remain authoritative; SQLite is deleted and rebuilt in an end-to-end test. CLI and MCP depend only on `ApplicationService`.

**Tech Stack:** Rust 1.94, Cargo workspace resolver 3, serde/serde_json, thiserror, ulid, rusqlite with bundled SQLite, clap, tempfile.

## Global Constraints

- All packages live under `crates/`; directory names have no `notez-` prefix.
- The published executable is named `notez`.
- Org files are authoritative; SQLite is a disposable projection.
- No unchanged Org input may be rewritten by scanning.
- CLI and MCP must not read files or SQLite directly.
- V1 entity granularity in this plan is Document plus Heading.
- Use stable ResourceRef values and never use a title or path as identity.

---

### Task 0: Initialize the project repository

**Files:**
- Create: `.gitignore`
- Track: `docs/superpowers/specs/2026-07-22-notez-design.md`
- Track: `docs/superpowers/plans/2026-07-22-notez-roadmap.md`
- Track: `docs/superpowers/plans/2026-07-22-notez-core-vertical-slice.md`

**Interfaces:**
- Produces: a Git repository with the approved design and executable plan as its baseline commit.

- [ ] **Step 1: Initialize Git and create the ignore file**

Run: `git init`

Expected: a new empty Git repository in the project root.

Create `.gitignore` with exactly:

```gitignore
/target/
/.notez/
/.superpowers/
```

- [ ] **Step 2: Verify only durable project files will be committed**

Run: `git status --short`

Expected: `.gitignore` and `docs/` are untracked; `.superpowers/` is absent.

- [ ] **Step 3: Commit the approved baseline**

```bash
git add .gitignore docs
git commit -m "docs: define notez architecture and core plan"
```

Expected: one root commit containing the approved design, roadmap, core plan, and ignore rules.

### Task 1: Cargo workspace and domain algebra

**Files:**
- Create: `Cargo.toml`
- Create: `crates/domain/Cargo.toml`
- Create: `crates/domain/src/lib.rs`
- Create: `crates/domain/src/resource.rs`
- Create: `crates/domain/src/query.rs`
- Test: `crates/domain/tests/algebra.rs`

**Interfaces:**
- Produces: `ResourceRef::parse(&str)`, `Resource`, `ResourceKind`, `Selector`, `Projection`, `QueryPage`.

- [ ] **Step 1: Write the failing domain test**

```rust
use domain::{Projection, ResourceKind, ResourceRef, Selector};

#[test]
fn resource_refs_and_selectors_compose() {
    let id = ResourceRef::parse("heading:01J00000000000000000000000").unwrap();
    assert_eq!(id.kind(), ResourceKind::Heading);
    let selector = Selector::kind(ResourceKind::Heading).with_title_contains("sync");
    assert_eq!(selector.kind, Some(ResourceKind::Heading));
    assert_eq!(selector.title_contains.as_deref(), Some("sync"));
    assert_eq!(Projection::summary().fields, vec!["ref", "title", "revision"]);
}
```

- [ ] **Step 2: Run the test and verify the missing crate failure**

Run: `cargo test -p domain --test algebra`

Expected: FAIL because the workspace/package does not exist.

- [ ] **Step 3: Create the workspace and minimal algebra**

Root `Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
ulid = { version = "1", features = ["serde"] }
```

`ResourceRef` must parse exactly `document:<ULID>` and `heading:<ULID>`, reject unknown kinds, expose `kind()`, and implement Display/Serialize/Deserialize. `Resource` must contain `ref`, `kind`, `title`, `revision`, `source_id`, `locator`, and `properties: BTreeMap<String,String>`. `Selector` contains optional kind, exact refs, and title substring. `Projection::summary()` returns `ref,title,revision`. Re-export all public types from `lib.rs`.

- [ ] **Step 4: Run domain tests**

Run: `cargo test -p domain`

Expected: PASS, including invalid-kind and invalid-ULID cases added beside the first test.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/domain
git commit -m "feat: define core resource algebra"
```

### Task 2: Lossless Org scanner

**Files:**
- Create: `crates/document/Cargo.toml`
- Create: `crates/document/src/lib.rs`
- Create: `crates/document/src/org.rs`
- Create: `crates/document/tests/fixtures/basic.org`
- Test: `crates/document/tests/org_scan.rs`

**Interfaces:**
- Consumes: `domain::{Resource, ResourceKind, ResourceRef}`.
- Produces: `OrgScanner::scan(path, source_id) -> Result<ScannedDocument, DocumentError>`; `ScannedDocument { raw, resources, links }`.

- [ ] **Step 1: Add the fixture and failing scan test**

Fixture:

```org
#+title: Notez Architecture
#+ID: 01J00000000000000000000000

* NEXT Design sync
:PROPERTIES:
:ID: 01J00000000000000000000001
:TYPE: project
:END:
See [[id:01J00000000000000000000002][Merge rules]].
```

Test assertions: raw bytes equal the fixture; resources are one Document and one Heading; heading title is `Design sync`; properties include `TYPE=project` and `TODO=NEXT`; one unresolved link target is the referenced ID; scanning does not change file metadata or contents.

- [ ] **Step 2: Run the scanner test**

Run: `cargo test -p document --test org_scan`

Expected: FAIL because `OrgScanner` is undefined.

- [ ] **Step 3: Implement the minimum lossless scanner**

Store the original input as `Arc<str>`. Parse line spans without normalizing text. Recognize `#+title`, `#+ID`, headings, property drawers, and `[[id:...][...]]`. Heading TODO is recognized only when the first title token is one of `TODO NEXT PEND WAIT DONE QUIT`; store the remainder as title. Return byte spans for every parsed node. Never expose a render method in this task.

- [ ] **Step 4: Add malformed input and nested heading tests, then run**

Run: `cargo test -p document`

Expected: PASS; malformed IDs report line/column and nested headings preserve their level and parent ref.

- [ ] **Step 5: Commit**

```bash
git add crates/document
git commit -m "feat: scan org resources without rewriting input"
```

### Task 3: Disposable SQLite projection

**Files:**
- Create: `crates/storage/Cargo.toml`
- Create: `crates/storage/src/lib.rs`
- Create: `crates/storage/src/sqlite.rs`
- Test: `crates/storage/tests/projection.rs`

**Interfaces:**
- Consumes: `domain::{Resource, ResourceRef, Selector, QueryPage}`.
- Produces: trait `ProjectionStore` with `replace_source`, `get`, `query`, and `clear`; implementation `SqliteProjection`.

- [ ] **Step 1: Write a failing replacement/rebuild test**

Create an in-memory store, insert two resources for source `native`, replace the same source with one resource, assert the removed row disappears, clear the database, reinsert, and assert the query result is identical. Also assert title queries are case-insensitive and results sort by `ref`.

- [ ] **Step 2: Run the storage test**

Run: `cargo test -p storage --test projection`

Expected: FAIL because `ProjectionStore` is undefined.

- [ ] **Step 3: Implement schema and store**

Use tables `resources(ref PRIMARY KEY, kind, title, revision, source_id, locator, properties_json)` and `relations(source_ref, relation, target_ref, source_id)`. `replace_source` runs one transaction: delete rows for source, insert resources and links, commit. Convert all rusqlite failures into `StorageError`; never panic on stored data.

- [ ] **Step 4: Run tests**

Run: `cargo test -p storage`

Expected: PASS, including rollback on duplicate refs and persistence after reopen.

- [ ] **Step 5: Commit**

```bash
git add crates/storage
git commit -m "feat: add rebuildable sqlite projection"
```

### Task 4: Application service and native scan

**Files:**
- Create: `crates/application/Cargo.toml`
- Create: `crates/application/src/lib.rs`
- Create: `crates/application/src/service.rs`
- Create: `crates/application/src/native.rs`
- Test: `crates/application/tests/vertical_slice.rs`

**Interfaces:**
- Consumes: `OrgScanner`, `ProjectionStore`, domain algebra.
- Produces: `ApplicationService<S: ProjectionStore>` with `scan_native`, `resolve`, `query`, `read`, and `rebuild`.

- [ ] **Step 1: Write the failing vertical-slice test**

Create a temp space containing two Org files. Call `scan_native`; query headings containing `sync`; resolve the exact ResourceRef; delete the SQLite file; create a new service and call `rebuild`; assert resources and relations equal the pre-delete snapshot.

- [ ] **Step 2: Run the application test**

Run: `cargo test -p application --test vertical_slice`

Expected: FAIL because `ApplicationService` is undefined.

- [ ] **Step 3: Implement the service**

`scan_native(root)` recursively accepts only `.org`, sorts paths before scanning, computes revision as a lowercase SHA-256 of bytes, and replaces source `native`. `resolve` accepts exact refs first, then exact locator, then case-insensitive title; zero hits return `NotFound`, multiple hits return `Ambiguous(Vec<ResourceRef>)`. `rebuild` clears the projection and invokes scan. Add `sha2 = "0.10"` and `walkdir = "2"` to workspace dependencies.

- [ ] **Step 4: Run all workspace tests**

Run: `cargo test --workspace`

Expected: PASS; corrupt Org produces a path-qualified error and preserves the last valid projection.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/application
git commit -m "feat: expose core notez application service"
```

### Task 5: CLI over ApplicationService

**Files:**
- Create: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/main.rs`
- Create: `crates/cli/src/commands.rs`
- Test: `crates/cli/tests/cli.rs`

**Interfaces:**
- Consumes: `ApplicationService` only.
- Produces: binary `notez`; commands `scan`, `resolve`, `query`, `read`, `space rebuild`.

- [ ] **Step 1: Write failing CLI integration tests**

Run the binary against a temp space and database. Assert `scan --json` reports file/resource counts; `query --kind heading --title-contains sync --json` emits one JSON object; ambiguous resolve exits 4; missing input exits 3; diagnostics appear only on stderr.

- [ ] **Step 2: Run CLI tests**

Run: `cargo test -p cli --test cli`

Expected: FAIL because binary `notez` does not exist.

- [ ] **Step 3: Implement Clap commands**

Use global required `--space <PATH>` and optional `--db <PATH>` defaulting to `<space>/.notez/index.sqlite`. Map exit codes: 0 success, 2 invalid request, 3 not found, 4 ambiguous/conflict, 5 internal failure. Human output is concise; `--json` serializes stable response structs.

- [ ] **Step 4: Run CLI and workspace tests**

Run: `cargo test --workspace`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli
git commit -m "feat: add notez query cli"
```

### Task 6: MCP stdio using the same algebra

**Files:**
- Create: `crates/mcp/Cargo.toml`
- Create: `crates/mcp/src/lib.rs`
- Create: `crates/mcp/src/server.rs`
- Modify: `crates/cli/src/commands.rs`
- Test: `crates/mcp/tests/stdio.rs`

**Interfaces:**
- Consumes: `ApplicationService` and domain request/response types.
- Produces: `McpServer::serve(reader, writer)` and CLI command `notez mcp serve`.

- [ ] **Step 1: Write failing JSON-RPC transcript tests**

Send `initialize`, `tools/list`, and `tools/call` requests for `resolve`, `query`, `read`, and `inspect`. Assert each response preserves request ID, emits one JSON object per line, and maps NotFound/Ambiguous to structured tool errors. Assert `tools/call query` returns the same serialized resources as CLI query.

- [ ] **Step 2: Run MCP tests**

Run: `cargo test -p mcp --test stdio`

Expected: FAIL because `McpServer` is undefined.

- [ ] **Step 3: Implement the MCP server**

Implement newline-delimited JSON-RPC framing behind `BufRead/Write`. Register only `resolve`, `query`, `read`, and `inspect` in this slice. Each schema uses `deny_unknown_fields`; invalid params return JSON-RPC `-32602`; unknown methods return `-32601`; application errors appear as `isError: true` tool results with stable error code and details. Do not access filesystem or SQLite outside `ApplicationService` construction in the CLI composition root.

- [ ] **Step 4: Run equivalence and protocol tests**

Run: `cargo test --workspace`

Expected: PASS, including malformed JSON recovery on the following line.

- [ ] **Step 5: Commit**

```bash
git add crates/mcp crates/cli
git commit -m "feat: expose notez queries over mcp stdio"
```

### Task 7: Acceptance, documentation, and rebuild proof

**Files:**
- Create: `tests/fixtures/space/index.org`
- Create: `tests/fixtures/space/sync.org`
- Create: `scripts/acceptance-core.sh`
- Create: `README.md`

**Interfaces:**
- Consumes: final `notez` binary.
- Produces: repeatable acceptance script and user-facing core usage documentation.

- [ ] **Step 1: Write the acceptance script before fixing failures**

The script must build `notez`, copy the fixture space to a temp directory, scan, query through CLI, query through MCP transcript, save normalized JSON outputs, delete `.notez/index.sqlite`, rebuild, and compare pre/post output byte-for-byte. It exits nonzero on any mismatch and cleans only its own `mktemp -d` directory via trap.

- [ ] **Step 2: Run acceptance and observe the first failure**

Run: `bash scripts/acceptance-core.sh`

Expected: FAIL until all command output and rebuild behavior satisfy the script.

- [ ] **Step 3: Make only the required fixes and document usage**

README must state files are authoritative, show workspace layout, provide exact scan/query/MCP commands, document exit codes, and explain that deleting `.notez/index.sqlite` is safe after stopping Notez.

- [ ] **Step 4: Run final verification**

Run: `cargo fmt --all -- --check`

Expected: exit 0.

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: exit 0.

Run: `cargo test --workspace`

Expected: all tests pass.

Run: `bash scripts/acceptance-core.sh`

Expected: prints `core acceptance: PASS` and exits 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts tests
git commit -m "test: prove core index is rebuildable"
```
