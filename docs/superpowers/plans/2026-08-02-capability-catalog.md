# CapabilityCatalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote `ApplicationFacade::register_capability` from a no-op hook to a queryable `CapabilityCatalog`. 9 `UseCase` traits each map to one descriptor (id, description, mutability). CLI gains a `--list-capabilities` flag for read-side visibility. All 183 existing tests must remain green.

**Architecture:** A `CapabilityCatalog` (HashMap keyed by `&'static str`) lives in `core::capability` next to the existing `CapabilityDescriptor` and `Mutability`. `with_builtins` populates 9 descriptors. `ApplicationFacade` swaps its `capability_log: Vec` for `capability_catalog: CapabilityCatalog`, initialised via `with_builtins()`. `register_capability` delegates to `catalog.register(...)`. CLI main gains a `--list-capabilities` flag that prints the catalog as JSON and exits 0 before any space/work setup runs.

**Tech Stack:** Rust 2024, Cargo workspace, existing `clap` for CLI, existing `serde_json` for the catalog dump, existing `ApplicationFacade` with all 9 use-case traits.

---

## File Structure

New files (1):

- `core/tests/capability_catalog.rs` — 6 contract tests: `new` is empty, `with_builtins` registers 9, `register` overwrites, `get` hit/miss, `list` returns all 9, `iter` yields key-value pairs, plus a test asserting `ApplicationFacade::capability_catalog()` mirrors `with_builtins()` on construction.

Modified files (3):

- `core/src/capability.rs` — add `CapabilityCatalog` struct + 6 public methods + `with_builtins()` registering 9 descriptors.
- `core/src/application/service.rs` — replace `capability_log: Vec<CapabilityDescriptor>` with `capability_catalog: CapabilityCatalog`; update `new` and `with_space` constructors to call `CapabilityCatalog::with_builtins()`; change `register_capability` body to delegate to the catalog; add a public `capability_catalog()` getter.
- `cli/src/main.rs` — add a `--list-capabilities` top-level flag that prints the catalog as a JSON array and exits 0 before any other wiring runs.

No changes to MCP or to any of the 9 use case traits or their implementations.

---

## Conventions

- `CapabilityCatalog` is a public type in `core::capability`.
- The 9 `CapabilityDescriptor::id` values are `&'static str` literal strings: `"scan"`, `"resource"`, `"link"`, `"task"`, `"attachment"`, `"community"`, `"artifact"`, `"sync"`, `"inspect"`.
- All 9 descriptors are hard-coded inside `CapabilityCatalog::with_builtins`; there is no automatic reflection from the use case traits.
- `CapabilityCatalog::list()` returns a `Vec<&'static CapabilityDescriptor>` in arbitrary order; tests must use length-based + contains-based assertions, not order-based.
- `ApplicationFacade::capability_catalog()` returns `&CapabilityCatalog` (a borrowed view); callers must not store the reference past the facade's lifetime.

---

## Task 1: Add `CapabilityCatalog` to `core::capability`

**Files:**
- Modify: `core/src/capability.rs`
- Test: `core/tests/capability_catalog.rs`

- [ ] **Step 1: Write the failing contract test**

Create `core/tests/capability_catalog.rs` with the following test bodies:

