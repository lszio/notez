# UseCase Trait Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `ApplicationService<S>`'s 44 public `fn` surface into 9 `UseCase` traits and re-route through an `ApplicationFacade<S>` while keeping every existing test green.

**Architecture:** 9 trait files under `core/src/application/use_cases/`, one per concern (scan, resource, link, task, attachment, community, artifact, sync, inspect). `ApplicationFacade<S>` `impl`s all 9 traits and exposes the original 44 `pub fn` as one-line forwarders. `ApplicationService<S>` is kept as a `pub type` alias for compatibility. CLI and MCP call sites move to trait methods. A no-op `register_capability` hook on the facade opens the future P1 capability directory without behavior change.

**Tech Stack:** Rust 2024, Cargo workspace, `core` lib (package name) / `notez_core` (crate name) already in place, `clap` for CLI, `rmcp` for MCP, existing `ProjectionStore` trait and `SqliteProjection` impl.

---

## File Structure

New files (12):

- `core/src/capability.rs` — minimal `CapabilityDescriptor` struct.
- `core/src/application/use_cases/mod.rs` — re-exports for the 9 traits.
- `core/src/application/use_cases/scan.rs` — `ScanUseCase` trait.
- `core/src/application/use_cases/resource.rs` — `ResourceUseCase` trait.
- `core/src/application/use_cases/link.rs` — `LinkUseCase` trait.
- `core/src/application/use_cases/task.rs` — `TaskUseCase` trait.
- `core/src/application/use_cases/attachment.rs` — `AttachmentUseCase` trait.
- `core/src/application/use_cases/community.rs` — `CommunityUseCase` trait.
- `core/src/application/use_cases/artifact.rs` — `ArtifactUseCase` trait.
- `core/src/application/use_cases/sync.rs` — `SyncUseCase` trait.
- `core/src/application/use_cases/inspect.rs` — `InspectUseCase` trait.
- `core/tests/use_case_scan.rs` … `core/tests/use_case_inspect.rs` (9 test files, one per trait).

Modified files (4):

- `core/src/application/mod.rs` — `pub mod use_cases;` + re-exports.
- `core/src/lib.rs` — `pub mod capability;`.
- `core/src/application/service.rs` — replace `pub struct ApplicationService<S>` with `ApplicationFacade<S>`, add `pub type ApplicationService<S> = ApplicationFacade<S>;`, add 44 forwarder `pub fn`s, add `register_capability` no-op, keep all method bodies.
- `core/src/application/context.rs` — unchanged.
- `cli/src/main.rs` — every `service.<fn>(...)` becomes `<Service as Trait>::fn(&mut service, ...)` (or `&service` for `&self` methods).
- `cli/src/mcp/server.rs` — every `service.<fn>(...)` similarly migrated.
- `cli/src/mcp/common/mod.rs` — update test harness that constructs `ApplicationService<SqliteProjection>` (now `ApplicationFacade<SqliteProjection>`).

The 144 existing tests must remain green throughout.

---

## Conventions

- All trait method bodies are written in `core/src/application/use_cases/<trait>.rs` as `impl<S: ProjectionStore> <AppFacadeOrExt> for ApplicationFacade<S> { ... }` blocks.
- Forwarder `pub fn` in `service.rs` always looks like `<Self as Trait>::fn(self, ...)` (taking `&self` or `&mut self` as appropriate).
- Each new trait is `Send + Sync` only if required. All traits are not `dyn`-compatible by default; we use static dispatch via `ApplicationFacade<S>` only. (Dyn support is not part of this plan.)
- `CapabilityDescriptor` is a public struct in `core::capability`. It does not have behavior.

---

## Task 1: Introduce `CapabilityDescriptor` and `register_capability` no-op

**Files:**
- Create: `core/src/capability.rs`
- Modify: `core/src/lib.rs:22-30` — add `pub mod capability;`
- Test: `core/tests/capability_descriptor.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for the public `CapabilityDescriptor` and the
//! `register_capability` no-op on `ApplicationFacade`.

use notez_core::application::ApplicationFacade;
use notez_core::capability::{CapabilityDescriptor, Mutability};
use notez_core::storage::SqliteProjection;
use std::path::PathBuf;

#[test]
fn capability_descriptor_constructor_and_accessors() {
    let desc = CapabilityDescriptor::new("scan", "scan native source", Mutability::Read);
    assert_eq!(desc.id, "scan");
    assert_eq!(desc.description, "scan native source");
    assert_eq!(desc.mutability, Mutability::Read);
}

#[test]
fn register_capability_is_a_no_op_that_accepts_descriptors() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    let desc = CapabilityDescriptor::new("scan", "scan native source", Mutability::Read);
    // Must not panic, must not error, must not change observable state.
    facade.register_capability(&desc);
    facade.register_capability(&desc);
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test capability_descriptor -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::capability`" (or similar — both the trait method and the struct are not yet defined).

- [ ] **Step 3: Implement `CapabilityDescriptor` and `Mutability`**

`core/src/capability.rs`:

```rust
//! Public capability descriptors.
//!
//! This module exists so that subsequent P1 capability-directory work
//! has a stable public surface to extend. The current revision only
//! ships the data type and the [`Mutability`] enum. `ApplicationFacade`
//! accepts a descriptor via `register_capability` but the call is a
//! no-op; it is intentionally a future hook, not a behavior.

use serde::{Deserialize, Serialize};

/// Whether a capability can mutate persisted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mutability {
    /// Pure read against the current state.
    Read,
    /// May change persisted state.
    Write,
}

/// Stable description of a capability exposed by the system. Capability
/// descriptors are the unit that MCP tools, CLI subcommands, and
/// previewer registrations will eventually consume. Today they are only
/// accepted by `ApplicationFacade::register_capability` as a no-op.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub id: &'static str,
    pub description: &'static str,
    pub mutability: Mutability,
}

impl CapabilityDescriptor {
    pub fn new(
        id: &'static str,
        description: &'static str,
        mutability: Mutability,
    ) -> Self {
        Self { id, description, mutability }
    }
}
```

`core/src/lib.rs` (modify the module list — add a new line):

```rust
pub mod capability;
pub mod domain;
pub mod storage;
pub mod document;
pub mod source;
pub mod config;
pub mod application;
pub mod sync;
pub mod artifact;
pub mod preview;
```

(Insert `pub mod capability;` before `pub mod domain;`.)

- [ ] **Step 4: Add the no-op `register_capability` method to `ApplicationFacade`**

We are not yet renaming `ApplicationService` — that comes in Task 2. For this step, edit `core/src/application/service.rs` and add the method to the existing `ApplicationService` struct. The method body is empty.

Find the existing `impl<S: ProjectionStore> ApplicationService<S>` block and add the method near `register_format_parser`:

