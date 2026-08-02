# ApplicationError Structured Variant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade `ApplicationError` from flat string errors to 8 structured variants carrying typed fields, serde serialisation, stable Display formats, CLI exit-code mapping, and MCP structured error JSON. All 189 existing tests (with Display-assertions rewritten) plus new error-contract tests must pass.

**Architecture:** Define the 8-variant `ApplicationError` + `StorageErrorKind` (4) + `DocumentErrorKind` (2) in `core/src/application/service.rs`. Migrate all ~45 `ApplicationError::Storage(String)` / `NotFound(String)` / `Unsupported(&str)` construction sites. Add `exit_code_for(err) -> i32` in CLI main and a `struct_err` JSON helper in MCP server. Rewrite the affected Display-assertion tests to the new stable formats.

**Tech Stack:** Rust 2024, Cargo workspace, existing `thiserror`/`serde` deps, existing `ApplicationFacade` with 9 use-case traits.

---

## File Structure

New files (2):

- `core/tests/application_error.rs` — error contract tests: construct each of the 8 variants, assert stable Display output, serde round-trip, and variant-match helpers.
- `cli/tests/api_error_exit.rs` — CLI exit-code contract tests for NotFound (3), UnsupportedCapability (6), ReadOnlySource (7), SourceNotFound (8), RevisionConflict (9), Storage (5).

Modified files (5):

- `core/src/application/service.rs` — replace the `ApplicationError` enum, add `StorageErrorKind` and `DocumentErrorKind`, migrate ~45 construction sites.
- `cli/src/main.rs` — add `exit_code_for`, replace scattered `exit(5)`/`exit(3)` in error branches.
- `cli/src/mcp/server.rs` — add `struct_err` JSON error helper, replace `text_err` in error branches.
- `cli/tests/doctor_cli.rs`, `cli/tests/evolution_cli.rs`, `cli/tests/sync_cli.rs`, `cli/tests/link_cli.rs`, `cli/tests/rules_cli.rs`, `core/tests/mutation_safety.rs`, `core/tests/space_context.rs` — rewrite Display-assertions to new formats.
- `docs/superpowers/specs/2026-08-02-application-error-design.org` — append Verification section (Task 6).

---

## Conventions

- `ApplicationError` keeps `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`.
- All 8 variants use `#[serde(tag = "kind", rename_all = "snake_case")]` on the enum.
- `StorageErrorKind` derives `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize`.
- `DocumentErrorKind` derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`.
- The old `#[from] DocumentError` / `#[from] MarkdownDocumentError` derives are removed; call sites use explicit `.map_err(|e| ApplicationError::Document { source: DocumentErrorKind::Org(e) })`.
- The Display strings in §2.2 of the spec are the stable contract; tests assert them exactly.

---

## Task 1: Define the structured error types

**Files:**
- Modify: `core/src/application/service.rs`
- Test: `core/tests/application_error.rs`

- [ ] **Step 1: Write the failing contract test**

Create `core/tests/application_error.rs`:

```rust
//! Contract tests for the structured `ApplicationError` variants.

use notez_core::application::ApplicationError;
use notez_core::capability::Mutability;
use notez_core::domain::{ResourceKind, ResourceRef};

#[test]
fn not_found_display_and_serde_round_trip() {
    let r_ref = ResourceRef::parse("heading:01J00000000000000000000F01").unwrap();
    let err = ApplicationError::NotFound {
        kind: ResourceKind::Heading,
        r_ref,
    };
    assert_eq!(err.to_string(), "resource not found: heading:01J00000000000000000000F01 (kind=heading)");
    let s = serde_json::to_string(&err).unwrap();
    let back: ApplicationError = serde_json::from_str(&s).unwrap();
    assert_eq!(back, err);
}

#[test]
fn storage_display_and_serde_round_trip() {
    let err = ApplicationError::Storage {
        kind: notez_core::application::StorageErrorKind::Sqlite,
        message: "db busy".to_string(),
    };
    assert_eq!(err.to_string(), "storage error (sqlite): db busy");
    let s = serde_json::to_string(&err).unwrap();
    let back: ApplicationError = serde_json::from_str(&s).unwrap();
    assert_eq!(back, err);
}

#[test]
fn unsupported_capability_display() {
    let err = ApplicationError::UnsupportedCapability { capability: "space doctor" };
    assert_eq!(err.to_string(), "unsupported capability: space doctor");
}

#[test]
fn source_not_found_display() {
    let err = ApplicationError::SourceNotFound { source_id: "vault".to_string() };
    assert_eq!(err.to_string(), "source not found: vault");
}

#[test]
fn read_only_source_display() {
    let err = ApplicationError::ReadOnlySource { source_id: "vault".to_string() };
    assert_eq!(err.to_string(), "read-only source: vault");
}

#[test]
fn revision_conflict_display() {
    let err = ApplicationError::RevisionConflict {
        expected: "r1".to_string(),
        actual: "r2".to_string(),
    };
    assert_eq!(err.to_string(), "revision conflict: expected r1, actual r2");
}

#[test]
fn io_display_and_serde_round_trip() {
    let err = ApplicationError::Io {
        path: None,
        source: std::io::ErrorKind::NotFound,
    };
    assert!(err.to_string().starts_with("io error: "));
    let s = serde_json::to_string(&err).unwrap();
    let back: ApplicationError = serde_json::from_str(&s).unwrap();
    assert_eq!(back, err);
}

#[test]
fn document_display_org_variant() {
    let err = ApplicationError::Document {
        source: notez_core::application::DocumentErrorKind::Org(
            notez_core::document::OrgDocumentError::Other("parse failed".into()),
        ),
    };
    assert_eq!(err.to_string(), "document error (org): parse failed");
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test application_error -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::StorageErrorKind`" (or similar — the new types and variants do not exist yet).

- [ ] **Step 3: Replace the `ApplicationError` enum in `service.rs`**

Find the current enum in `core/src/application/service.rs`:

```rust
#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Document error: {0}")]
    Document(#[from] DocumentError),
    #[error("markdown document error: {0}")]
    MarkdownDocument(#[from] crate::document::MarkdownDocumentError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("unsupported capability: {0}")]
    Unsupported(&'static str),
}
```

Replace it with:

```rust
/// The stable error taxonomy for the application layer. Every variant
/// carries typed fields; the `Display` output and the `serde` shape are
/// public contracts (see docs/superpowers/specs/2026-08-02-application-error-design.org).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplicationError {
    /// A single resource reference could not be resolved to a row.
    NotFound {
        kind: ResourceKind,
        r_ref: ResourceRef,
    },
    /// A projection/storage operation failed.
    Storage {
        kind: StorageErrorKind,
        message: String,
    },
    /// A document parser failed.
    Document { source: DocumentErrorKind },
    /// A filesystem operation failed.
    Io {
        path: Option<PathBuf>,
        source: std::io::ErrorKind,
    },
    /// A requested capability is not implemented by this build.
    UnsupportedCapability { capability: &'static str },
    /// An expected revision did not match the persisted value.
    RevisionConflict { expected: String, actual: String },
    /// A write was attempted against a read-only source.
    ReadOnlySource { source_id: String },
    /// A source with the given id is not registered in the space.
    SourceNotFound { source_id: String },
}

/// Classification of storage-layer failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageErrorKind {
    /// Failure from the SQLite projection.
    Sqlite,
    /// A required blob is missing.
    BlobMissing,
    /// No source is registered in the space.
    NoSourceRegistered,
    /// Any other invalid-state condition.
    InvalidState,
}

/// Which document parser produced the error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum DocumentErrorKind {
    Org(crate::document::OrgDocumentError),
    Markdown(crate::document::MarkdownDocumentError),
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicationError::NotFound { kind, r_ref } => {
                write!(f, "resource not found: {r_ref} (kind={})", kind.as_str())
            }
            ApplicationError::Storage { kind, message } => {
                write!(f, "storage error ({kind}): {message}")
            }
            ApplicationError::Document { source } => match source {
                DocumentErrorKind::Org(e) => write!(f, "document error (org): {e}"),
                DocumentErrorKind::Markdown(e) => write!(f, "document error (markdown): {e}"),
            },
            ApplicationError::Io { path, source } => match path {
                Some(p) => write!(f, "io error: {source} at {}", p.display()),
                None => write!(f, "io error: {source}"),
            },
            ApplicationError::UnsupportedCapability { capability } => {
                write!(f, "unsupported capability: {capability}")
            }
            ApplicationError::RevisionConflict { expected, actual } => {
                write!(f, "revision conflict: expected {expected}, actual {actual}")
            }
            ApplicationError::ReadOnlySource { source_id } => {
                write!(f, "read-only source: {source_id}")
            }
            ApplicationError::SourceNotFound { source_id } => {
                write!(f, "source not found: {source_id}")
            }
        }
    }
}

impl std::error::Error for ApplicationError {}
```