```rust
//! Contract tests for `CapabilityCatalog`.

use notez_core::capability::{CapabilityCatalog, CapabilityDescriptor, Mutability};
use notez_core::application::ApplicationFacade;
use notez_core::storage::SqliteProjection;
use std::collections::HashSet;

fn make_facade() -> ApplicationFacade<SqliteProjection> {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    ApplicationFacade::new(store)
}

#[test]
fn new_catalog_is_empty() {
    let cat = CapabilityCatalog::new();
    assert!(cat.list().is_empty());
    assert!(!cat.contains("scan"));
    assert_eq!(cat.get("scan"), None);
}

#[test]
fn with_builtins_registers_nine_capabilities() {
    let cat = CapabilityCatalog::with_builtins();
    let ids: HashSet<&'static str> =
        cat.list().iter().map(|d| d.id).collect();
    for id in [
        "scan", "resource", "link", "task", "attachment",
        "community", "artifact", "sync", "inspect",
    ] {
        assert!(ids.contains(id), "missing capability `{id}`");
    }
    assert_eq!(cat.list().len(), 9);
}

#[test]
fn register_overwrites_existing_descriptor() {
    let mut cat = CapabilityCatalog::new();
    cat.register(CapabilityDescriptor::new("custom", "first", Mutability::Read));
    cat.register(CapabilityDescriptor::new("custom", "second", Mutability::Write));
    assert_eq!(cat.get("custom").unwrap().description, "second");
    assert_eq!(cat.get("custom").unwrap().mutability, Mutability::Write);
    assert_eq!(cat.list().len(), 1);
}

#[test]
fn get_returns_hit_or_none() {
    let cat = CapabilityCatalog::with_builtins();
    assert!(cat.get("scan").is_some());
    assert_eq!(cat.get("nonexistent"), None);
}

#[test]
fn iter_yields_each_registered_pair_once() {
    let cat = CapabilityCatalog::with_builtins();
    let mut seen = HashSet::new();
    for (id, desc) in cat.iter() {
        assert!(seen.insert(id), "duplicate id `{id}` in iter");
        assert_eq!(id, desc.id);
    }
    assert_eq!(seen.len(), 9);
}

#[test]
fn application_facade_default_catalog_covers_use_case_traits() {
    // The ApplicationFacade must surface a catalog with the same nine
    // use-case ids after construction. This pins the integration
    // between the facade and CapabilityCatalog::with_builtins.
    let facade = make_facade();
    let cat = facade.capability_catalog();
    let ids: HashSet<&'static str> = cat.list().iter().map(|d| d.id).collect();
    for id in [
        "scan", "resource", "link", "task", "attachment",
        "community", "artifact", "sync", "inspect",
    ] {
        assert!(ids.contains(id), "facade missing `{id}`");
    }
    assert_eq!(cat.list().len(), 9);
}
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `cargo test -p core --test capability_catalog -- --nocapture`
Expected: FAIL with "unresolved import `notez_core::capability::CapabilityCatalog`" (or similar — the struct does not exist yet).

- [ ] **Step 3: Implement `CapabilityCatalog` in `core/src/capability.rs`**

Open `core/src/capability.rs`. The file currently contains the `Mutability` enum, the `CapabilityDescriptor` struct, and its `new` constructor. Append the following at the end of the file (do not modify the existing `Mutability` or `CapabilityDescriptor` definitions):

```rust
use std::collections::HashMap;

/// Registry of public capabilities exposed by the system.
///
/// Capabilities are static descriptors (id, description, mutability)
/// that callers — CLI help text, MCP tool listings, documentation
/// generators — can query at runtime. The catalog is a
/// `HashMap<&'static str, CapabilityDescriptor>` keyed by id.
///
/// `with_builtins()` populates the nine capabilities that map
/// 1-to-1 to the nine `UseCase` traits in `core::application::use_cases`.
/// Third-party code may register additional descriptors via
/// `register`; the public API does not distinguish between built-in
/// and externally-registered entries.
pub struct CapabilityCatalog {
    descriptors: HashMap<&'static str, CapabilityDescriptor>,
}

impl Default for CapabilityCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityCatalog {
    /// Construct an empty catalog.
    pub fn new() -> Self {
        Self { descriptors: HashMap::new() }
    }