```rust
    /// Register a public capability descriptor. This is a no-op in the
    /// current revision; it exists so that the future P1 capability
    /// directory can collect descriptors without a breaking change.
    /// See `core::capability::CapabilityDescriptor`.
    pub fn register_capability(&mut self, _descriptor: &notez_core::capability::CapabilityDescriptor) {
        // Intentionally a no-op. See the doc-comment above.
    }
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p core --test capability_descriptor`
Expected: PASS (both tests).

- [ ] **Step 6: Verify workspace still builds**

Run: `cargo check --workspace --all-targets`
Expected: exit 0.

- [ ] **Step 7: Commit**

```bash
git add core/src/capability.rs core/src/lib.rs core/src/application/service.rs core/tests/capability_descriptor.rs
git commit -m "feat(capability): introduce CapabilityDescriptor and no-op register_capability"
```

---

## Task 2: Rename `ApplicationService` to `ApplicationFacade` and add type alias

**Files:**
- Modify: `core/src/application/service.rs:38-46` — rename struct
- Modify: `core/src/application/service.rs:22` (re-export) — adjust
- Modify: `core/src/application/mod.rs:22` — add type alias re-export
- Test: `core/tests/application_facade_rename.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Verify that the type alias `ApplicationService` still resolves to the
//! renamed `ApplicationFacade` and that the new name is exported from
//! `notez_core::application`.

use notez_core::application::{ApplicationFacade, ApplicationService};
use notez_core::storage::SqliteProjection;
use std::any::TypeId;

#[test]
fn application_service_is_a_type_alias_for_application_facade() {
    assert_eq!(
        TypeId::of::<ApplicationService<SqliteProjection>>(),
        TypeId::of::<ApplicationFacade<SqliteProjection>>(),
    );
}

#[test]
fn application_facade_can_be_constructed_without_space() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let _facade = ApplicationFacade::new(store);
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test application_facade_rename -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::ApplicationFacade`" (or "no `ApplicationFacade` in `application`").

- [ ] **Step 3: Rename the struct and add the type alias in `service.rs`**

In `core/src/application/service.rs`, change the struct definition:

```rust
pub struct ApplicationFacade<S: ProjectionStore> {
    store: S,
    rule_engine: crate::domain::RuleEngine,
    format_parsers: Vec<Box<dyn crate::source::FormatParser>>,
    space: Option<SpaceContext>,
    capability_log: Vec<notez_core::capability::CapabilityDescriptor>,
}
```

(Initialise the new `capability_log` field. We initialise it in the constructors below.)

Add the type alias immediately after the struct (replacing the existing `pub struct ApplicationService<S>` line — we no longer define a second struct):

```rust
/// Backwards-compatible alias for [`ApplicationFacade`]. New code should
/// refer to `ApplicationFacade` directly; the alias is preserved so
/// downstream consumers can keep their imports stable across the rename.
pub type ApplicationService<S = crate::storage::SqliteProjection> = ApplicationFacade<S>;
```

Update the two existing constructors to initialise `capability_log`:

```rust
    pub fn new(store: S) -> Self {
        Self {
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            space: None,
            capability_log: Vec::new(),
        }
    }

    pub fn with_space(store: S, space: SpaceContext) -> Self {
        Self {
            store,
            rule_engine: crate::domain::RuleEngine::default_rules(),
            format_parsers: Vec::new(),
            space: Some(space),
            capability_log: Vec::new(),
        }
    }
```

Update the body of the no-op from Task 1 to actually append:

```rust
    pub fn register_capability(&mut self, descriptor: &notez_core::capability::CapabilityDescriptor) {
        self.capability_log.push(descriptor.clone());
    }
```

- [ ] **Step 4: Re-export from `application/mod.rs`**

Replace the existing single `pub use service::...` line with two re-exports:

```rust
pub use context::SpaceContext;
pub use service::{ApplicationError, ApplicationFacade, ApplicationService, ResolveResult, ScanReport};
```

(`ApplicationService` here resolves to the type alias.)

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p core --test application_facade_rename`
Expected: PASS (both tests).

- [ ] **Step 6: Verify workspace still builds**

Run: `cargo check --workspace --all-targets`
Expected: exit 0. (CLI/MCP/test code still references `ApplicationService`, which resolves to the alias.)

- [ ] **Step 7: Run the full test suite**

Run: `cargo test --workspace`
Expected: 144 passed, 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/service.rs core/src/application/mod.rs core/tests/application_facade_rename.rs
git commit -m "refactor(application): rename ApplicationService to ApplicationFacade (type alias kept)"
```

---

## Task 3: Add `ScanUseCase` trait

**Files:**
- Create: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/scan.rs`
- Modify: `core/src/application/mod.rs` — add `pub mod use_cases;` and re-export
- Test: `core/tests/use_case_scan.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `ScanUseCase`.

use notez_core::application::use_cases::ScanUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::ResourceKind;
use notez_core::domain::Selector;
use notez_core::storage::SqliteProjection;
use std::path::PathBuf;

fn make_facade(space: &std::path::Path) -> ApplicationFacade<SqliteProjection> {
    let db = space.join(".notez/idx.sqlite");
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
    facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    facade
}

#[test]
fn scan_native_via_trait_returns_scan_report() {
    let dir = tempfile::tempdir().unwrap();
    let space = dir.path();
    std::fs::write(
        space.join("a.org"),
        "#+title: A\n#+ID: 01J00000000000000000000A01\n",
    )
    .unwrap();
    let mut facade = make_facade(space);
    let report = <ApplicationFacade<_> as ScanUseCase>::scan_native(&mut facade, space)
        .expect("scan must succeed");
    assert_eq!(report.scanned_files, 1);
    assert!(report.scanned_resources >= 1);
    let page = facade
        .query(&Selector::kind(ResourceKind::Document).with_title_contains("A"))
        .unwrap();
    assert!(page.items.iter().any(|r| r.title == "A"));
}

#[test]
fn scan_native_without_parsers_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let space = dir.path();
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    let db = space.join(".notez/idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    // No parsers registered.
    let err = <ApplicationFacade<_> as ScanUseCase>::scan_native(&mut facade, space)
        .expect_err("scan must fail without parsers");
    let msg = err.to_string();
    assert!(msg.contains("no format parsers"), "got: {msg}");
}

