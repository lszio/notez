# Aggregated Core Format Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the aggregated `core` crate, move Org/Markdown parsing behind real adapter crates, unify scanning, and make source/projection mutations fail-safe.

**Architecture:** `core` owns format-neutral domain/protocol types, source transports, registry, application facade, and storage. `adapters/orgmode` and `adapters/markdown` implement `FormatParser` against `RawEntity.payload`; `cli` and `app` are composition roots. `ApplicationService` receives an explicit `SpaceContext`, a parser/source registry, and a projection store; complete scans use `replace_source`, mutations use per-resource operations.

**Tech Stack:** Rust 2024, Cargo workspace, serde, rusqlite, pulldown-cmark, org parser currently in `core/src/document/org.rs`, Markdown parser currently in `core/src/document/markdown.rs`, assert_cmd, tempfile, tokio/rmcp.

---

## Task 1: Establish the failing adapter contract

**Files:**
- Create: `adapters/orgmode/src/parser.rs`
- Create: `adapters/markdown/src/parser.rs`
- Modify: `adapters/orgmode/src/lib.rs`
- Modify: `adapters/markdown/src/lib.rs`
- Test: `adapters/orgmode/tests/parser.rs`
- Test: `adapters/markdown/tests/parser.rs`

- [ ] **Step 1: Write the failing Org parser contract test**

Create a test that constructs `core::source::RawEntity` with `locator` pointing to a nonexistent path and valid Org bytes in `payload`. Call `OrgParser::parse` and assert it returns a document and heading with the expected title. The test must prove the parser does not read `locator`.

- [ ] **Step 2: Run the Org contract test and verify the expected failure**

Run:

```bash
cargo test -p orgmode --test parser -- --nocapture
```

Expected: compilation or missing-symbol failure because `OrgParser` is not yet exported.

- [ ] **Step 3: Write the failing Markdown parser contract test**

Construct the same shape of `RawEntity`, with `mime_type: "text/markdown"`, a nonexistent locator, frontmatter and a heading in `payload`. Assert the parsed resource title and link occurrence. Include one unsupported MIME assertion that returns `false` from `supports`.

- [ ] **Step 4: Run the Markdown contract test and verify the expected failure**

Run:

```bash
cargo test -p markdown --test parser -- --nocapture
```

Expected: missing `MarkdownParser` implementation/export.

- [ ] **Step 5: Implement parser entrypoints with payload-only input**

Move the format-specific parsing logic from `core/src/document/org.rs` and `core/src/document/markdown.rs` into the adapter crates. Refactor the parser internals to accept `&[u8]` or `&str` and a logical locator, rather than opening a file. Export `OrgParser` and `MarkdownParser` from each adapter `lib.rs`. Keep only format-neutral `ScannedDocument` and workflow/security types in `core::document`.

- [ ] **Step 6: Run both adapter contract tests**

Run:

```bash
cargo test -p orgmode --test parser
cargo test -p markdown --test parser
```

Expected: all adapter contract tests pass, including nonexistent-locator payload parsing.

---

## Task 2: Add parser registry and make composed scanning strict

**Files:**
- Modify: `core/src/source/protocol.rs`
- Modify: `core/src/source/adapter.rs`
- Create: `core/src/source/registry.rs`
- Modify: `core/src/source/mod.rs`
- Test: `core/tests/source/registry.rs`
- Modify: `core/tests/source/includes_excludes.rs`

- [ ] **Step 1: Write registry failure tests**

Add tests for:

1. one transport returning both Org and Markdown raw entities dispatches each entity to its matching parser;
2. an unknown MIME returns `ParserNotFound` and does not return a partial `ScannedSource`;
3. a parser error returns an error and does not silently print and continue;
4. a successful scan produces one complete `ScannedSource`.

Use an in-memory test transport and small test parsers that return deterministic `ParsedEntity` values. Do not use mocks for the parser behavior itself.

- [ ] **Step 2: Run the registry tests and verify they fail**

Run:

```bash
cargo test -p core --test registry -- --nocapture
```

Expected: missing registry/error behavior failures.

- [ ] **Step 3: Implement explicit parser errors and registry dispatch**

Add a `ParserNotFound` variant to the source/application error path containing source ID, locator, and MIME. Add `SourceRegistry` with parser registration and a method that scans a configured `SourceTransport`. Select exactly one parser per entity; return on unknown MIME or parser failure. Do not call `eprintln!` from library code.