    /// Construct a catalog pre-populated with the nine built-in
    /// capabilities. This is the default state used by
    /// `ApplicationFacade::new` and `ApplicationFacade::with_space`.
    pub fn with_builtins() -> Self {
        let mut c = Self::new();
        c.register(CapabilityDescriptor::new(
            "scan",
            "scan native source and rebuild projection",
            Mutability::Read,
        ));
        c.register(CapabilityDescriptor::new(
            "resource",
            "per-resource CRUD, query, read, list",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "link",
            "link occurrence and resolution queries",
            Mutability::Read,
        ));
        c.register(CapabilityDescriptor::new(
            "task",
            "agenda, PARA overview, state transitions",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "attachment",
            "attachment add, extraction, segment query",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "community",
            "community create, list",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "artifact",
            "derived artifacts (summary, llms.txt, context-pack, skill)",
            Mutability::Read,
        ));
        c.register(CapabilityDescriptor::new(
            "sync",
            "sync push, pull, relay, conflict list",
            Mutability::Write,
        ));
        c.register(CapabilityDescriptor::new(
            "inspect",
            "rule inspection, doctor, jobs, artifact freshness",
            Mutability::Read,
        ));
        c
    }

    /// Register `descriptor`. If a descriptor already exists for
    /// `descriptor.id`, the new one replaces it.
    pub fn register(&mut self, descriptor: CapabilityDescriptor) {
        self.descriptors.insert(descriptor.id, descriptor);
    }

    /// Look up a descriptor by id.
    pub fn get(&self, id: &str) -> Option<&CapabilityDescriptor> {
        self.descriptors.get(id)
    }

    /// List all registered descriptors in arbitrary order.
    pub fn list(&self) -> Vec<&CapabilityDescriptor> {
        self.descriptors.values().collect()
    }

    /// True if a descriptor is registered for `id`.
    pub fn contains(&self, id: &str) -> bool {
        self.descriptors.contains_key(id)
    }

    /// Iterate over `(id, descriptor)` pairs in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &CapabilityDescriptor)> {
        self.descriptors.iter().map(|(id, d)| (*id, d))
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p core --test capability_catalog`
Expected: PASS (6 tests).

- [ ] **Step 5: Verify the workspace still builds and no existing tests regress**

Run: `cargo check --workspace --all-targets && cargo test --workspace`
Expected: `cargo check` exit 0, `cargo test` 0 failed.

- [ ] **Step 6: Commit**

```bash
git add core/src/capability.rs core/tests/capability_catalog.rs
git commit -m "feat(capability): add CapabilityCatalog with 9 built-in descriptors"
```

---

## Task 2: Wire `CapabilityCatalog` into `ApplicationFacade`

**Files:**
- Modify: `core/src/application/service.rs`

- [ ] **Step 1: Swap the field on `ApplicationFacade`**

In `core/src/application/service.rs`, find the struct definition:

```rust
pub struct ApplicationFacade<S: ProjectionStore> {
    store: S,
    rule_engine: crate::domain::RuleEngine,
    format_parsers: Vec<Box<dyn crate::source::FormatParser>>,
    space: Option<SpaceContext>,
    capability_log: Vec<crate::capability::CapabilityDescriptor>,
    source_registry: crate::source::SourceRegistry,
}
```

Replace `capability_log: Vec<crate::capability::CapabilityDescriptor>,` with:

```rust
    capability_catalog: notez_core::capability::CapabilityCatalog,
```

(The other five fields are unchanged.)

- [ ] **Step 2: Update both constructors to initialise the new field**

In the same file, find the `new` constructor and the `with_space` constructor. Both currently have a `capability_log: Vec::new(),` initialiser. Replace each with:

```rust
            capability_catalog: notez_core::capability::CapabilityCatalog::with_builtins(),
```

(There are exactly two occurrences: one in `new`, one in `with_space`. Apply the same edit to each.)

Also update the `with_registry` constructor (added in T3 of the prior plan). It currently has `capability_log: Vec::new(),`. Replace that with:

```rust
            capability_catalog: notez_core::capability::CapabilityCatalog::with_builtins(),
```

- [ ] **Step 3: Change the `register_capability` body to delegate to the catalog**

Find the existing `register_capability` method on `ApplicationFacade`. The current body pushes to `capability_log`. Replace the method body so it delegates to the catalog instead. The method stays `pub fn register_capability(&mut self, descriptor: &CapabilityDescriptor)`. The new body is:

```rust
    pub fn register_capability(&mut self, descriptor: &notez_core::capability::CapabilityDescriptor) {
        self.capability_catalog.register(descriptor.clone());
    }
```

- [ ] **Step 4: Add the public `capability_catalog` getter**

Add a new public method on `ApplicationFacade` (place it immediately after `register_capability`):

```rust
    /// Borrow the active capability catalog. Used by CLI help text,
    /// MCP tool listings, and documentation generators to surface a
    /// single source of truth for what the facade can do.
    pub fn capability_catalog(&self) -> &notez_core::capability::CapabilityCatalog {
        &self.capability_catalog
    }
```

- [ ] **Step 5: Verify no remaining references to `capability_log`**

Run:

```bash
grep -nE "capability_log" core/src
```

Expected: empty output. If anything matches, fix the reference (the field no longer exists; all accesses must be removed).

- [ ] **Step 6: Run the workspace test**

Run: `cargo test --workspace`
Expected: 0 failed. Total tests should be 189 (183 + 6 new from Task 1).

- [ ] **Step 7: Commit**

```bash
git add core/src/application/service.rs
git commit -m "refactor(application): route register_capability through CapabilityCatalog"
```

---

## Task 3: Add `--list-capabilities` to the CLI

**Files:**
- Modify: `cli/src/main.rs`
- Modify: `cli/src/commands.rs`

- [ ] **Step 1: Add the flag to `Cli`**

Open `cli/src/commands.rs`. The `Cli` struct currently has `space`, `db`, and `json` as global flags. Add a top-level `list_capabilities` flag. The full `Cli` struct becomes:

```rust
#[derive(Parser, Debug)]
#[command(name = "notez", version, about = "Notez CLI")]
pub struct Cli {
    #[arg(long, global = true)]
    pub space: Option<String>,

    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    #[arg(long, global = true, default_value_t = false)]
    pub json: bool,

