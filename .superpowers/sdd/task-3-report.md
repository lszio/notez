# Task 3 Report

## Status

DONE

## Scope

Established the `notez-ports` contract crate surface with object-safe source, projection, journal, audit, watcher-ingestion, clock, and blob ports. Added typed source/projection snapshots, patches, outcomes, and infrastructure errors without exposing SQLite, Axum, Dioxus, or filesystem handles.

Extended `notez-protocol` with typed `Command`, `Query`, `ObjectAddress`, `GraphQuery`, `IngestWatchBatch`, command outcomes, structured error variants, response DTOs, and schema registration. Existing `Request`/`Response` compatibility remains intact. No engine or surface migration was performed.

## TDD Evidence

1. Added `crates/ports/tests/object_safe_ports.rs` and `crates/protocol/tests/contract_parity.rs` before production contract implementation.
2. Ran both new targets and confirmed expected RED failures from missing ports, command/query types, command result, and error field mismatch.
3. Implemented the minimum contract types and reran the targets GREEN.

## Impact Analysis

GitNexus was unavailable in this workspace (`.gitnexus` absent). Per repository fallback, ran `rg` symbol/reference scans over `crates` and `packages` before editing. The scan found existing protocol consumers and confirmed the only changed legacy error field remained `actual`; new command/query types are additive. No engine or surface files were changed.

## Verification

- `cargo test -p notez-protocol --test contract_parity`: 4 passed.
- `cargo test -p notez-ports --test object_safe_ports`: 2 passed.
- `cargo test -p notez-protocol`: 3 unit tests, 4 integration tests, doc tests passed.
- `cargo run -p cli -- list-capabilities --json`: passed; emitted the existing 9 capabilities.
- `cargo test -p notez-ports && cargo test -p notez-protocol && cargo test --workspace`: passed, exit code 0.
- `git diff --check`: passed.

Workspace verification retained pre-existing test warnings in unrelated test code; no new production warnings were introduced by the contract additions.

## Review Repair (2026-09-07)

The first review identified incomplete protocol parity. Added failing contract tests before production changes for canonical watcher wire fields (`batch_id`, `space_id`, `events`), non-empty mutation revisions, principal/space authorization scope, graph principal identity and positioned fingerprints, all Space/Document/Object/Graph/Summary/Watch response variants, complete command/query/error JSON round trips, and schema coverage. Production contracts now enforce those requirements. `UpdateDocument` also carries `space_id`; `Query::Graph` carries principal identity directly.

The initial red run failed on the missing fields, variants, fingerprint, graph identity, and revision validation. After the minimal contract changes, all new tests passed.

## Concerns

The new ports are intentionally contracts only. Existing core infrastructure implementations still use their current internal/domain port traits and are not migrated in Task 3, as required. The new unified `Command`/`Query` types are likewise not wired into the existing dispatcher yet; surfaces continue using legacy `Request` until a later migration task.
