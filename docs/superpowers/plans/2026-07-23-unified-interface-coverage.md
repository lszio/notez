# Unified Interface Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose every public Notez capability through one typed application protocol with complete, test-enforced CLI and MCP coverage.

**Architecture:** Define shared requests, responses, errors and a capability catalog in `application`; route both transports through one dispatcher; derive MCP schemas from Rust DTO schemas; test transport parity over the same fixtures.

**Tech Stack:** Rust 2024, serde, serde_json, schemars, clap, JSON-RPC, cargo test

## Global Constraints

- CLI and MCP contain transport logic only.
- Every public capability is declared once with explicit transport exposure.
- Resource targets use `ResourceAddress`; writes require unique resolution.
- Errors have stable code, message, details and remediation fields.
- Equivalent CLI and MCP requests return equivalent domain results.

---

### Task 1: Shared protocol DTOs and stable errors

**Files:**
- Create: `crates/application/src/protocol.rs`
- Modify: `crates/application/src/lib.rs`
- Modify: `crates/application/Cargo.toml`
- Test: `crates/application/tests/protocol.rs`

**Interfaces:**
- Produces: `NotezRequest`, `NotezResponse`, `ApiError`, `ErrorCode`, operation request/response structs.
- Consumes: `ResourceAddress`, `Selector`, projections and existing report types.

- [ ] **Step 1: Write JSON contract tests**

```rust
#[test]
fn ambiguous_target_has_stable_error_shape() {
    let error = ApiError::ambiguous(vec![candidate_a(), candidate_b()]);
    let json = serde_json::to_value(error).unwrap();
    assert_eq!(json["code"], "AMBIGUOUS_TARGET");
    assert!(json["details"]["candidates"].is_array());
    assert!(json["remediation"].is_string());
}
```

- [ ] **Step 2: Verify protocol tests fail**

Run: `cargo test -p application --test protocol`

Expected: FAIL because shared DTOs do not exist.

- [ ] **Step 3: Implement tagged requests and responses**

Create variants for resource, link, task/PARA, source, attachment, community/artifact, sync, space/system and config/registry operations. Use `#[serde(tag = "operation", content = "input", rename_all = "snake_case")]`. Map existing `ApplicationError` to stable codes at one boundary.

- [ ] **Step 4: Run application tests**

Run: `cargo test -p application`

Expected: PASS and existing service methods remain callable during migration.

- [ ] **Step 5: Commit**

```bash
git add crates/application/Cargo.toml crates/application/src/protocol.rs crates/application/src/lib.rs crates/application/tests/protocol.rs
git commit -m "feat(application): define shared public protocol"
```

### Task 2: Capability catalog and coverage assertions

**Files:**
- Create: `crates/application/src/capability.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/capability_catalog.rs`

**Interfaces:**
- Produces: `CapabilityDescriptor { name, mutability, cli, mcp, request_schema, response_schema }` and `public_capabilities()`.
- Consumes: operation types from Task 1.

- [ ] **Step 1: Write required-operation coverage test**

Assert the catalog includes every operation listed in `docs/architecture.md`: resource, link, task/PARA, source, attachment, community/artifact, sync, space/system and config/registry. Assert each has CLI and MCP exposure or a non-empty explicit exclusion reason.

- [ ] **Step 2: Verify catalog test fails**

Run: `cargo test -p application --test capability_catalog`

Expected: FAIL because the catalog is absent.

- [ ] **Step 3: Implement static descriptors**

Give each capability a stable snake_case name, mutability, request/response type names and transport names. Add uniqueness checks for capability, CLI and MCP names.

- [ ] **Step 4: Run catalog tests**

Run: `cargo test -p application --test capability_catalog`

Expected: PASS with no duplicate or unaccounted operations.

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/capability.rs crates/application/src/lib.rs crates/application/tests/capability_catalog.rs
git commit -m "feat(application): catalog public capabilities"
```

### Task 3: Single application dispatcher

**Files:**
- Create: `crates/application/src/dispatcher.rs`
- Modify: `crates/application/src/lib.rs`
- Modify: `crates/application/src/service.rs`
- Test: `crates/application/tests/dispatcher.rs`

**Interfaces:**
- Produces: `Dispatcher<S>::dispatch(&mut self, context: &RequestContext, request: NotezRequest) -> Result<NotezResponse, ApiError>`.
- Consumes: `ApplicationService`, `RuntimeConfig`, protocol DTOs and address resolution.

- [ ] **Step 1: Write dispatcher tests for reads and guarded writes**

Assert read by ref and file locator return the same resource; ambiguous title write returns `AMBIGUOUS_TARGET`; read-only source write returns `SOURCE_READ_ONLY`; revision mismatch returns `REVISION_CONFLICT`.

- [ ] **Step 2: Verify dispatcher tests fail**

Run: `cargo test -p application --test dispatcher`

Expected: FAIL because `Dispatcher` does not exist.

- [ ] **Step 3: Implement dispatch and context checks**

`RequestContext` contains space root, actor, trace ID and dry-run flag. Resolve `ResourceAddress` before invoking existing methods. Move direct uses of `Path::new(".")` and hard-coded workflow defaults behind context/config. Return warnings and trace ID in every response envelope.

- [ ] **Step 4: Run application suite**

Run: `cargo test -p application`

Expected: PASS; old methods can temporarily delegate to dispatcher-compatible helpers.

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/dispatcher.rs crates/application/src/lib.rs crates/application/src/service.rs crates/application/tests/dispatcher.rs
git commit -m "feat(application): dispatch all operations through one boundary"
```