Also add a `Display` impl for `StorageErrorKind`:

```rust
impl std::fmt::Display for StorageErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageErrorKind::Sqlite => f.write_str("sqlite"),
            StorageErrorKind::BlobMissing => f.write_str("blob_missing"),
            StorageErrorKind::NoSourceRegistered => f.write_str("no_source_registered"),
            StorageErrorKind::InvalidState => f.write_str("invalid_state"),
        }
    }
}
```

Ensure `ResourceKind`, `ResourceRef`, and `PathBuf` are in scope (they already are via the file's `use` statements; if `ResourceKind::as_str` is not public, check — it is, per the domain module).

- [ ] **Step 4: Fix the `DocumentError` alias usage**

The old enum used `DocumentError` (an alias imported at the top of `service.rs` as `OrgDocumentError as DocumentError`). The new `ApplicationError::Document { source }` no longer uses that alias. Check whether `DocumentError` is still referenced elsewhere in the file; if not, remove the `use crate::document::{OrgDocumentError as DocumentError, OrgScanner};` import and keep only `OrgScanner` if still used (it is used in `scan_native`). Adjust the import to `use crate::document::{OrgDocumentError, OrgScanner};` if needed.

- [ ] **Step 5: Run the contract test and verify it passes**

Run: `cargo test -p core --test application_error`
Expected: The 8 tests PASS. (The rest of the workspace will fail to compile because ~45 call sites still reference the old `Storage(String)`/`NotFound(String)`/`Unsupported(&str)` constructors — that is expected and fixed in Task 2.)

- [ ] **Step 6: Verify the enum compiles in isolation**

Run: `cargo check -p core --lib`
Expected: compile errors at the ~45 old construction sites (expected, Task 2 fixes them). If there are errors OTHER than the old-constructor calls, fix them (e.g. missing imports) before proceeding.

- [ ] **Step 7: Commit**

```bash
git add core/src/application/service.rs core/tests/application_error.rs
git commit -m "feat(error): define structured ApplicationError variants with stable Display and serde"
```

---

## Task 2: Migrate `service.rs` construction sites

**Files:**
- Modify: `core/src/application/service.rs`

- [ ] **Step 1: Find every old-constructor call**

Run:

```bash
grep -nE "ApplicationError::(Storage|NotFound|Unsupported)\(" core/src/application/service.rs
```

Expected: ~45 hits. Each must be migrated.

- [ ] **Step 2: Migrate `ApplicationError::Storage(e.to_string())` from store failures**

For each `.map_err(|e| ApplicationError::Storage(e.to_string()))?` where `e` is the `S::Error` from a `ProjectionStore` method (e.g. `replace_source`, `upsert_resource`, `delete_resource`, `query`, `get`, `insert_segments`, `query_segments`), replace with:

```rust
.map_err(|e| ApplicationError::Storage {
    kind: StorageErrorKind::Sqlite,
    message: e.to_string(),
})?
```

(These are the majority. `StorageErrorKind::Sqlite` is correct because `ProjectionStore::Error` is `StorageError` for the SQLite implementation.)

- [ ] **Step 3: Migrate `ApplicationError::Storage(...)` from `SpaceSourcesConfig::load/save`**

Find calls like `.load(space_root)?` returning `std::io::Error` and `cfg.save(space_root)?`. These currently bubble as `ApplicationError::Storage(...)` via the `?` on the function's `Result<_, ApplicationError>` return? No — they use `?` directly, so they need explicit mapping. Locate them (search for `SpaceSourcesConfig::load(space_root)` inside `add_source`, `list_sources`, `scan_federation`, `writeback_resource`). Wrap with:

```rust
.map_err(|e| ApplicationError::Storage {
    kind: StorageErrorKind::InvalidState,
    message: e.to_string(),
})?
```

- [ ] **Step 4: Migrate `ApplicationError::Storage("no sources registered ...")`**

The `writeback_resource` function currently returns:

```rust
return Err(ApplicationError::Storage(format!(
    "no sources registered in space `{}`",
    space.space_id
)));
```

Replace with:

```rust
return Err(ApplicationError::Storage {
    kind: StorageErrorKind::NoSourceRegistered,
    message: format!("no sources registered in space `{}`", space.space_id),
});
```

- [ ] **Step 5: Migrate `ApplicationError::Storage("source ... is not registered ...")`**

The `writeback_resource` function currently returns:

```rust
.ok_or_else(|| {
    ApplicationError::Storage(format!(
        "source `{source_id}` is not registered in space `{}`",
        space.space_id
    ))
})?
```

Replace with:

```rust
.ok_or_else(|| ApplicationError::SourceNotFound {
    source_id: source_id.to_string(),
})?
```

- [ ] **Step 6: Migrate `ApplicationError::Storage("source ... is read-only")`**

Replace with:

```rust
return Err(ApplicationError::ReadOnlySource {
    source_id: source_id.to_string(),
});
```

- [ ] **Step 7: Migrate `ApplicationError::NotFound(r_ref.to_string())`**

Search for `ApplicationError::NotFound(`. There are two uses:
- In `transition_task`: `.ok_or_else(|| ApplicationError::NotFound(r_ref.to_string()))?` → replace with `.ok_or_else(|| ApplicationError::NotFound { kind: r_ref.kind(), r_ref: r_ref.clone() })?`.
- In `run_extraction`: `.ok_or_else(|| ApplicationError::NotFound(att_ref.to_string()))?` → replace with `.ok_or_else(|| ApplicationError::NotFound { kind: att_ref.kind(), r_ref: att_ref.clone() })?`.
- In `derive_artifact` / `export_skill` community lookup: `.ok_or_else(|| ApplicationError::NotFound(format!("community {community_id}")))?` → replace with `.ok_or_else(|| ApplicationError::NotFound { kind: ResourceKind::Document, r_ref: ResourceRef::parse("document:01J00000000000000000000XXX").unwrap_or_else(|_| ResourceRef::new(ResourceKind::Document, ulid::Ulid::nil())) })?`. (Note: the community id is not a `ResourceRef`; the simplest faithful mapping is to keep the message context. Prefer keeping the semantics: use `ApplicationError::Storage { kind: StorageErrorKind::InvalidState, message: format!("community {community_id} not found") }` for these two — the original text was "community not found" via `NotFound`, but a community id is not a ResourceRef. Choose the Storage/InvalidState mapping for community lookups to avoid fabricating a fake ResourceRef.)
- In `query_segments` blob hash lookup: `.ok_or_else(|| ApplicationError::NotFound(format!("blob hash {hash}")))?` → replace with `.ok_or_else(|| ApplicationError::Storage { kind: StorageErrorKind::BlobMissing, message: format!("blob hash {hash}") })?`.
- In `add_attachment` missing-hash-property: `.ok_or_else(|| ApplicationError::Storage("missing hash property".to_string()))?` → replace with `.ok_or_else(|| ApplicationError::Storage { kind: StorageErrorKind::BlobMissing, message: "missing hash property".to_string() })?`.

- [ ] **Step 8: Migrate `ApplicationError::Document(crate::document::OrgDocumentError::Other(...))`**

Search for `ApplicationError::Document(`. There are three uses (in `transition_task`, `run_extraction`, `derive_artifact`). Replace each with:

```rust
ApplicationError::Document {
    source: DocumentErrorKind::Org(crate::document::OrgDocumentError::Other(e.to_string())),
}
```

(For `run_extraction`, the `e` is an extraction error string; the mapping stays `Org(...Other(...))`.)

- [ ] **Step 9: Migrate `ApplicationError::Unsupported("...")`**

Search for `ApplicationError::Unsupported(`. There are 5 uses (list_conflicts, space_doctor, list_jobs, check_artifact_freshness, relay_sync). Replace each with:

```rust
ApplicationError::UnsupportedCapability { capability: "<same string>" }
```

Keep the exact same string values as the current `Unsupported(&'static str)` payloads so downstream message substrings like `"space doctor"` remain.

- [ ] **Step 10: Migrate `ApplicationError::Io(#[from] std::io::Error)` call sites**

Search for `.map_err(ApplicationError::Io)?` and similar. The old `Io(#[from] std::io::Error)` is now `Io { path: Option<PathBuf>, source: ErrorKind }`. Replace each call site. For example, `blob_store.store_bytes(...).map_err(ApplicationError::Io)?` becomes:

```rust
.map_err(|e| ApplicationError::Io { path: Some(file_path.to_path_buf()), source: e.kind() })?
```

For sites without a path, use `path: None`.

- [ ] **Step 11: Fix `?`-propagation of `DocumentError` and `MarkdownDocumentError`**

The old enum had `Document(#[from] DocumentError)` and `MarkdownDocument(#[from] MarkdownDocumentError)`, so `?` on a `Result<_, OrgDocumentError>` auto-converted. The new enum has no `#[from]`, so any remaining `?` on a `Result<_, DocumentError>` or `Result<_, MarkdownDocumentError>` will fail to compile. Search for call sites that use `?` on scanner results (e.g. `OrgScanner::scan(&path, "native")?` inside `scan_native` — note this was already rewritten to go through the registry in the SourceRegistry slice, but any remaining direct `OrgScanner::scan` calls need explicit mapping). For each, wrap:

```rust
.map_err(|e| ApplicationError::Document {
    source: DocumentErrorKind::Org(e),
})?
```

or `Markdown(e)` as appropriate.

- [ ] **Step 12: Verify the workspace compiles**

Run: `cargo check --workspace --all-targets`
Expected: exit 0. If any old-constructor call remains, `grep -nE "ApplicationError::(Storage|NotFound|Unsupported)\\("` again and fix.

- [ ] **Step 13: Run the contract test**

Run: `cargo test -p core --test application_error`
Expected: 8 PASS.

- [ ] **Step 14: Commit**

```bash
git add core/src/application/service.rs
git commit -m "refactor(error): migrate service.rs to structured ApplicationError variants"
```

---

## Task 3: Add `exit_code_for` and migrate CLI error branches

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add the `exit_code_for` helper**

Add a free function near the top of `cli/src/main.rs` (after the imports, before `fn main`):

```rust
/// Map an `ApplicationError` to a stable process exit code.
///
/// The mapping is part of the CLI contract and must not change without
/// a coordinated release (see the error design spec).
fn exit_code_for(err: &notez_core::application::ApplicationError) -> i32 {
    use notez_core::application::ApplicationError;
    match err {
        ApplicationError::NotFound { .. } => 3,
        ApplicationError::Storage { .. } => 5,
        ApplicationError::Document { .. } => 5,
        ApplicationError::Io { .. } => 5,
        ApplicationError::UnsupportedCapability { .. } => 6,
        ApplicationError::ReadOnlySource { .. } => 7,
        ApplicationError::SourceNotFound { .. } => 8,
        ApplicationError::RevisionConflict { .. } => 9,
    }
}
```

- [ ] **Step 2: Find all error-exit sites in main.rs**

Run:

```bash
grep -nE "exit\([0-9]+\);" cli/src/main.rs
```

Categorise each: `exit(3)` sites are `NotFound` mappings (currently hard-coded), `exit(5)` sites are `Storage`/`Document`/`Io` mappings, `exit(2)` sites are config/discovery failures (NOT `ApplicationError` — leave them at 2), and `exit(6)`/`7`/`8`/`9` don't exist yet.

- [ ] **Step 3: Replace `exit(3)` with `exit_code_for(&err)` in NotFound branches**

Find each `Ok(ResolveResult::NotFound) => { ... exit(3); }` block and the `ResourceUseCase::read` / `ResourceUseCase::resolve` error branches that currently `exit(3)`. The `NotFound` branches that print "Resource not found" and `exit(3)` should call `exit(exit_code_for(&ApplicationError::NotFound { ... }))` — but since the `NotFound` path is a `ResolveResult` variant rather than an error, the simplest faithful change is to keep `exit(3)` where it represents a resolve-miss and add the `exit_code_for` call where the branch has an actual `ApplicationError` value in scope.

For branches with an `ApplicationError` in scope (`Err(e) => { eprintln!(...); exit(N); }`), replace the `exit(N)` with `exit(exit_code_for(&e))`. The `Err(e)` variable is always in scope in those branches.

For `ResolveResult::NotFound` branches (which are not errors), keep `exit(3)` — it already matches `exit_code_for(NotFound)`.

- [ ] **Step 4: Replace `exit(5)` with `exit_code_for(&e)` in `Err(e)` branches**

Find every `Err(e) => { eprintln!("..."); exit(5); }` block. Replace `exit(5)` with `exit(exit_code_for(&e))`.

- [ ] **Step 5: Verify the CLI builds and smoke works**

Run: `cargo build -p cli && target/debug/notez --space /tmp/notez-smoke scan`
Expected: builds; scan prints "Scanned 1 files, 2 resources, 0 relations." (assuming `/tmp/notez-smoke` still exists; if not, use any space with a `.org` file).

- [ ] **Step 6: Verify exit codes by hand**

Run:

```bash
target/debug/notez --space /tmp/notez-smoke resolve definitely_missing_query; echo "exit=$?"
```

Expected: exit=3 (NotFound).

```bash
target/debug/notez --space /tmp/notez-smoke space doctor; echo "exit=$?"
```

Expected: exit=6 (UnsupportedCapability).

```bash
target/debug/notez --space /tmp/notez-smoke sync relay --id missing_src; echo "exit=$?"
```

Expected: exit=6 (UnsupportedCapability).

- [ ] **Step 7: Run the workspace tests that exercise exit codes**

Run: `cargo test -p cli --test doctor_cli --test evolution_cli --test sync_cli --test link_cli --test rules_cli`
Expected: FAIL — these tests assert the old exit code 5 for UnsupportedCapability paths. Task 5 rewrites them.

- [ ] **Step 8: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat(cli): map ApplicationError variants to stable exit codes"
```

---

## Task 4: Add `struct_err` to MCP and migrate error branches

**Files:**
- Modify: `cli/src/mcp/server.rs`

- [ ] **Step 1: Add the `struct_err` helper**

Find the existing `text_err` helper (it returns `Result<Value, ErrorData>`. Err(...) with a text message). Add next to it:

```rust
/// Build an MCP error whose payload is a structured JSON object with
/// `kind` (snake_case) and `message` (the Display text). This is the
/// stable MCP error contract; see the error design spec.
fn struct_err(err: &notez_core::application::ApplicationError) -> Result<Value, McpError> {
    let kind = match err {
        notez_core::application::ApplicationError::NotFound { .. } => "not_found",
        notez_core::application::ApplicationError::Storage { .. } => "storage",
        notez_core::application::ApplicationError::Document { .. } => "document",
        notez_core::application::ApplicationError::Io { .. } => "io",
        notez_core::application::ApplicationError::UnsupportedCapability { .. } => "unsupported_capability",
        notez_core::application::ApplicationError::ReadOnlySource { .. } => "read_only_source",
        notez_core::application::ApplicationError::SourceNotFound { .. } => "source_not_found",
        notez_core::application::ApplicationError::RevisionConflict { .. } => "revision_conflict",
    };
    Err(McpError::internal(format!(
        "{}",
        serde_json::json!({ "kind": kind, "message": err.to_string() })
    )))
}
```

(Adapt the exact `McpError` type — check how `text_err` is defined in this file and mirror its error-construction shape, e.g. `McpError::internal(...)` or `text_err`'s underlying mechanism. If `McpError` is `rmcp::ErrorData`, use the same constructor that `text_err` uses.)

- [ ] **Step 2: Find every `text_err(format!(...))` call that wraps an `ApplicationError`**

Run:

```bash
grep -nE "text_err|format!\(" cli/src/mcp/server.rs | head -60
```

For each handler that receives an `Err(e)` where `e: ApplicationError` and currently does `Err(McpError::internal(format!("{}", e)))` or `text_err(e.to_string())`, replace with `struct_err(&e)`.

- [ ] **Step 3: Leave non-ApplicationError text errors unchanged**

The `text_err` calls for DTO-validation / parameter errors (e.g. "Unknown source kind") are NOT `ApplicationError`; leave them as `text_err`.

- [ ] **Step 4: Verify the MCP builds**

Run: `cargo build -p cli`
Expected: exit 0.

- [ ] **Step 5: Run the MCP tests**

Run: `cargo test -p cli --test mcp`
Expected: FAIL — the MCP tests assert text error messages; Task 5 rewrites them.

- [ ] **Step 6: Commit**

```bash
git add cli/src/mcp/server.rs
git commit -m "feat(mcp): return structured error JSON from tool handlers"
```

---

## Task 5: Rewrite Display-assertions in affected tests

**Files:**
- Modify: `cli/tests/doctor_cli.rs`, `cli/tests/evolution_cli.rs`, `cli/tests/sync_cli.rs`, `cli/tests/link_cli.rs`, `cli/tests/rules_cli.rs`, `core/tests/mutation_safety.rs`, `core/tests/space_context.rs`

- [ ] **Step 1: Run the full workspace test to enumerate failures**

Run: `cargo test --workspace 2>&1 | tee /tmp/error-test.log | grep -E "FAILED|panicked" | head -60`

Categorise the failures:

- Tests asserting `"unsupported capability"` in stderr → the new Display is `"unsupported capability: <capability>"` — the substring `"unsupported capability"` still matches, so these may pass unchanged. Verify each.
- Tests asserting `"Storage error: ..."` → new Display is `"storage error (sqlite): ..."` — substring `"storage error"` still matches (lowercase vs title-case! Check the case: old was "Storage error:", new is "storage error ("). Tests asserting `"Storage error"` (capital S) will FAIL on case; rewrite to `"storage error"`.
- Tests asserting `"no sources registered"` → now `Storage { NoSourceRegistered, ... }` → Display `"storage error (no_source_registered): no sources registered in space ..."` — substring `"no sources registered"` still matches.
- Tests asserting `"is not registered"` → now `SourceNotFound` → Display `"source not found: vault"` — the substring `"is not registered"` NO LONGER matches. Rewrite these to assert `"source not found"`.
- Tests asserting `"Resource not found"` / `"not found"` for resolve/read → new Display `"resource not found: ..."` — substring `"not found"` matches but `"Resource not found"` (capital R) does not. Rewrite to lowercase `"resource not found"`.
- Tests asserting `"corrupt.org"` (scan_corrupt_org test in vertical_slice) → the scanner error still surfaces via `DocumentErrorKind::Org`, Display `"document error (org): ... corrupt.org ..."` — the `"corrupt.org"` substring still matches.

- [ ] **Step 2: Rewrite each failing assertion**

For each failure, open the test file and change the predicate:

- `predicates::str::contains("unsupported capability")` → unchanged (lowercase matches).
- `predicates::str::contains("Storage error")` → `predicates::str::contains("storage error")`.
- `predicates::str::contains("is not registered")` → `predicates::str::contains("source not found")`.
- `predicates::str::contains("no sources registered")` → unchanged (lowercase already matches `"no sources registered"` inside the new Display).
- `predicates::str::contains("Resource not found")` → `predicates::str::contains("resource not found")`.
- `assert!(err.to_string().contains("corrupt.org"))` → unchanged.
- `assert!(err.to_string().contains("01J..."))` for transition-task NotFound → the new Display includes the `r_ref` so it still matches; verify case-sensitive substrings.
- `assert!(msg.contains("registered"))` in evolution_cli writeback test → the new Display `"source not found: anytype_src"` does NOT contain "registered". Rewrite to assert `"source not found"`.

- [ ] **Step 3: Run the workspace test until green**

Run: `cargo test --workspace 2>&1 | tee /tmp/error-test2.log | grep -E "FAILED|panicked" | head -40`
Iterate: fix each failure, re-run. Stop when 0 failed.

Expected final: 189 tests + 8 application_error + 7 api_error_exit (from Task 6) = 204 total, 0 failed. (If `api_error_exit` is added in Task 6, the count here is 197 until then.)

- [ ] **Step 4: Commit**

```bash
git add cli/tests core/tests
git commit -m "test(error): rewrite Display-assertions to structured ApplicationError formats"
```

---

## Task 6: Add CLI exit-code contract tests and final verification

**Files:**
- Create: `cli/tests/api_error_exit.rs`
- Modify (audit only): `docs/superpowers/specs/2026-08-02-application-error-design.org`

- [ ] **Step 1: Write the exit-code contract test**

Create `cli/tests/api_error_exit.rs`:

```rust
//! Exit-code contract for `ApplicationError` variants via the CLI.

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

fn warm_space(space: &std::path::Path) {
    let doc = space.join("note.org");
    fs::write(
        &doc,
        "#+title: Healthy\n#+ID: 01J00000000000000000000E01\n",
    )
    .unwrap();
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("scan")
        .assert()
        .success();
}

#[test]
fn resolve_missing_is_exit_3_not_found() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("resolve")
        .arg("no_such_title_anywhere")
        .assert()
        .code(3);
}