- [ ] **Step 4: Make `ComposedSourceAdapter::scan` delegate to the registry behavior**

Remove the current nested loop that logs parser errors and skips failed entities. The composed adapter must return the first error and must not expose partial resources as a successful scan.

- [ ] **Step 5: Run registry and existing source tests**

Run:

```bash
cargo test -p core --test registry
cargo test -p core --test includes_excludes
```

Expected: registry behavior and include/exclude behavior pass.

---

## Task 3: Wire real adapters into CLI and remove the duplicate native scan

**Files:**
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/main.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Modify: `core/src/source/native.rs`
- Modify: `core/src/document/mod.rs`
- Remove: `core/src/document/org.rs` after parser migration
- Remove: `core/src/document/markdown.rs` after parser migration
- Test: `core/tests/application/vertical_slice.rs`
- Test: `cli/tests/cli.rs`

- [ ] **Step 1: Change the vertical-slice test to construct the composition root**

Replace direct `ApplicationService::new(store)` and `scan_native` calls with construction of `SpaceContext`, native source configuration, `OrgParser`/`MarkdownParser`, and the registry. Assert that scanning the same two files returns the existing resource counts.

- [ ] **Step 2: Run the updated vertical-slice test and verify it fails**

Run:

```bash
cargo test -p core --test vertical_slice -- --nocapture
```

Expected: constructor/scan API failures until the new runtime wiring exists.

- [ ] **Step 3: Add adapter dependencies to the CLI composition root**

Add path dependencies for `orgmode` and `markdown` to `cli/Cargo.toml`. Build parser registration in one runtime helper called by `main`, then construct the native source adapter from runtime configuration. Keep `core` independent from both adapter crates.

- [ ] **Step 4: Replace `scan_native` with registry-backed source scanning**

Add an application scan method that receives a source ID or source adapter registry. It must fetch and parse through the registry, call `replace_source` only after the entire scan succeeds, then resolve links. Preserve the existing `ScanReport` observable fields.

- [ ] **Step 5: Run the core and CLI smoke tests**

Run:

```bash
cargo test -p core --test vertical_slice
cargo test -p cli --test cli -- cli_scan_query_resolve_read_rebuild
```

Expected: scan/query/resolve/read/rebuild pass through the unified parser path.

---

## Task 4: Restore a clean workspace build while retaining crate name `core`