### Task 4: Migrate CLI to the dispatcher and complete commands

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Test: `crates/cli/tests/capability_cli.rs`

**Interfaces:**
- Produces complete CLI commands matching the capability catalog.
- Consumes: dispatcher and configuration plans.

- [ ] **Step 1: Write missing-command and behavior tests**

Cover `link list/diagnose/reindex`, `para overview`, `source capabilities`, `resource mutate/relate`, and existing scan/read/sync/artifact commands. Assert JSON output deserializes to the shared response envelope.

- [ ] **Step 2: Verify CLI coverage fails**

Run: `cargo test -p cli --test capability_cli`

Expected: FAIL for currently missing commands and legacy response shapes.

- [ ] **Step 3: Map Clap arguments to `NotezRequest`**

Keep rendering and exit-code mapping in CLI, but remove direct calls to individual service methods. Parse every resource argument with `ResourceAddress::parse`. Map stable errors to exit codes 2 invalid request, 3 not found, 4 ambiguous/conflict and 5 internal/storage.

- [ ] **Step 4: Run CLI suite**

Run: `cargo test -p cli`

Expected: PASS, including pre-existing commands and new coverage assertions.

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/commands.rs crates/cli/src/main.rs crates/cli/tests/capability_cli.rs
git commit -m "feat(cli): expose the complete application protocol"
```

### Task 5: Generate MCP schemas and complete MCP tools

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/application/Cargo.toml`
- Create: `crates/mcp/src/schema.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/mcp/tests/capability_mcp.rs`

**Interfaces:**
- Produces MCP tools generated from the capability catalog and schemars request schemas.
- Consumes: dispatcher from Task 3 and capability catalog from Task 2.

- [ ] **Step 1: Write MCP catalog parity tests**

Initialize the server, collect `tools/list`, and assert its names equal catalog entries with MCP exposure. Call `inspect`, `source_sync`, `source_writeback`, `relay_sync`, `space_rebuild`, `community_list` and `para_overview`; assert shared response envelopes.

- [ ] **Step 2: Verify MCP tests fail**

Run: `cargo test -p mcp --test capability_mcp`

Expected: FAIL because current tools are hand-written and incomplete.

- [ ] **Step 3: Implement schema conversion and dispatcher calls**

Add `schemars = "0.8"`; derive `JsonSchema` for public request DTOs; convert root schemas to MCP `inputSchema`; validate arguments by deserializing the request type. Make `inspect` invoke the actual inspect operation instead of querying all resources.

- [ ] **Step 4: Run MCP suite**

Run: `cargo test -p mcp`

Expected: PASS and `tools/list` exactly matches the MCP-exposed capability catalog.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/application/Cargo.toml crates/mcp/src/schema.rs crates/mcp/src/server.rs crates/mcp/tests/capability_mcp.rs
git commit -m "feat(mcp): generate complete tools from shared capabilities"
```

### Task 6: End-to-end transport parity and release gate

**Files:**
- Create: `tests/interface_parity.rs`
- Modify: `scripts/acceptance-release.sh`
- Modify: `README.md`

**Interfaces:**
- Produces a release gate comparing CLI/MCP results and documenting the completed public protocol.
- Consumes: all previous tasks in this plan.

- [ ] **Step 1: Write parity scenarios**

Build one fixture space and run equivalent resolve, read, query, link diagnose, PARA, source list, community list, doctor and conflict requests through CLI and MCP. Remove transport-only fields and assert equal JSON values; compare stable error codes for ambiguous writes.

- [ ] **Step 2: Verify parity test catches a deliberate mismatch**

Run: `cargo test --test interface_parity`

Expected before final wiring: FAIL showing the first non-equivalent response path.

- [ ] **Step 3: Normalize response envelopes and update user examples**

Fix only transport presentation differences; do not duplicate application logic. Update root README examples for named spaces, resource addresses and the complete MCP tool families.

- [ ] **Step 4: Run full release verification**

Run: `cargo test --workspace && bash scripts/acceptance-release.sh`

Expected: all workspace and acceptance suites PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/interface_parity.rs scripts/acceptance-release.sh README.md
git commit -m "test: enforce CLI and MCP protocol parity"
```