    /// List the capabilities the current build exposes and exit.
    #[arg(long, default_value_t = false)]
    pub list_capabilities: bool,

    #[command(subcommand)]
    pub command: Commands,
}
```

(Do not modify `Commands` or any of its subcommand enums in this task.)

- [ ] **Step 2: Add the early-exit handling at the top of `main`**

Open `cli/src/main.rs`. Find the `fn main() { let cli = Cli::parse();` line. Immediately after `let cli = Cli::parse();`, insert the following block (before any space selection or store opening):

```rust
    if cli.list_capabilities {
        // Print the catalog as a JSON array and exit. This is a
        // read-side view of the facade's capabilities; no space,
        // no store, no application service required.
        let facade = notez_core::application::ApplicationFacade::new(
            notez_core::storage::SqliteProjection::open(
                std::path::Path::new("/tmp/notez-capabilities-stub.sqlite"),
            )
            .map_err(|e| {
                eprintln!("Failed to open stub database: {e}");
                std::process::exit(5);
            })?,
        );
        let entries: Vec<serde_json::Value> = facade
            .capability_catalog()
            .list()
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.id,
                    "description": d.description,
                    "mutability": match d.mutability {
                        notez_core::capability::Mutability::Read => "read",
                        notez_core::capability::Mutability::Write => "write",
                    },
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&entries).unwrap());
        std::process::exit(0);
    }
```

(This block sits between `let cli = Cli::parse();` and the existing first space-discovery step.)

- [ ] **Step 3: Run the CLI to confirm the flag works**

Run:

```bash
rm -f /tmp/notez-capabilities-stub.sqlite
cargo run -p cli -- --list-capabilities
```

Expected: a JSON array with 9 entries, each with `id`, `description`, `mutability` fields. The `id` values are `"scan"`, `"resource"`, `"link"`, `"task"`, `"attachment"`, `"community"`, `"artifact"`, `"sync"`, `"inspect"`. The `mutability` values are `"read"` for `scan`/`link`/`artifact`/`inspect` and `"write"` for the rest.

- [ ] **Step 4: Verify the CLI scan smoke still works**

Run:

```bash
rm -rf /tmp/notez-cap-smoke && mkdir -p /tmp/notez-cap-smoke
cat > /tmp/notez-cap-smoke/note.org <<'EOF'
#+title: Catalog Smoke
#+ID: 01J00000000000000000000D01