**Files:**
- Modify: `cli/src/commands.rs`
- Modify: `cli/src/lib.rs`
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/mcp/server.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/Cargo.toml`
- Modify: `adapters/orgmode/src/lib.rs`
- Modify: `adapters/markdown/src/lib.rs`

- [ ] **Step 1: Add a compile regression check for generated derives**

Keep the existing `clap::Parser` and MCP schema types in a minimal compile-tested path. The test/build target must exercise `::core::env`, `::core::convert`, and schema generation behavior with the package still named `core`.

- [ ] **Step 2: Implement the narrowest compatibility fix**

Resolve the macro `::core::*` collision without renaming the package. Apply the smallest set of changes:

1. In `core/src/lib.rs` add `extern crate core as std_core;` to expose the standard library's `core` under an alias; rely on the 2024 prelude only if the alias is not required.
2. Rewrite `thiserror::Error` derives that expand to `::core::write` / `::core::fmt` / `::core::option` / `::core::convert` to hand-rolled `impl Display` plus `impl std::error::Error` blocks. Apply the rewrite only to the error enums that the failing `cargo check` output names. Do not rewrite other `thiserror` derives in this task.
3. Keep the existing hand-written schemars implementations for the MCP server and the hand-written `json_schema` helpers in `cli/src/mcp/server.rs`.

The fix must stay local to `core` and `cli`; do not introduce a build script or rename sub-modules to dodge the conflict.

- [ ] **Step 3: Remove stale feature declarations and paths**

Either wire the `web` feature to a real dependency or remove the dead CLI web command/feature for this scope. Make MCP’s always-compiled behavior truthful in `cli/Cargo.toml` and `cli/src/lib.rs`. Fix comments that still point to `crates/adapters` or claim core features that do not exist.

- [ ] **Step 4: Run the workspace build**

Run:

```bash
cargo check --workspace --all-targets
```

Expected: exit code 0. Warnings may remain only where existing incomplete adapters are intentionally retained; no macro resolution errors.

---

## Task 5: Remove write no-ops and introduce explicit mutation errors

**Files:**
- Modify: `core/src/domain/query.rs`
- Modify: `core/src/source/adapter.rs`
- Modify: `core/src/source/protocol.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/application/mutation_safety.rs`
- Test: `core/tests/storage/per_resource_write.rs`

- [ ] **Step 1: Write failing mutation safety tests**

Cover these observable behaviors:

1. updating one resource leaves another resource from the same source intact;
2. a missing source returns `SourceNotFound` and does not modify SQLite;
3. a read-only source returns `ReadOnlySource` and does not modify SQLite;
4. an expected revision mismatch returns `RevisionConflict` and does not modify SQLite;
5. a source commit failure leaves projection unchanged;
6. a store implementation that lacks a requested write capability cannot report success.

- [ ] **Step 2: Run the tests and verify the current bugs**

Run:

```bash
cargo test -p core --test mutation_safety -- --nocapture
cargo test -p core --test per_resource_write -- --nocapture
```

Expected: current implementation either reports success for unsupported paths or loses same-source resources.

- [ ] **Step 3: Replace default successful store methods**

Remove successful no-op defaults for required write operations. Add explicit capability errors or split the projection trait into reader/writer/link/segment capabilities without breaking `SqliteProjection`. `SqliteProjection` must implement every capability used by production application methods.

- [ ] **Step 4: Replace source write defaults**

Make `prepare_write` and `commit_write` return `UnsupportedCapability` unless an adapter explicitly implements them. Enforce `read_only` and `capabilities().can_write` before preparation. A missing source must be an error, never `committed: true`.

- [ ] **Step 5: Change mutation ordering and per-resource updates**

Add `expected_revision` to the mutation request path. Perform source commit before projection update. Replace `replace_source(source_id, vec![resource], ...)` in `transition_task` and attachment/resource mutation paths with `upsert_resource`/`delete_resource` or a transaction API.

- [ ] **Step 6: Run the mutation tests**

Run:

```bash
cargo test -p core --test mutation_safety
cargo test -p core --test per_resource_write
```

Expected: all failure paths leave projection unchanged and successful updates preserve sibling resources.

---

## Task 6: Add explicit `SpaceContext` and fix CLI/MCP space isolation

**Files:**
- Create: `core/src/application/context.rs`
- Modify: `core/src/application/mod.rs`
- Modify: `core/src/application/service.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/src/mcp/server.rs`
- Modify: `cli/tests/mcp/common/mod.rs`
- Test: `core/tests/application/space_context.rs`
- Test: `cli/tests/mcp/space_isolation.rs`

- [ ] **Step 1: Write failing space isolation tests**

Construct a service with an explicit root and database path while changing the process current directory to another temporary directory. Assert source configuration, blob paths, and writeback use the explicit space root. Add an MCP test that attempts to pass a different request space and asserts a structured rejection rather than using that path.

- [ ] **Step 2: Run the tests and verify current split-brain behavior**

Run:

```bash
cargo test -p core --test space_context -- --nocapture
cargo test -p cli --test space_isolation -- --nocapture
```

Expected: current `Path::new(".")`/per-request space behavior fails the assertions.

- [ ] **Step 3: Implement `SpaceContext`**

Define the context with root, stable ID, and resolved runtime configuration. Pass it into `ApplicationService` at construction. Remove all application lookups that load source/community config from `.`.

- [ ] **Step 4: Bind MCP to one context**

Remove or reject request-level space overrides. `NotezMcpServer` must use the service’s startup context and its matching projection. Return structured errors for mismatched space requests.

- [ ] **Step 5: Run isolation and CLI/MCP tests**

Run:

```bash
cargo test -p core --test space_context
cargo test -p cli --test space_isolation
cargo test -p cli --tests
```

Expected: no request can combine one space’s files/config with another space’s projection.

---

## Task 7: Split the application implementation and decouple preview

**Files:**
- Modify: `core/src/application/mod.rs`
- Create: `core/src/application/scan.rs`
- Create: `core/src/application/resources.rs`
- Create: `core/src/application/links.rs`
- Create: `core/src/application/tasks.rs`
- Create: `core/src/application/attachments.rs`
- Create: `core/src/application/artifacts.rs`
- Create: `core/src/application/sync.rs`
- Create: `core/src/application/inspect.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/preview/mod.rs`
- Modify: `core/src/preview/builders/link_embed.rs`
- Test: existing corresponding `core/tests/application/*` and `core/tests/preview/*`

- [ ] **Step 1: Move one impl group at a time without behavior changes**

Move scan, resources, links, tasks, attachments, artifacts, sync, and inspect method implementations into their named modules as `impl<S: ProjectionStore> ApplicationService<S>` blocks. Keep all public method signatures stable until the earlier tests are green.

- [ ] **Step 2: Run the affected tests after each group**

Run the narrow matching test group after each move, then:

```bash
cargo test -p core --tests
```

Expected: no behavior change; this task is structural only.

- [ ] **Step 3: Write a preview test with no service handle**

Create a preview context from already-loaded resource, bytes, segments, siblings, and a minimal loader/result if embed resolution requires lookup. Assert preview rendering works without constructing `ApplicationService<SqliteProjection>`.

- [ ] **Step 4: Replace the concrete service field**

Remove `Option<&ApplicationService<SqliteProjection>>` from `PreviewContext`. Pass prepared data or a narrow `PreviewResourceLoader` interface only where link/query embeds require it. Keep preview independent from storage implementation.

- [ ] **Step 5: Run preview and application tests**

Run:

```bash
cargo test -p core --test catalog_resolution
cargo test -p core --tests
```

Expected: preview and application behavior remain green without the reverse dependency.

---

## Task 8: Remove false capabilities and clean architecture drift

**Files:**
- Modify: `core/src/application/job_manager.rs`
- Modify: `core/src/application/writeback.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/source/apple_notes.rs`
- Modify: `core/src/source/apple_calendar.rs`
- Modify: `core/src/config/migrate.rs`
- Modify: `core/src/application/federation.rs`
- Modify: `core/src/application/community_app.rs`
- Modify: `docs/architecture.org`
- Modify: `core/src/lib.rs`
- Modify: `core/src/document/mod.rs`
- Modify: `core/src/preview/builders/block_embed.rs`
- Modify: `core/src/preview/builders/link_embed.rs`

- [ ] **Step 1: Write failing capability/error tests**

Assert that jobs, relay sync, Apple integrations, and artifact freshness either return `UnsupportedCapability` or are absent from public command/tool registration. Assert legacy JSON migration is read-only input and normal source/community writes target `notez.toml`.

- [ ] **Step 2: Implement explicit unsupported behavior**

Remove fixed empty/fresh/success responses. Keep capability descriptors truthful. Preserve migration reads only; route normal writes through the canonical space configuration.

- [ ] **Step 3: Update architecture documentation**

Document `core` as an aggregated crate with internal modules, `adapters/orgmode` and `adapters/markdown` as parser implementations, and `cli/app` as composition roots. Remove stale `crates/...` paths and nonexistent feature claims.

- [ ] **Step 4: Run full verification and smoke paths**

Run:

```bash
cargo check --workspace --all-targets
cargo test --workspace
cargo run -p cli -- --help
```

Then run the existing CLI scan/query/read flow and MCP initialize/read flow against a temporary space. Expected: build and tests pass; unsupported capabilities produce explicit nonzero/structured errors.

---

## Final acceptance checklist

- [ ] Workspace package remains named `core`; Clap and MCP schema compilation succeeds.
- [ ] Org and Markdown parsers are implemented in their adapter crates and parse payload bytes only.
- [ ] `core` has no dependency on format adapters.
- [ ] All scanning uses one registry-backed transport/parser path.
- [ ] Complete source scans are the only callers of `replace_source`.
- [ ] Single-resource mutation preserves sibling resources.
- [ ] Source/store capability failures are explicit and non-mutating.
- [ ] `SpaceContext` prevents current-directory and cross-space split-brain behavior.
- [ ] Preview no longer depends on `ApplicationService<SqliteProjection>`.
- [ ] Stub capabilities are removed or explicitly unsupported.
- [ ] Architecture documentation matches the workspace and dependency directions.
- [ ] `cargo check --workspace --all-targets` and `cargo test --workspace` pass.
