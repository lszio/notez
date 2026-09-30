# Slice 0 Runtime and RevisionPrecondition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace implicit/optional revision guards with an explicit `MustMatch` / `MustNotExist` precondition, then make the host supervisor share one Runtime and watcher set per process.

**Architecture:** `notez_protocol` owns the wire-stable `RevisionPrecondition` enum. `MustMatch { revision }` protects updates, deletes, transitions, and writebacks; `MustNotExist` permits creation. The dispatcher converts this precondition into domain validation before invoking a use case, and journal/audit records retain the exact precondition rather than flattening it to `Option<String>`. CLI and Web translate explicit user intent into the same enum. Host service assembly receives one `composition::Runtime` and never opens a second cache or watcher.

**Tech Stack:** Rust, serde/schemars, clap, Axum, Tokio, `notez_protocol`, `notez_core`, `notez_composition`, cargo integration tests.

---

## Scope and invariants

- No `Option<String>` or empty-string sentinel represents a write guard.
- No mutation silently means “force overwrite.” A force command is out of this slice.
- `MustNotExist` is valid only for create/upsert-at-new-identity paths.
- `MustMatch` is required for update/delete/transition/writeback and carries a non-empty revision.
- Read-only Janet execution has no revision precondition.
- Existing ObjectTree, `index.org`, UI workbench, sync graph, dashboard, and authorization redesign remain out of scope.
- The current user `.gitignore` change and existing design document remain untouched.

## File map

- `crates/protocol/src/request.rs` — `RevisionPrecondition` and mutation request fields.
- `crates/protocol/src/lib.rs` — public re-export if required.
- `crates/protocol/src/schema.rs` — schema fixtures.
- `crates/protocol/tests/contract_parity.rs` — wire contract tests.
- `crates/core/src/domain/change.rs` — durable precondition field.
- `crates/core/src/application/write_check.rs` — `check_precondition` semantics.
- `crates/core/src/application/dispatcher.rs` — dispatch all mutation preconditions.
- `crates/core/src/application/projector.rs` — journal `Change` with exact precondition.
- `crates/core/src/application/service.rs` — document update accepts a `MustMatch` revision.
- `crates/core/src/storage/journal.rs` and `crates/core/src/storage/sqlite.rs` — persist the serialized precondition.
- `crates/core/tests/{protocol_parity,write_spine}.rs` — creation, update, stale, delete behavior.
- `crates/cli/src/commands.rs` — `--revision` plus explicit create/update mode where needed.
- `crates/cli/src/handlers/{resource,task,source}.rs` — translate CLI intent.
- `packages/web/src/data/space.rs` — Web update uses `MustMatch`.
- `crates/cli/src/host.rs` — shared Runtime/watcher.
- `crates/cli/tests/host_runtime.rs` — runtime identity regression.
- `scripts/acceptance-revision.sh` — stale update and create acceptance.

### Task 1: Define the explicit protocol precondition

- [ ] Add a failing protocol contract test covering:
  - `RevisionPrecondition::MustMatch { revision: "r1" }` round-trips;
  - `RevisionPrecondition::MustNotExist` round-trips;
  - `{"kind":"must_match","revision":""}` and whitespace revision fail;
  - omitted precondition fails for each mutation request;
  - Janet request accepts no revision field and has no revision field in generated schema.
- [ ] Run the focused protocol test and observe failure because the enum/request fields do not exist.
- [ ] Add to `crates/protocol/src/request.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RevisionPrecondition {
    MustMatch { revision: NonEmptyRevision },
    MustNotExist,
}
```

Use a small transparent `NonEmptyRevision(String)` newtype with a custom deserializer and `AsRef<str>`. This keeps the enum expressive while making whitespace rejection impossible at construction from wire data. Derive/implement the traits required by existing JSON/schema tests.
- [ ] Add `revision: RevisionPrecondition` to Delete, Upsert, Transition, Writeback, and UpdateDocument requests. Remove `expected_revision` and `base_revision` from those structs. Remove Janet revision fields entirely.
- [ ] Update schema fixture constructors with `RevisionPrecondition::MustMatch { revision: NonEmptyRevision::new("revision-1") }` or `MustNotExist` as appropriate.
- [ ] Run `cargo test -p notez-protocol --test contract_parity` and `cargo test -p notez-protocol`; both must pass.

### Task 2: Carry preconditions through domain Change and storage