* NEXT Catalog heading
:PROPERTIES:
:ID: 01J00000000000000000000D02
:END:
EOF
cargo run -p cli -- --space /tmp/notez-cap-smoke scan
```

Expected: `Scanned 1 files, 2 resources, 0 relations.`

- [ ] **Step 5: Run the workspace test**

Run: `cargo test --workspace`
Expected: 0 failed. Total tests should still be 189.

- [ ] **Step 6: Commit**

```bash
git add cli/src/main.rs cli/src/commands.rs
git commit -m "feat(cli): add --list-capabilities flag exposing the facade catalog"
```

---

## Task 4: Final verification

**Files:**
- Modify (audit only): `docs/superpowers/specs/2026-08-02-capability-catalog-design.org` — append a "Verification" section.

- [ ] **Step 1: Run the full workspace test**

Run: `cargo test --workspace`
Expected: 0 failed. Capture the total passed count.

- [ ] **Step 2: Run the workspace build**

Run: `cargo check --workspace --all-targets`
Expected: exit 0.

- [ ] **Step 3: Audit grep: no `capability_log` references remain**

Run:

```bash
grep -nE "capability_log" core/src cli/src cli/tests
```

Expected: empty output.

- [ ] **Step 4: Verify the catalog JSON via the CLI**

Run:

```bash
cargo run -p cli -- --list-capabilities 2>&1 | head -40
```

Expected: a JSON array of 9 objects; each has `id`, `description`, `mutability` (`"read"` or `"write"`).

- [ ] **Step 5: Update the spec with verification counts**

Append a "Verification" section to `docs/superpowers/specs/2026-08-02-capability-catalog-design.org`:

```org
** 8. Verification

- cargo test --workspace: <N> tests passed, 0 failed.
- cargo check --workspace --all-targets: exit 0.
- notez --list-capabilities: outputs 9-entry JSON array; ids are scan/resource/link/task/attachment/community/artifact/sync/inspect.
- Audit grep: no `capability_log` references in core/src, cli/src, or cli/tests.
```

Replace `<N>` with the actual test count from Step 1.

- [ ] **Step 6: Commit the spec update**

```bash
git add docs/superpowers/specs/2026-08-02-capability-catalog-design.org
git commit -m "docs(capability): record CapabilityCatalog final verification counts"
```

---

## Self-Review Notes

**Spec coverage:**

- §2.1 `CapabilityCatalog` API — Task 1 Step 3.
- §2.2 9 built-in descriptors (1-to-1 with `UseCase` traits) — Task 1 Step 3.
- §2.3 `ApplicationFacade` field swap + `register_capability` delegation + new `capability_catalog` getter — Task 2.
- §2.4 CLI `--list-capabilities` — Task 3.
- §3 Verification — Task 4.
- §4 Migration order — Tasks 1, 2, 3, 4 in that order.
- §5 Risks — `capability_log` removal has no test exposure (Task 2 Step 5 audits for this), `HashMap` ordering is not asserted in tests (Task 1 test bodies use `HashSet` for assertions), CLI flag does not conflict with `--space` (placed in the top-level `Cli` struct, not under a subcommand).
- §6 Non-goals — no task touches `ApplicationError`, `SourceRegistry`, dynamic MCP/CLI registration, or use case trait signatures.
- §7 Acceptance — covered by Task 4.

**Type consistency:** `CapabilityCatalog::register(&mut self, descriptor: CapabilityDescriptor)` (consumes the descriptor) and `ApplicationFacade::register_capability(&mut self, descriptor: &CapabilityDescriptor)` (clones the descriptor into the catalog) are consistent: the facade method takes a reference, the catalog method takes ownership, the call site clones. `capability_catalog(&self) -> &CapabilityCatalog` returns a borrow; tests do not store the reference.

**Placeholder scan:** 0 occurrences of "TBD/TODO/FIXME/待定" in the plan.
