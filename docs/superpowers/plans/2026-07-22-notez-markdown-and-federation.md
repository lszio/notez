# Notez Markdown and Federation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Markdown document scanning (YAML frontmatter IDs, HTML comment heading IDs, WikiLinks `[[...]]`), `source` crate with `SourceAdapter` trait, adapters (`NativeSourceAdapter`, `GitSourceAdapter`, `ObsidianSourceAdapter`), multi-source federation scanning in `ApplicationService`, and CLI/MCP `source` management commands.

**Architecture:** Extend `document` with Markdown scanner. Introduce `crates/source` containing `SourceAdapter` trait and source implementations. Update `ApplicationService` to manage federated sources in space configuration and scan across multiple native/git/obsidian sources into SQLite projection.

**Tech Stack:** Rust 1.94, Cargo workspace, serde, serde_yaml, thiserror, ulid, rusqlite, clap, walkdir.

---

### Task 1: Lossless Markdown Scanner and WikiLinks Parsing

**Files:**
- Create: `crates/document/src/markdown.rs`
- Modify: `crates/document/src/lib.rs`
- Test: `crates/document/tests/markdown_scan.rs`

**Interfaces:**
- Produces: `MarkdownScanner::scan(path, source_id) -> Result<ScannedDocument, DocumentError>`.

- [ ] **Step 1: Write failing Markdown integration test**

Create `crates/document/tests/markdown_scan.rs`: test Markdown with YAML frontmatter `id` and `title`, headings with `<!-- id: 01J... -->`, and `[[WikiLink]]` / `[[id:01J...][label]]` links.

- [ ] **Step 2: Run test and verify missing Markdown scanner failure**

Run: `cargo test -p document --test markdown_scan`
Expected: FAIL because `MarkdownScanner` is missing.

- [ ] **Step 3: Implement MarkdownScanner**

Parse YAML frontmatter delimited by `---`, extract document ID/title/properties, parse headings (`#`, `##`) with optional HTML comment IDs `<!-- id: <ULID> -->`, and extract `[[...]]` links.

- [ ] **Step 4: Run document tests**

Run: `cargo test -p document`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/document
git commit -m "feat: add lossless markdown scanner with frontmatter and wikilinks"
```

---

### Task 2: Source Crate and SourceAdapter Trait

**Files:**
- Create: `crates/source/Cargo.toml`
- Create: `crates/source/src/lib.rs`
- Create: `crates/source/src/adapter.rs`
- Test: `crates/source/tests/adapter.rs`

**Interfaces:**
- Produces: `SourceKind`, `SourceConfig`, trait `SourceAdapter`, `ScannedSource`.

- [ ] **Step 1: Write failing source adapter test**

Create `crates/source/tests/adapter.rs`: test creating a mock `SourceAdapter` with ID and capabilities; verify scanning yields `ScannedSource`.

- [ ] **Step 2: Run source test**

Run: `cargo test -p source --test adapter`
Expected: FAIL because `source` crate is missing.

- [ ] **Step 3: Implement SourceAdapter trait and SourceConfig**

Create `crates/source/Cargo.toml` and implement `SourceKind` (`Native`, `Git`, `Obsidian`), `SourceConfig`, `SourceAdapter` trait (`id()`, `kind()`, `scan()`), and `ScannedSource`.

- [ ] **Step 4: Run source tests**

Run: `cargo test -p source`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/source
git commit -m "feat: add source crate and SourceAdapter trait"
```

---

### Task 3: Native, Git, and Obsidian SourceAdapters

**Files:**
- Create: `crates/source/src/native.rs`
- Create: `crates/source/src/git.rs`
- Create: `crates/source/src/obsidian.rs`
- Modify: `crates/source/src/lib.rs`
- Test: `crates/source/tests/federated_sources.rs`

**Interfaces:**
- Produces: `NativeSourceAdapter`, `GitSourceAdapter`, `ObsidianSourceAdapter`.

- [ ] **Step 1: Write failing federated source test**

Create `crates/source/tests/federated_sources.rs`: scan a simulated Obsidian Vault with Markdown notes and a Git folder with Org notes.

- [ ] **Step 2: Run federated source test**

Run: `cargo test -p source --test federated_sources`
Expected: FAIL because adapters are missing.

- [ ] **Step 3: Implement Native, Git, and Obsidian SourceAdapters**

Implement `NativeSourceAdapter` (scans Org + Markdown), `GitSourceAdapter` (verifies Git repo and scans notes), and `ObsidianSourceAdapter` (scans Vault `.md` files).

- [ ] **Step 4: Run source tests**

Run: `cargo test -p source`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/source
git commit -m "feat: implement native, git, and obsidian source adapters"
```

---

### Task 4: Multi-Source Federation in ApplicationService

**Files:**
- Create: `crates/application/src/federation.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/federation.rs`

**Interfaces:**
- Produces: `ApplicationService::add_source`, `ApplicationService::list_sources`, `ApplicationService::scan_federation`.

- [ ] **Step 1: Write failing federation integration test**

Create `crates/application/tests/federation.rs`: add native space and an external Obsidian Vault source; call `scan_federation`; query resources across both sources.

- [ ] **Step 2: Run federation test**

Run: `cargo test -p application --test federation`
Expected: FAIL because `scan_federation` is missing.

- [ ] **Step 3: Implement multi-source federation management**

Store source configs in space settings (`.notez/sources.json`); implement `add_source`, `list_sources`, and `scan_federation`.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: support multi-source federation in application service"
```

---

### Task 5: CLI and MCP Extensions for Source Management

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/cli/tests/source_cli.rs`
- Test: `crates/mcp/tests/source_mcp.rs`

**Interfaces:**
- Produces: CLI commands `source add`, `source list`, `source sync`; MCP tools `source_list`, `source_add`.

- [ ] **Step 1: Write failing CLI and MCP source tests**

Test CLI `notez source add --id vault --kind obsidian --path /path/to/vault`, `notez source list --json`; test corresponding MCP tools.

- [ ] **Step 2: Run CLI and MCP source tests**

Run: `cargo test -p cli --test source_cli` and `cargo test -p mcp --test source_mcp`
Expected: FAIL because `source` subcommands/tools are missing.

- [ ] **Step 3: Implement CLI subcommands and MCP tools**

Add clap `source` subcommand family; register `source_list` and `source_add` tools in MCP stdio server.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli crates/mcp
git commit -m "feat: expose source management over cli and mcp"
```

---

### Task 6: Acceptance Script, Documentation, and Final Verification

**Files:**
- Create: `scripts/acceptance-federation.sh`
- Modify: `README.md`

**Interfaces:**
- Produces: `scripts/acceptance-federation.sh` script testing multi-source scanning, Markdown links, rebuild, and cross-source queries.

- [ ] **Step 1: Write federation acceptance script**

The script mounts native Org and Obsidian Vault Markdown fixtures, scans, queries across sources, rebuilds index, and verifies consistency.

- [ ] **Step 2: Run federation acceptance script**

Run: `bash scripts/acceptance-federation.sh`
Expected: PASS (`federation acceptance: PASS`).

- [ ] **Step 3: Update README.md**

Document Markdown support, frontmatter/HTML comment IDs, `source` CLI/MCP commands, and Obsidian/Git vault mounting.

- [ ] **Step 4: Final verification checks**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace`
Run: `bash scripts/acceptance-core.sh`
Run: `bash scripts/acceptance-rules.sh`
Run: `bash scripts/acceptance-federation.sh`
Expected: All exit 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts docs
git commit -m "test: add federation acceptance script and update documentation"
```