#[test]
fn space_doctor_is_exit_6_unsupported_capability() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("space")
        .arg("doctor")
        .assert()
        .code(6);
}

#[test]
fn sync_relay_is_exit_6_unsupported_capability() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("sync")
        .arg("relay")
        .arg("--id")
        .arg("anytype_src")
        .assert()
        .code(6);
}

#[test]
fn source_writeback_unknown_source_is_exit_8() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("source")
        .arg("writeback")
        .arg("--id")
        .arg("missing_src")
        .arg("--r-ref")
        .arg("heading:01J00000000000000000000E02")
        .arg("--payload")
        .arg("payload")
        .assert()
        .code(8);
}

#[test]
fn task_transition_missing_resource_is_exit_3() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("task")
        .arg("transition")
        .arg("heading:01J00000000000000000000E99")
        .arg("--to")
        .arg("DONE")
        .assert()
        .code(3);
}
```

- [ ] **Step 2: Run the exit-code tests**

Run: `cargo test -p cli --test api_error_exit`
Expected: PASS (5 tests).

- [ ] **Step 3: Run the full workspace test**

Run: `cargo test --workspace`
Expected: 0 failed. Total should be 197 + 5 = 202, plus the 8 application_error from Task 1 = 204 if the workspace was already at 189 and Task 1 added 8 and Task 6 adds 5 — the exact count depends on how many of the 189 were rewritten vs unchanged; the invariant is 0 failed.

- [ ] **Step 4: Run the workspace build**

Run: `cargo check --workspace --all-targets`
Expected: exit 0.

- [ ] **Step 5: Verify the exit codes by hand once more**

Run:

```bash
target/debug/notez --space /tmp/notez-smoke space doctor; echo "doctor=$?"
target/debug/notez --space /tmp/notez-smoke resolve definitely_missing; echo "resolve=$?"
```

Expected: `doctor=6`, `resolve=3`.

- [ ] **Step 6: Update the spec with verification counts**

Append a "Verification" section to `docs/superpowers/specs/2026-08-02-application-error-design.org`:

```org
** 8. Verification

