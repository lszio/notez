# Notez Post-v0.1.0 Evolution Roadmap Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Phase 8 Evolution features: `Anytype` SourceAdapter contract stub, explicit Writeback Capabilities (`prepare_write`, `commit_write`), Block/Paragraph-level fine-grained entities, and Relay/P2P SyncTransport abstraction.

**Architecture:** Extend `source` with `AnytypeSourceAdapter` and writeback capability flags. Add `ResourceKind::Block` in `domain`. Update `application` with atomic writeback transactions and Relay transport interfaces.

---

### Task 1: Anytype SourceAdapter and Writeback Capabilities

**Files:**
- Create: `crates/source/src/anytype.rs`
- Modify: `crates/source/src/adapter.rs`
- Modify: `crates/source/src/lib.rs`
- Test: `crates/source/tests/anytype_adapter.rs`

**Interfaces:**
- Produces: `AnytypeSourceAdapter`, `SourceCapabilities`, `prepare_write`, `commit_write`.

- [ ] **Step 1: Write failing Anytype adapter test**

Create `crates/source/tests/anytype_adapter.rs`: test `AnytypeSourceAdapter` scanning and capability declarations (`read`, `write`, `import`).

- [ ] **Step 2: Run test and verify missing Anytype adapter failure**

Run: `cargo test -p source --test anytype_adapter`
Expected: FAIL because `AnytypeSourceAdapter` is missing.

- [ ] **Step 3: Implement AnytypeSourceAdapter and SourceCapabilities**

Implement `AnytypeSourceAdapter` contract stub, `SourceCapabilities` struct, and writeback method declarations.

- [ ] **Step 4: Run source tests**

Run: `cargo test -p source`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/source
git commit -m "feat: implement anytype source adapter contract and writeback capabilities"
```

---

### Task 2: Fine-Grained Block Entities and Selection

**Files:**
- Modify: `crates/domain/src/resource.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/document/src/org.rs`
- Modify: `crates/document/src/markdown.rs`
- Test: `crates/document/tests/block_entities.rs`

**Interfaces:**
- Produces: `ResourceKind::Block`, block-level reference parsing (`block:<ULID>`).

- [ ] **Step 1: Write failing block entity test**

Create `crates/document/tests/block_entities.rs`: test parsing paragraph block IDs in Org (`:ID:` drawers or `#+NAME:`) and Markdown (`^block-id` or comment IDs).

- [ ] **Step 2: Run block entity test**

Run: `cargo test -p document --test block_entities`
Expected: FAIL because `ResourceKind::Block` is missing.

- [ ] **Step 3: Implement ResourceKind::Block and block parsing**

Add `ResourceKind::Block`, update `ResourceRef::parse` for `block:<ULID>`, and update document scanners to extract paragraph blocks.

- [ ] **Step 4: Run document tests**

Run: `cargo test -p document`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/domain crates/document
git commit -m "feat: support fine-grained block entities and block-level references"
```

---

### Task 3: Relay/P2P SyncTransport Abstraction

**Files:**
- Create: `crates/sync/src/relay.rs`
- Modify: `crates/sync/src/lib.rs`
- Test: `crates/sync/tests/relay_transport.rs`

**Interfaces:**
- Produces: `RelayTransport`, `P2pTransport`, trait `SyncTransport`.

- [ ] **Step 1: Write failing relay transport test**

Create `crates/sync/tests/relay_transport.rs`: test `RelayTransport` memory stub exchanging manifests and objects.

- [ ] **Step 2: Run relay test**

Run: `cargo test -p sync --test relay_transport`
Expected: FAIL because `RelayTransport` is missing.

- [ ] **Step 3: Implement RelayTransport and SyncTransport trait**

Define `SyncTransport` trait and implement `RelayTransport` stub.

- [ ] **Step 4: Run sync tests**

Run: `cargo test -p sync`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sync
git commit -m "feat: add relay/p2p sync transport abstraction"
```

---

### Task 4: Application Service Writeback and Relay Sync Integration

**Files:**
- Create: `crates/application/src/writeback.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/writeback.rs`

**Interfaces:**
- Produces: `ApplicationService::writeback_resource`, `ApplicationService::relay_sync`.

- [ ] **Step 1: Write failing writeback application test**

Create `crates/application/tests/writeback.rs`: test executing writeback mutation on an external writeable source.

- [ ] **Step 2: Run writeback test**

Run: `cargo test -p application --test writeback`
Expected: FAIL because `writeback_resource` is missing.

- [ ] **Step 3: Implement Writeback and Relay Services**

Implement `writeback_resource` with atomic `prepare_write` / `commit_write` transactions and `relay_sync`.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: implement writeback transactions and relay sync in application service"
```

---

### Task 5: CLI and MCP Extensions for Writeback and Evolution Features

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/cli/tests/evolution_cli.rs`

**Interfaces:**
- Produces: CLI commands `source writeback`, `sync relay`; MCP tools `source_writeback`, `sync_relay`.

- [ ] **Step 1: Write failing evolution CLI test**

Test CLI `notez source writeback`, `notez sync relay`.

- [ ] **Step 2: Run CLI test**

Run: `cargo test -p cli --test evolution_cli`
Expected: FAIL because subcommands are missing.

- [ ] **Step 3: Implement CLI subcommands and MCP tools**

Add clap `writeback` and `relay` subcommands; register `source_writeback` and `sync_relay` tools in MCP stdio server.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli crates/mcp
git commit -m "feat: expose writeback and relay sync over cli and mcp"
```

---

### Task 6: Evolution Acceptance Script, Documentation, and Final Release Cut

**Files:**
- Create: `scripts/acceptance-evolution.sh`
- Modify: `README.md`

**Interfaces:**
- Produces: `scripts/acceptance-evolution.sh` running full evolution test suite.

- [ ] **Step 1: Write evolution acceptance script**

The script tests Anytype adapter, writeback capabilities, block-level references, and relay transport.

- [ ] **Step 2: Run evolution acceptance script**

Run: `bash scripts/acceptance-evolution.sh`
Expected: PASS (`evolution acceptance: PASS`).

- [ ] **Step 3: Update README.md**

Document Anytype adapter, Writeback capabilities, Block entities (`block:<ULID>`), and Relay/P2P sync.

- [ ] **Step 4: Final verification checks**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace`
Run: `bash scripts/acceptance-release.sh`
Run: `bash scripts/acceptance-evolution.sh`
Expected: All exit 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts docs
git commit -m "release: cut notez 0.2.0 evolution release"
```