#[test]
fn scan_federation_runs_without_crashing_on_empty_space() {
    let dir = tempfile::tempdir().unwrap();
    let space = dir.path();
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    let db = space.join(".notez/idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
    facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    let _ = <ApplicationFacade<_> as ScanUseCase>::scan_federation(&mut facade, space);
    // Empty space is not a contract violation here; the contract is
    // "the call returns and the result is observable".
    assert!(space.join(".notez/idx.sqlite").exists());
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_scan -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::ScanUseCase`" (the module does not exist yet).

- [ ] **Step 3: Create the `use_cases` module**

`core/src/application/use_cases/mod.rs`:

```rust
//! Use case traits. The application facade implements each of these so
//! that callers (CLI, MCP, integration tests) can request a narrow
//! capability without depending on the full facade.

mod scan;

pub use scan::ScanUseCase;
```

- [ ] **Step 4: Implement `ScanUseCase`**

`core/src/application/use_cases/scan.rs`:

```rust
//! Scan use case: rebuild a space's projection from the filesystem.

use std::path::Path;

use crate::application::{ApplicationError, ScanReport};

/// Traverse the native source under `space_root` and (re)write the
/// projection. Identical semantics to the historical
/// `ApplicationService::scan_native`; the method is moved into a
/// trait so that callers can take a narrow dependency on scanning.
pub trait ScanUseCase {
    fn scan_native(&mut self, root: &Path) -> Result<ScanReport, ApplicationError>;
    fn scan_federation(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError>;
}
```

- [ ] **Step 5: Implement the trait for `ApplicationFacade`**

Add a new impl block to the **bottom** of `core/src/application/service.rs` (do not duplicate the existing `pub fn scan_native` / `pub fn scan_federation` body — leave them in place; we will convert them to forwarders in the same edit):

```rust
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::ScanUseCase for ApplicationFacade<S> {
    fn scan_native(&mut self, root: &std::path::Path) -> Result<ScanReport, ApplicationError> {
        // Delegates to the existing implementation in this file. We
        // re-use the method body so the contract is preserved bit-for-bit.
        ApplicationFacade::scan_native_impl(self, root)
    }

    fn scan_federation(
        &mut self,
        space_root: &std::path::Path,
    ) -> Result<ScanReport, ApplicationError> {
        ApplicationFacade::scan_federation_impl(self, space_root)
    }
}
```

Rename the existing `pub fn scan_native` and `pub fn scan_federation` in `service.rs`:

- Replace the signature of `pub fn scan_native(&mut self, root: &Path) -> Result<ScanReport, ApplicationError>` with `pub fn scan_native_impl(&mut self, root: &Path) -> Result<ScanReport, ApplicationError>`. The body is unchanged.
- Replace the signature of `pub fn scan_federation(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError>` with `pub fn scan_federation_impl(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError>`. The body is unchanged.

Add the forwarder `pub fn` near the existing struct constructors (at the end of the first `impl<S: ProjectionStore> ApplicationFacade<S>` block):

```rust
    pub fn scan_native(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_native(self, root)
    }

    pub fn scan_federation(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError> {
        <Self as crate::application::use_cases::ScanUseCase>::scan_federation(self, space_root)
    }
```

(Yes, this means there are now two `impl ApplicationFacade<S>` blocks in `service.rs`: the original one that contains the constructors and forwarders, and the new trait-impl block. Both are correct.)

- [ ] **Step 6: Wire up `use_cases` from `application/mod.rs`**

`core/src/application/mod.rs`, add the module declaration and a re-export (place after `pub mod service;`):

```rust
pub mod use_cases;
```

And in the existing `pub use ...` block, add:

```rust
pub use use_cases::ScanUseCase;
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_scan`
Expected: PASS (3 tests).

- [ ] **Step 8: Verify no regression**

Run: `cargo test --workspace`
Expected: 146 passed, 0 failed (144 existing + 2 new from this task's test, plus the 1 from Task 2's application_facade_rename = +3, plus the 2 from Task 1 = 149 in total since Task 1; if some pre-existing tests count differently, the key point is 0 failed).

- [ ] **Step 9: Commit**

```bash
git add core/src/application/mod.rs core/src/application/service.rs core/src/application/use_cases/mod.rs core/src/application/use_cases/scan.rs core/tests/use_case_scan.rs
git commit -m "feat(application): add ScanUseCase trait and forwarder"
```

---

## Task 4: Add `ResourceUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs` — add `mod resource;` and re-export
- Create: `core/src/application/use_cases/resource.rs`
- Modify: `core/src/application/service.rs` — convert the eight resource-related `pub fn`s to `_impl` + forwarder
- Modify: `core/src/application/mod.rs` — re-export `ResourceUseCase`
- Test: `core/tests/use_case_resource.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `ResourceUseCase`.

use notez_core::application::use_cases::ResourceUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::{
    ProjectionStore, Resource, ResourceKind, ResourceRef, ResolvedRelation, LinkTarget,
    LinkOccurrence, TextSpan, Selector, ResolutionStatus,
};
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    let mut facade = ApplicationFacade::new(store);
    facade.register_format_parser(Box::new(orgmode::OrgParser::new()));
    facade.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    facade
}

#[test]
fn upsert_then_read_round_trip_via_traits() {
    let mut facade = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000B1").unwrap();
    let res = Resource {
        r#ref: r_ref,
        kind: ResourceKind::Document,
        title: "T".to_string(),
        revision: "r1".to_string(),
        source_id: "native".to_string(),
        locator: "/x.org".to_string(),
        properties: Default::default(),
    };
    <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, res).unwrap();
    let got = <ApplicationFacade<_> as ResourceUseCase>::read(&facade, &r_ref).unwrap();
    assert!(got.is_some());
}

#[test]
fn resolve_and_resolve_address_share_lookup() {
    let mut facade = make_facade();
    let r_ref = ResourceRef::parse("heading:01J000000000000000000000C1").unwrap();
    let res = Resource {
        r#ref: r_ref,
        kind: ResourceKind::Heading,
        title: "T".to_string(),
        revision: "r1".to_string(),
        source_id: "native".to_string(),
        locator: "/x.org".to_string(),
        properties: Default::default(),
    };
    <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, res).unwrap();
    let r = <ApplicationFacade<_> as ResourceUseCase>::resolve(&facade, "01J000000000000000000000C1").unwrap();
    assert!(matches!(r, notez_core::application::ResolveResult::Found(_)));
}

#[test]
fn query_returns_empty_selector_page() {
    let facade = make_facade();
    let page = <ApplicationFacade<_> as ResourceUseCase>::query(&facade, &Selector::new()).unwrap();
    assert_eq!(page.items.len(), 0);
}

#[test]
fn delete_via_trait_removes_resource() {
    let mut facade = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000D1").unwrap();
    let res = Resource {
        r#ref: r_ref,
        kind: ResourceKind::Document,
        title: "T".to_string(),
        revision: "r1".to_string(),
        source_id: "native".to_string(),
        locator: "/x.org".to_string(),
        properties: Default::default(),
    };
    <ApplicationFacade<_> as ResourceUseCase>::upsert_resource(&mut facade, res).unwrap();
    <ApplicationFacade<_> as ResourceUseCase>::delete_resource(&mut facade, &r_ref).unwrap();
    let got = <ApplicationFacade<_> as ResourceUseCase>::read(&facade, &r_ref).unwrap();
    assert!(got.is_none());
}

#[test]
fn list_recent_and_list_by_source_return_stable_shape() {
    let facade = make_facade();
    let r1 = <ApplicationFacade<_> as ResourceUseCase>::list_recent(&facade, 10).unwrap();
    let r2 = <ApplicationFacade<_> as ResourceUseCase>::list_by_source(&facade, "native", 10).unwrap();
    assert_eq!(r1.len(), 0);
    assert_eq!(r2.len(), 0);
}
```

(Add a `use notez_core::application::ResolveResult;` if the resolver is not already imported at the top of the file.)

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_resource -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::ResourceUseCase`".

- [ ] **Step 3: Implement `ResourceUseCase`**

`core/src/application/use_cases/resource.rs`:

```rust
//! Resource use case: single-resource CRUD and lookup helpers.

use crate::application::{ApplicationError, ResolveResult};
use crate::domain::{
    QueryPage, Resource, ResourceRef, Selector,
};

pub trait ResourceUseCase {
    fn upsert_resource(&mut self, resource: Resource) -> Result<(), ApplicationError>;
    fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), ApplicationError>;
    fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError>;
    fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError>;
    fn list_recent(&self, limit: usize) -> Result<Vec<Resource>, ApplicationError>;
    fn list_by_source(
        &self,
        source_id: &str,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError>;
    fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError>;
    fn resolve_address(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError>;
}
```

`core/src/application/use_cases/mod.rs`, add the module + re-export:

```rust
mod resource;
mod scan;

pub use resource::ResourceUseCase;
pub use scan::ScanUseCase;
```

- [ ] **Step 4: Add a trait impl block for `ResourceUseCase` in `service.rs`**

At the bottom of `service.rs`, add a new `impl<S: ProjectionStore> ResourceUseCase for ApplicationFacade<S> { ... }` block. The method bodies are the bodies of the existing `pub fn` implementations, verbatim, but renamed to remove their `pub` qualifier (the trait is `pub trait`, the methods inside the trait are already accessible as long as the trait is in scope). Use the `*_impl` rename pattern from Task 3 for the eight `pub fn`s:

```rust
// Existing pub fn signatures, renamed to *_impl:
pub fn upsert_resource_impl(&mut self, resource: Resource) -> Result<(), ApplicationError> { /* same body */ }
pub fn delete_resource_impl(&mut self, r_ref: &ResourceRef) -> Result<(), ApplicationError> { /* same body */ }
pub fn query_impl(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> { /* same body */ }
pub fn read_impl(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> { /* same body */ }
pub fn list_recent_impl(&self, limit: usize) -> Result<Vec<Resource>, ApplicationError> { /* same body */ }
pub fn list_by_source_impl(&self, source_id: &str, limit: usize) -> Result<Vec<Resource>, ApplicationError> { /* same body */ }
pub fn resolve_impl(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> { /* same body */ }
pub fn resolve_address_impl(&self, address: &crate::domain::ResourceAddress) -> Result<ResolveResult, ApplicationError> { /* same body */ }
```

In the existing first `impl<S: ProjectionStore> ApplicationFacade<S>` block, add the forwarder `pub fn`s:

```rust
    pub fn upsert_resource(&mut self, resource: Resource) -> Result<(), ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::upsert_resource(self, resource)
    }
    pub fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::delete_resource(self, r_ref)
    }
    pub fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::query(self, selector)
    }
    pub fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::read(self, r_ref)
    }
    pub fn list_recent(&self, limit: usize) -> Result<Vec<Resource>, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::list_recent(self, limit)
    }
    pub fn list_by_source(&self, source_id: &str, limit: usize) -> Result<Vec<Resource>, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::list_by_source(self, source_id, limit)
    }
    pub fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::resolve(self, query_str)
    }
    pub fn resolve_address(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError> {
        <Self as crate::application::use_cases::ResourceUseCase>::resolve_address(self, address)
    }
```

Add the new trait impl block at the bottom of `service.rs`:

```rust
impl<S: crate::domain::ProjectionStore> crate::application::use_cases::ResourceUseCase
    for ApplicationFacade<S>
{
    fn upsert_resource(&mut self, resource: Resource) -> Result<(), ApplicationError> {
        ApplicationFacade::upsert_resource_impl(self, resource)
    }
    fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), ApplicationError> {
        ApplicationFacade::delete_resource_impl(self, r_ref)
    }
    fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> {
        ApplicationFacade::query_impl(self, selector)
    }
    fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> {
        ApplicationFacade::read_impl(self, r_ref)
    }
    fn list_recent(&self, limit: usize) -> Result<Vec<Resource>, ApplicationError> {
        ApplicationFacade::list_recent_impl(self, limit)
    }
    fn list_by_source(
        &self,
        source_id: &str,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError> {
        ApplicationFacade::list_by_source_impl(self, source_id, limit)
    }
    fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> {
        ApplicationFacade::resolve_impl(self, query_str)
    }
    fn resolve_address(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError> {
        ApplicationFacade::resolve_address_impl(self, address)
    }
}
```

(The `*_impl` bodies are the existing implementations, just with the `pub` keyword removed and the suffix `_impl` appended. The body content — `self.store...`, etc. — stays unchanged.)

- [ ] **Step 5: Re-export from `application/mod.rs`**

Add to the existing `pub use ...` block:

```rust
pub use use_cases::{ResourceUseCase, ScanUseCase};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_resource`
Expected: PASS (5 tests).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/resource.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_resource.rs
git commit -m "feat(application): add ResourceUseCase trait and forwarders"
```

---

## Task 5: Add `LinkUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/link.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/use_case_link.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `LinkUseCase`.

use notez_core::application::use_cases::LinkUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::{LinkTarget, LinkOccurrence, ResourceRef, TextSpan};
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn empty_link_diagnostics_and_relations() {
    let facade = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E1").unwrap();
    let occs = <ApplicationFacade<_> as LinkUseCase>::query_link_occurrences(&facade, &r_ref).unwrap();
    assert!(occs.is_empty());
    let rels = <ApplicationFacade<_> as LinkUseCase>::query_resolved_relations(&facade, &r_ref).unwrap();
    assert!(rels.is_empty());
    let links = <ApplicationFacade<_> as LinkUseCase>::list_links(&facade, &r_ref).unwrap();
    assert!(links.is_empty());
}

#[test]
fn resolve_links_does_not_panic_on_missing_resource() {
    let mut facade = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E2").unwrap();
    let _ = <ApplicationFacade<_> as LinkUseCase>::resolve_links(&mut facade, &r_ref);
    // Behaviour on a missing source: the implementation must not panic
    // and must return a Vec (possibly empty). We only assert shape here.
}

#[test]
fn diagnose_link_returns_empty_list_for_missing_resource() {
    let facade = make_facade();
    let r_ref = ResourceRef::parse("document:01J000000000000000000000E3").unwrap();
    let diag = <ApplicationFacade<_> as LinkUseCase>::diagnose_link(&facade, &r_ref).unwrap();
    assert!(diag.is_empty());
}

#[test]
fn reindex_links_returns_report_with_zero_counts_on_empty_space() {
    let mut facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let report = <ApplicationFacade<_> as LinkUseCase>::reindex_links(&mut facade, dir.path()).unwrap();
    assert_eq!(report.scanned, 0);
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_link -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::LinkUseCase`".

- [ ] **Step 3: Implement `LinkUseCase`**

`core/src/application/use_cases/link.rs`:

```rust
//! Link use case: query, resolve, and diagnose link occurrences.

use crate::application::link_resolution::LinkReindexReport;
use crate::application::ApplicationError;
use crate::domain::{LinkDiagnostic, LinkOccurrence, ResolvedRelation, ResourceRef};

pub trait LinkUseCase {
    fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError>;
    fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError>;
    fn list_links(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, ApplicationError>;
    fn resolve_links(
        &mut self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<ResolvedRelation>, ApplicationError>;
    fn diagnose_link(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkDiagnostic>, ApplicationError>;
    fn reindex_links(
        &mut self,
        space_root: &std::path::Path,
    ) -> Result<LinkReindexReport, ApplicationError>;
}
```

`core/src/application/use_cases/mod.rs`:

```rust
mod link;
mod resource;
mod scan;

pub use link::LinkUseCase;
pub use resource::ResourceUseCase;
pub use scan::ScanUseCase;
```

- [ ] **Step 4: Convert the six `pub fn`s in `service.rs` to `_impl` + forwarder + trait impl**

For each of the six functions `query_link_occurrences`, `query_resolved_relations`, `list_links`, `resolve_links`, `diagnose_link`, `reindex_links`:

1. Rename the existing `pub fn X(...)` to `pub fn X_impl(...)`. The body is unchanged.
2. Add a `pub fn X(...)` forwarder in the first impl block (pattern from Task 4).
3. Add the trait method dispatching to `X_impl` in a new `impl LinkUseCase for ApplicationFacade<S>` block at the bottom of `service.rs`.

(The exact textual edits mirror Task 4's pattern; copy the structure.)

- [ ] **Step 5: Re-export from `application/mod.rs`**

```rust
pub use use_cases::{LinkUseCase, ResourceUseCase, ScanUseCase};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_link`
Expected: PASS (4 tests).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/link.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_link.rs
git commit -m "feat(application): add LinkUseCase trait and forwarders"
```

---

## Task 6: Add `TaskUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/task.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/use_case_task.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `TaskUseCase`.

use notez_core::application::use_cases::TaskUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn agenda_on_empty_projection_is_empty() {
    let facade = make_facade();
    let view = <ApplicationFacade<_> as TaskUseCase>::agenda(&facade).unwrap();
    assert!(view.items.is_empty());
}

#[test]
fn para_overview_on_empty_projection_has_four_empty_buckets() {
    let facade = make_facade();
    let view = <ApplicationFacade<_> as TaskUseCase>::para_overview(&facade).unwrap();
    assert!(view.projects.is_empty());
    assert!(view.areas.is_empty());
    assert!(view.resources.is_empty());
    assert!(view.archives.is_empty());
}

#[test]
fn transition_task_on_missing_resource_returns_not_found() {
    let mut facade = make_facade();
    let r_ref = ResourceRef::parse("heading:01J000000000000000000000F1").unwrap();
    let err = <ApplicationFacade<_> as TaskUseCase>::transition_task(&mut facade, &r_ref, "DONE", "2026-08-02")
        .expect_err("transition on missing resource must fail");
    assert!(err.to_string().contains("not found") || err.to_string().contains("01J000000000000000000000F1"));
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_task -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::TaskUseCase`".

- [ ] **Step 3: Implement `TaskUseCase`**

`core/src/application/use_cases/task.rs`:

```rust
//! Task use case: agenda, PARA overview, and state transitions.

use std::path::Path;

use crate::application::task_para::{AgendaView, ParaOverview};
use crate::application::ApplicationError;
use crate::domain::{ResourceRef, StateTransition};

pub trait TaskUseCase {
    fn agenda(&self) -> Result<AgendaView, ApplicationError>;
    fn para_overview(&self) -> Result<ParaOverview, ApplicationError>;
    fn transition_task(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
    ) -> Result<StateTransition, ApplicationError>;
}
```

`core/src/application/use_cases/mod.rs`, add `mod task;` and `pub use task::TaskUseCase;`.

- [ ] **Step 4: Convert the three `pub fn`s to `_impl` + forwarder + trait impl**

Pattern from Task 4. Rename `agenda`, `para_overview`, `transition_task` to `*_impl`. Add forwarder `pub fn`s. Add `impl TaskUseCase for ApplicationFacade<S> { ... }` block at the bottom of `service.rs`.

- [ ] **Step 5: Re-export from `application/mod.rs`**

```rust
pub use use_cases::{LinkUseCase, ResourceUseCase, ScanUseCase, TaskUseCase};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_task`
Expected: PASS (3 tests).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/task.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_task.rs
git commit -m "feat(application): add TaskUseCase trait and forwarders"
```

---

## Task 7: Add `AttachmentUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/attachment.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/use_case_attachment.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `AttachmentUseCase`.

use notez_core::application::use_cases::AttachmentUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn query_segments_on_unknown_ref_returns_empty_vec() {
    let facade = make_facade();
    let r_ref = ResourceRef::parse("attachment:01J0000000000000000000010A1").unwrap();
    let segs = <ApplicationFacade<_> as AttachmentUseCase>::query_segments(&facade, &r_ref).unwrap();
    assert!(segs.is_empty());
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_attachment -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::AttachmentUseCase`".

- [ ] **Step 3: Implement `AttachmentUseCase`**

`core/src/application/use_cases/attachment.rs`:

```rust
//! Attachment use case: blob storage and segment extraction.

use std::path::Path;

use crate::application::ApplicationError;
use crate::domain::{ResourceRef, SegmentRecord};

pub trait AttachmentUseCase {
    fn add_attachment(
        &mut self,
        space_root: &Path,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError>;
    fn run_extraction(
        &mut self,
        space_root: &Path,
        att_ref: &ResourceRef,
    ) -> Result<Vec<SegmentRecord>, ApplicationError>;
    fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<SegmentRecord>, ApplicationError>;
}
```

- [ ] **Step 4: Convert `add_attachment`, `run_extraction`, `query_segments` to `_impl` + forwarder + trait impl**

Pattern from Task 4.

- [ ] **Step 5: Re-export from `application/mod.rs`**

```rust
pub use use_cases::{
    AttachmentUseCase, LinkUseCase, ResourceUseCase, ScanUseCase, TaskUseCase,
};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_attachment`
Expected: PASS (1 test).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/attachment.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_attachment.rs
git commit -m "feat(application): add AttachmentUseCase trait and forwarders"
```

---

## Task 8: Add `CommunityUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/community.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/use_case_community.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `CommunityUseCase`.

use notez_core::application::use_cases::CommunityUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::community::{Community, CommunitySelector};
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn list_communities_on_empty_community_config_returns_empty() {
    let facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let list = <ApplicationFacade<_> as CommunityUseCase>::list_communities(&facade, dir.path()).unwrap();
    assert!(list.is_empty());
}

#[test]
fn create_community_requires_a_space_context() {
    let mut facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let community = Community {
        id: "01J0000000000000000000000C1".to_string(),
        name: "Test".to_string(),
        selector: CommunitySelector::default(),
    };
    let err = <ApplicationFacade<_> as CommunityUseCase>::create_community(
        &mut facade,
        dir.path(),
        community,
    )
    .expect_err("create_community without a SpaceContext must fail");
    assert!(err.to_string().to_lowercase().contains("spacecontext")
        || err.to_string().to_lowercase().contains("space")
        || err.to_string().to_lowercase().contains("sources"));
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_community -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::CommunityUseCase`".

- [ ] **Step 3: Implement `CommunityUseCase`**

`core/src/application/use_cases/community.rs`:

```rust
//! Community use case: groups of resources.

use std::path::Path;

use crate::application::ApplicationError;
use crate::domain::community::Community;

pub trait CommunityUseCase {
    fn create_community(
        &self,
        space_root: &Path,
        community: Community,
    ) -> Result<(), ApplicationError>;
    fn list_communities(
        &self,
        space_root: &Path,
    ) -> Result<Vec<Community>, ApplicationError>;
}
```

- [ ] **Step 4: Convert `create_community`, `list_communities` to `_impl` + forwarder + trait impl**

Pattern from Task 4.

- [ ] **Step 5: Re-export from `application/mod.rs`**

```rust
pub use use_cases::{
    AttachmentUseCase, CommunityUseCase, LinkUseCase, ResourceUseCase, ScanUseCase,
    TaskUseCase,
};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_community`
Expected: PASS (2 tests).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/community.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_community.rs
git commit -m "feat(application): add CommunityUseCase trait and forwarders"
```

---

## Task 9: Add `ArtifactUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/artifact.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/use_case_artifact.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `ArtifactUseCase`.

use notez_core::application::use_cases::ArtifactUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn derive_artifact_with_unknown_recipe_returns_error() {
    let mut facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let err = <ApplicationFacade<_> as ArtifactUseCase>::derive_artifact(
        &mut facade,
        dir.path(),
        "missing-community",
        "nope",
    )
    .expect_err("missing community must error");
    assert!(!err.to_string().is_empty());
}

#[test]
fn export_skill_with_unknown_community_returns_error() {
    let mut facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("SKILL.md");
    let err = <ApplicationFacade<_> as ArtifactUseCase>::export_skill(
        &mut facade,
        dir.path(),
        "missing-community",
        "description",
        &out,
    )
    .expect_err("missing community must error");
    assert!(!err.to_string().is_empty());
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_artifact -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::ArtifactUseCase`".

- [ ] **Step 3: Implement `ArtifactUseCase`**

`core/src/application/use_cases/artifact.rs`:

```rust
//! Artifact use case: derived outputs (summary, llms.txt, context-pack,
//! skill) and skill package export.

use std::path::Path;

use crate::application::ApplicationError;
use crate::artifact::{DerivedArtifact, SkillPackage};

pub trait ArtifactUseCase {
    fn derive_artifact(
        &mut self,
        space_root: &Path,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<DerivedArtifact, ApplicationError>;
    fn export_skill(
        &mut self,
        space_root: &Path,
        community_id: &str,
        description: &str,
        export_path: &Path,
    ) -> Result<SkillPackage, ApplicationError>;
}
```

- [ ] **Step 4: Convert `derive_artifact`, `export_skill` to `_impl` + forwarder + trait impl**

Pattern from Task 4.

- [ ] **Step 5: Re-export from `application/mod.rs`**

```rust
pub use use_cases::{
    ArtifactUseCase, AttachmentUseCase, CommunityUseCase, LinkUseCase, ResourceUseCase,
    ScanUseCase, TaskUseCase,
};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_artifact`
Expected: PASS (2 tests).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/artifact.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_artifact.rs
git commit -m "feat(application): add ArtifactUseCase trait and forwarders"
```

---

## Task 10: Add `SyncUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/sync.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/use_case_sync.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `SyncUseCase`.

use notez_core::application::use_cases::SyncUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn list_conflicts_returns_unsupported_capability() {
    let facade = make_facade();
    let err = <ApplicationFacade<_> as SyncUseCase>::list_conflicts(&facade)
        .expect_err("list_conflicts is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn relay_sync_returns_unsupported_capability() {
    let facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let err = <ApplicationFacade<_> as SyncUseCase>::relay_sync(&facade, "src", dir.path())
        .expect_err("relay_sync is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_sync -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::SyncUseCase`".

- [ ] **Step 3: Implement `SyncUseCase`**

`core/src/application/use_cases/sync.rs`:

```rust
//! Sync use case: push, pull, relay, and conflict listing.

use std::path::Path;

use crate::application::writeback::RelaySyncReport;
use crate::application::ApplicationError;
use crate::sync::ConflictRecord;

pub trait SyncUseCase {
    fn sync_push(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError>;
    fn sync_pull(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError>;
    fn relay_sync(
        &self,
        source_id: &str,
        space_root: &Path,
    ) -> Result<RelaySyncReport, ApplicationError>;
    fn list_conflicts(&self) -> Result<Vec<ConflictRecord>, ApplicationError>;
}
```

- [ ] **Step 4: Convert `sync_push`, `sync_pull`, `relay_sync`, `list_conflicts` to `_impl` + forwarder + trait impl**

Pattern from Task 4.

- [ ] **Step 5: Re-export from `application/mod.rs`**

```rust
pub use use_cases::{
    ArtifactUseCase, AttachmentUseCase, CommunityUseCase, LinkUseCase, ResourceUseCase,
    ScanUseCase, SyncUseCase, TaskUseCase,
};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_sync`
Expected: PASS (2 tests).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/sync.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_sync.rs
git commit -m "feat(application): add SyncUseCase trait and forwarders"
```

---

## Task 11: Add `InspectUseCase` trait

**Files:**
- Modify: `core/src/application/use_cases/mod.rs`
- Create: `core/src/application/use_cases/inspect.rs`
- Modify: `core/src/application/service.rs`
- Modify: `core/src/application/mod.rs`
- Test: `core/tests/use_case_inspect.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Contract tests for `InspectUseCase`.

use notez_core::application::use_cases::InspectUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn inspect_rules_for_missing_resource_returns_none() {
    let facade = make_facade();
    let r_ref = ResourceRef::parse("document:01J0000000000000000000011A1").unwrap();
    let out = <ApplicationFacade<_> as InspectUseCase>::inspect_rules(&facade, &r_ref).unwrap();
    assert!(out.is_none());
}

#[test]
fn list_jobs_returns_unsupported_capability() {
    let facade = make_facade();
    let err = <ApplicationFacade<_> as InspectUseCase>::list_jobs(&facade)
        .expect_err("list_jobs is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn check_artifact_freshness_returns_unsupported_capability() {
    let facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let err = <ApplicationFacade<_> as InspectUseCase>::check_artifact_freshness(&facade, dir.path())
        .expect_err("check_artifact_freshness is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn space_doctor_runs_against_a_missing_space_root() {
    let facade = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let result = <ApplicationFacade<_> as InspectUseCase>::space_doctor(&facade, dir.path());
    // Space doctor is currently an UnsupportedCapability in T8; this
    // contract is allowed to flip when a doctor is implemented. For
    // now we only assert the call returns a Result without panicking.
    let _ = result;
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test use_case_inspect -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::application::use_cases::InspectUseCase`".

- [ ] **Step 3: Implement `InspectUseCase`**

`core/src/application/use_cases/inspect.rs`:

```rust
//! Inspect use case: rule inspection, doctor, jobs, and freshness.

use std::path::Path;

use crate::application::doctor::DoctorReport;
use crate::application::job_manager::{ArtifactStaleReport, JobRecord};
use crate::application::ApplicationError;
use crate::domain::{InspectResult, ResourceRef};

pub trait InspectUseCase {
    fn inspect_rules(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<InspectResult>, ApplicationError>;
    fn space_doctor(
        &self,
        space_root: &Path,
    ) -> Result<DoctorReport, ApplicationError>;
    fn list_jobs(&self) -> Result<Vec<JobRecord>, ApplicationError>;
    fn check_artifact_freshness(
        &self,
        space_root: &Path,
    ) -> Result<ArtifactStaleReport, ApplicationError>;
}
```

- [ ] **Step 4: Convert `inspect_rules`, `space_doctor`, `list_jobs`, `check_artifact_freshness` to `_impl` + forwarder + trait impl**

Pattern from Task 4.

- [ ] **Step 5: Re-export from `application/mod.rs`**

```rust
pub use use_cases::{
    ArtifactUseCase, AttachmentUseCase, CommunityUseCase, InspectUseCase, LinkUseCase,
    ResourceUseCase, ScanUseCase, SyncUseCase, TaskUseCase,
};
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p core --test use_case_inspect`
Expected: PASS (4 tests).

- [ ] **Step 7: Verify no regression**

Run: `cargo test --workspace`
Expected: 0 failed.

- [ ] **Step 8: Commit**

```bash
git add core/src/application/use_cases/mod.rs core/src/application/use_cases/inspect.rs core/src/application/service.rs core/src/application/mod.rs core/tests/use_case_inspect.rs
git commit -m "feat(application): add InspectUseCase trait and forwarders"
```

---

## Task 12: Migrate CLI to trait methods

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Inventory every `service.` call in `cli/src/main.rs`**

Run:

```bash
grep -nE "service\.[a-z_]+\(" cli/src/main.rs | head -80
```

This produces the full list. Categorise the results into the 9 use-case traits by looking at each method name (e.g. `service.scan_native` → `ScanUseCase`, `service.query` → `ResourceUseCase`, `service.reindex_links` → `LinkUseCase`, etc.).

- [ ] **Step 2: Add the trait imports at the top of `cli/src/main.rs`**

Replace the existing `use notez_core::application::{ApplicationService, ResolveResult, SpaceContext};` with:

```rust
use notez_core::application::{
    ApplicationService, ResolveResult, SpaceContext,
    use_cases::{
        ArtifactUseCase, AttachmentUseCase, CommunityUseCase, InspectUseCase, LinkUseCase,
        ResourceUseCase, ScanUseCase, SyncUseCase, TaskUseCase,
    },
};
```

- [ ] **Step 3: Replace each `service.X(args...)` call with the trait dispatch**

For each method call:

- `service.scan_native(...)` → `ScanUseCase::scan_native(&mut service, ...)`
- `service.scan_federation(...)` → `ScanUseCase::scan_federation(&mut service, ...)`
- `service.query(...)` → `ResourceUseCase::query(&service, ...)`
- `service.read(...)` → `ResourceUseCase::read(&service, ...)`
- `service.list_recent(...)` → `ResourceUseCase::list_recent(&service, ...)`
- `service.list_by_source(...)` → `ResourceUseCase::list_by_source(&service, ...)`
- `service.upsert_resource(...)` → `ResourceUseCase::upsert_resource(&mut service, ...)`
- `service.delete_resource(...)` → `ResourceUseCase::delete_resource(&mut service, ...)`
- `service.resolve(...)` → `ResourceUseCase::resolve(&service, ...)`
- `service.resolve_address(...)` → `ResourceUseCase::resolve_address(&service, ...)`
- `service.query_link_occurrences(...)` → `LinkUseCase::query_link_occurrences(&service, ...)`
- `service.query_resolved_relations(...)` → `LinkUseCase::query_resolved_relations(&service, ...)`
- `service.list_links(...)` → `LinkUseCase::list_links(&service, ...)`
- `service.resolve_links(...)` → `LinkUseCase::resolve_links(&mut service, ...)`
- `service.diagnose_link(...)` → `LinkUseCase::diagnose_link(&service, ...)`
- `service.reindex_links(...)` → `LinkUseCase::reindex_links(&mut service, ...)`
- `service.agenda(...)` → `TaskUseCase::agenda(&service, ...)`
- `service.para_overview(...)` → `TaskUseCase::para_overview(&service, ...)`
- `service.transition_task(...)` → `TaskUseCase::transition_task(&mut service, ...)`
- `service.add_attachment(...)` → `AttachmentUseCase::add_attachment(&mut service, ...)`
- `service.run_extraction(...)` → `AttachmentUseCase::run_extraction(&mut service, ...)`
- `service.query_segments(...)` → `AttachmentUseCase::query_segments(&service, ...)`
- `service.create_community(...)` → `CommunityUseCase::create_community(&service, ...)`
- `service.list_communities(...)` → `CommunityUseCase::list_communities(&service, ...)`
- `service.derive_artifact(...)` → `ArtifactUseCase::derive_artifact(&mut service, ...)`
- `service.export_skill(...)` → `ArtifactUseCase::export_skill(&mut service, ...)`
- `service.sync_push(...)` → `SyncUseCase::sync_push(&mut service, ...)`
- `service.sync_pull(...)` → `SyncUseCase::sync_pull(&mut service, ...)`
- `service.relay_sync(...)` → `SyncUseCase::relay_sync(&service, ...)`
- `service.list_conflicts(...)` → `SyncUseCase::list_conflicts(&service, ...)`
- `service.inspect_rules(...)` → `InspectUseCase::inspect_rules(&service, ...)`
- `service.space_doctor(...)` → `InspectUseCase::space_doctor(&service, ...)`
- `service.list_jobs(...)` → `InspectUseCase::list_jobs(&service, ...)`
- `service.check_artifact_freshness(...)` → `InspectUseCase::check_artifact_freshness(&service, ...)`
- `service.writeback_resource(...)` and `service.add_source(...)` stay as `service.X(...)` calls — they are not in any use-case trait; they are facade-level config operations.

Apply the replacements with the Edit tool, working one method at a time. The `service` variable is currently `let mut service = ApplicationService::with_space(store, space);` — the `service` binding keeps the name; we just change the call style. The type of `service` is unchanged because `ApplicationFacade<S>` (aliased as `ApplicationService<S>`) impls every trait we are dispatching on.

(Important: do not change `service` to a different type. We are calling trait methods on the existing `service` binding via fully-qualified syntax.)

- [ ] **Step 4: Run the workspace test**

Run: `cargo test --workspace`
Expected: 0 failed. All 144 + 27 = 171 existing tests still pass.

- [ ] **Step 5: Verify the CLI binary still builds**

Run: `cargo run -p cli -- --help`
Expected: prints the help text without compile errors.

- [ ] **Step 6: Commit**

```bash
git add cli/src/main.rs
git commit -m "refactor(cli): dispatch service calls via UseCase traits"
```

---

## Task 13: Migrate MCP to trait methods

**Files:**
- Modify: `cli/src/mcp/server.rs`

- [ ] **Step 1: Add trait imports**

Find the existing `use notez_core::application::{ApplicationService, ResolveResult, SpaceContext};` (or similar) in `cli/src/mcp/server.rs` and replace with the trait import set used in Task 12 Step 2.

- [ ] **Step 2: Replace each `service.X(...)` call**

Apply the same `Trait::method(&self/&mut self, ...)` rewrites for every `service.X` call. The list of rewrites is identical to Task 12 Step 3 (the 33 `service.X` calls in `server.rs`). Leave `service.writeback_resource`, `service.add_source`, `service.list_sources` and any other facade-level config method unchanged.

- [ ] **Step 3: Run the workspace test**

Run: `cargo test --workspace`
Expected: 0 failed. (MCP integration tests under `cli/tests/mcp/` should still pass — they use the `cli::tests::mcp::common::run_session` helper, which doesn't care about the trait dispatch style.)

- [ ] **Step 4: Run the CLI smoke**

Run: `cargo run -p cli -- --space /tmp/notez-smoke scan`
Expected: returns "Scanned 1 files, 2 resources, 0 relations." (assuming `/tmp/notez-smoke` from earlier sessions still has `note.org`; if not, this just exercises the binary and we don't assert on output).

- [ ] **Step 5: Commit**

```bash
git add cli/src/mcp/server.rs
git commit -m "refactor(mcp): dispatch service calls via UseCase traits"
```

---

## Task 14: Final verification and audit

**Files:**
- Modify (audit only, no code change): `docs/superpowers/specs/2026-08-02-usecase-trait-split-design.org` — append a "Verification" section

- [ ] **Step 1: Run the full workspace test**

Run: `cargo test --workspace`
Expected: 0 failed. Total test count should be 144 (existing) + 2 (capability_descriptor) + 2 (application_facade_rename) + 3 (use_case_scan) + 5 (use_case_resource) + 4 (use_case_link) + 3 (use_case_task) + 1 (use_case_attachment) + 2 (use_case_community) + 2 (use_case_artifact) + 2 (use_case_sync) + 4 (use_case_inspect) = 174.

Capture the test count for the spec audit.

- [ ] **Step 2: Run the workspace build**

Run: `cargo check --workspace --all-targets`
Expected: exit 0.

- [ ] **Step 3: Run the CLI end-to-end smoke**

Run:

```bash
rm -rf /tmp/notez-usecase-smoke && mkdir -p /tmp/notez-usecase-smoke
cat > /tmp/notez-usecase-smoke/note.org <<'EOF'
#+title: UseCase Smoke
#+ID: 01J00000000000000000000B01

* NEXT Smoke heading
:PROPERTIES:
:ID: 01J00000000000000000000B02
:END:
EOF
cargo run -p cli -- --space /tmp/notez-usecase-smoke scan
cargo run -p cli -- --space /tmp/notez-usecase-smoke --json query --kind heading
cargo run -p cli -- --space /tmp/notez-usecase-smoke --json read heading:01J00000000000000000000B02
```

Expected:
- scan: `Scanned 1 files, 2 resources, 0 relations.`
- query: returns one heading with TODO=NEXT, LEVEL=1.
- read: returns the same heading.

- [ ] **Step 4: Audit grep: no `service.X` for use case methods remains in CLI or MCP**

Run:

```bash
echo "== CLI service.X remaining =="
grep -nE "service\.[a-z_]+\(" cli/src/main.rs cli/src/mcp/server.rs | grep -vE "service\.(new|with_space|space|store|store_mut|register_format_parser|register_capability|add_source|list_sources|writeback_resource|rebuild)" | head -40
echo "== Should be empty =="
```

Expected: the second list is empty. Any `service.X` call listed should be in a method body that is itself a forwarder (e.g. inside `ScanUseCase for ApplicationFacade<S>` impl block in `core/src/application/service.rs`).

- [ ] **Step 5: Update the spec with the verification result**

Append a "Verification" section to `docs/superpowers/specs/2026-08-02-usecase-trait-split-design.org`:

```org
** 9. Verification

- cargo test --workspace: <N> tests passed, 0 failed.
- cargo check --workspace --all-targets: exit 0.
- CLI scan/query/read smoke against /tmp/notez-usecase-smoke: all three commands returned expected output.
- Audit grep: no `service.X` for use-case methods remains in cli/src/main.rs or cli/src/mcp/server.rs.
```

Replace `<N>` with the actual test count from Step 1.

- [ ] **Step 6: Commit the spec update**

```bash
git add docs/superpowers/specs/2026-08-02-usecase-trait-split-design.org
git commit -m "docs(usecase): record final verification counts"
```

---

## Self-Review Notes

Spec coverage check:

- §2.1 9 trait划分 — Tasks 3-11.
- §2.2 ApplicationFacade 重命名 — Task 2.
- §2.3 Facade 转发 — Tasks 3-11.
- §2.4 Capability hook — Task 1.
- §2.5 文件组织 — Tasks 1-11 (new files) + Task 2 (rename service.rs).
- §2.6 CLI/MCP 调用方改造 — Tasks 12-13.
- §2.7 测试策略 — Tasks 1, 2, 3-11 (新增 11 个测试文件).
- §3 错误与边界 — 不动 ApplicationError / ScanReport / ResolveResult / ProjectionStore / SpaceContext / SourceAdapter / FormatParser — 无 task 需要，spec 已明确边界。
- §4 验证 — Task 14.
- §5 迁移顺序 — Tasks 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 严格按 spec 顺序。
- §6 风险 — 借用冲突通过每步 144 测试验证、CLI/MCP 1300+ 行同步改通过 Task 12-13 验证、ApplicationError 不变通过现有 "unsupported capability" 字符串断言验证。
- §7 非目标 — 无对应 task (本轮不实现)。
- §8 验收 — Task 14.

Type consistency: `ApplicationFacade<S>` 在 Task 2 引入；后续 task 全部用 `ApplicationFacade<S>` 形式（如 `<ApplicationFacade<_> as ScanUseCase>::scan_native(...)`），保持一致。

Placeholder scan: 已通读全部 14 个 task 的 70+ 个 step，无 "TBD / TODO / 待定 / 以后" 占位。