- cargo test --workspace: <N> tests passed, 0 failed.
- cargo check --workspace --all-targets: exit 0.
- CLI exit codes: resolve-miss → 3, space doctor → 6, sync relay → 6, source writeback missing → 8, task transition missing → 3.
- MCP error responses are structured JSON ({ "kind": "...", "message": "..." }).
```

Replace `<N>` with the actual test count from Step 3.

- [ ] **Step 7: Commit**

```bash
git add cli/tests/api_error_exit.rs docs/superpowers/specs/2026-08-02-application-error-design.org
git commit -m "test(error): add CLI exit-code contract tests and record verification"
```

---

## Self-Review Notes

**Spec coverage:**

- §2.1 8 variants + StorageErrorKind + DocumentErrorKind — Task 1.
- §2.2 stable Display formats — Task 1 (impl) + Task 5 (test rewrites).
- §2.3 serde round-trip — Task 1 (derive + tests).
- §2.4 CLI exit_code_for — Task 3.
- §2.5 MCP struct_err — Task 4.
- §2.6 45-site migration — Task 2.
- §2.7 compatibility (`#[from]` removal) — Task 2 Steps 8-11.
- §3 verification — Task 6.
- §4 migration order — Tasks 1-6 in order.
- §5 risks — addressed: case-sensitivity (Task 5 Step 1), capability strings preserved (Task 2 Step 9), `#[from]` removal (Task 2 Step 11), MCP text→JSON (Task 4 + Task 5).
- §6 non-goals — no task touches SourceError/Registry/CapabilityCatalog/use-case traits.
- §7 acceptance — covered by Task 6.

**Placeholder scan:** 0 occurrences of "TBD/TODO/FIXME/待定" in the plan body (the Self-Review Notes section mentions the scan itself, which is fine).

**Type consistency:** `ApplicationError::NotFound { kind, r_ref }` is constructed in Task 2 Step 7 with `r_ref.kind()` and `r_ref.clone()`; the contract test in Task 1 Step 1 uses the same field names. `exit_code_for` matches all 8 variants exhaustively. `struct_err` matches all 8 variants exhaustively. `DocumentErrorKind::Org` / `Markdown` wrap the same types that were previously `#[from]`-converted.
