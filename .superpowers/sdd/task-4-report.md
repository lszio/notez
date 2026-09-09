# Task 4 Report

Implemented the engine dispatcher and ingestion spine in `crates/engine`.

## Changes

- Removed unused imports and removed the unused placeholder projector/summary/query modules from compilation.
- Added application-level non-empty `expected_revision` validation for Rust-constructed commands.
- `UpdateDocument` now authorizes, rereads the source, checks the revision before calling `apply`, and records journal plus audit outcomes.
- Watch ingestion validates batch identifiers/events, rereads the source when configured, rejects revision drift as retryable, and preserves duplicate idempotency.
- Query dispatch now maps each supported protocol query to an explicit capability-specific `Unsupported` result instead of a generic empty implementation. This is testable until projection/catalog ports are available.
- Added regression coverage for empty revisions and explicit query capability reporting.

## Verification

- `cargo test -p notez-engine` passed.
- `cargo test -p notez-protocol --test contract_parity` passed.
- `cargo test --workspace` compiled the workspace and emitted existing warnings outside engine; the command output was too large for the tool preview. A second redirected confirmation was attempted but the command classifier timed out.

The focused engine build is warning-free after the changes.