- [ ] Add a failing core test that performs a create (`MustNotExist`) and update (`MustMatch`) then reads the journal and asserts the exact precondition kind is preserved.
- [ ] Run the focused core test and observe failure because `Change.expected_revision` is currently `Option<String>`.
- [ ] Replace `Change.expected_revision: Option<String>` with `Change.precondition: RevisionPrecondition` (or an equivalent domain-owned mirror if core cannot depend on protocol). Keep protocol and domain boundaries explicit: if domain owns the type, provide typed conversion in dispatcher; do not serialize an untyped JSON sentinel.
- [ ] Update `Journaling::record`, `record_writeback`, Projector calls, SQLite schema adapter, and journal reader to persist/restore the enum. Existing databases must migrate nullable `expected_revision` to a serialized precondition with a deterministic legacy mapping: non-null values become `MustMatch`; null bulk events become `MustNotExist` only for create operations and a dedicated `UnconditionalObservation` for scan/reindex events. Do not reinterpret old null update events as safe writes.
- [ ] Run `cargo test -p notez-core --test write_spine` and the focused journal tests.

### Task 3: Enforce preconditions in dispatcher and write checks

- [ ] Add failing tests for:
  - upsert with `MustNotExist` succeeds only when the resource does not exist;
  - upsert with `MustNotExist` against an existing resource returns `Conflict`;
  - update/delete/transition/writeback with stale `MustMatch` returns `RevisionConflict`;
  - update/delete/transition/writeback with `MustNotExist` returns `InvalidRequest`.
- [ ] Run these tests before implementation and verify the expected failures.
- [ ] Replace `check_revision` with `check_precondition`:
  - `MustMatch` requires a non-empty revision and compares the persisted revision;
  - `MustNotExist` rejects an existing row and succeeds for an absent row only on create-capable operations;
  - operation-specific validation rejects invalid combinations before any journal append or source write.
- [ ] Remove `effective_expected` and all `None`/empty fallback behavior from dispatcher. Each mutation arm passes the typed precondition to validation and then to its Projector call.
- [ ] Update `Engine::update_document` to accept `&RevisionPrecondition` but reject `MustNotExist` because document update targets an existing indexed file; creation is a separate command.
- [ ] Run `cargo test -p notez-core --test protocol_parity`, `cargo test -p notez-core --test write_spine`, and focused write-check tests.

### Task 4: Migrate CLI and Web surfaces without implicit overwrite

- [ ] Add failing CLI tests for `resource upsert --create` with `MustNotExist`, `resource upsert --revision r1` with `MustMatch`, and stale delete returning exit 9.
- [ ] Add explicit CLI syntax: `--create` maps to `MustNotExist`; `--expected-revision <REVISION>` maps to `MustMatch`. Reject using both. Delete/transition/writeback require `--expected-revision`; no default.
- [ ] Update CLI handlers to construct typed preconditions and remove every empty-string/`None` placeholder. Janet handler removes revision entirely.
- [ ] Update Web `save_doc` to construct `MustMatch { revision }`; no `base_revision` alias remains.
- [ ] Run CLI contract tests, help output checks, and Web/core protocol parity tests.

### Task 5: Unify host Runtime and watcher

- [ ] Add a failing `host_runtime` test for Runtime watcher identity.
- [ ] Construct one `Arc<WatchService>` and `Runtime::with_watch(watch.clone())` in `run_supervisor_loop` before service thread creation.
- [ ] Pass `Runtime` into `run_api_mcp_servers` and `push_via_engine`; use `runtime.open(&selected)` for MCP and sync. Remove `Runtime::new`, `open_space`, temporary Engine creation, and duplicate `WatchService::new` from host service paths. Remove duplicate `install_sigterm()`.
- [ ] Run `cargo test -p notez-cli --test host_runtime` and `sync_cli`, then a bounded host smoke.

### Task 6: Acceptance and review

- [ ] Add `scripts/acceptance-revision.sh` that creates a new document with `--create`, updates it with its returned revision, attempts a stale update and asserts exit 9, then verifies the file remains the successful writer’s content.
- [ ] Run `cargo build -p cli`, the acceptance script, protocol/core/CLI focused suites, then `cargo test --workspace`.
- [ ] Run `git diff --check` and `git status --short`; do not claim completion until all affected callers and tests use the typed precondition.

## Plan self-review

- Creation and update semantics are explicit and non-overlapping.
- No task relies on `None`, empty strings, or an untyped JSON fallback for write guards.
- Journal migration distinguishes create/update semantics from scan observations; old nullable rows are not silently treated as unconditional updates.
- Protocol, domain, storage, CLI, Web, and host work are ordered by dependency.
