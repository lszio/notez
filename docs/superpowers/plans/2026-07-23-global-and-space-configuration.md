# Global and Space Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Load strict versioned configuration from an XDG global space registry and a movable per-space `notez.toml`, with explainable precedence and explicit legacy migration.

**Architecture:** Create a focused `config` crate that discovers paths, parses global/space TOML, selects a space, merges overrides into `RuntimeConfig`, and reports provenance. CLI constructs every service from this result before opening SQLite.

**Tech Stack:** Rust 2024, serde, toml, clap, tempfile, cargo test

## Global Constraints

- Global config discovers spaces; space config owns space behavior.
- Relative space paths resolve from the containing `notez.toml`.
- Built-in link profiles require no user configuration.
- Unknown fields and invalid versions fail before database access.
- Legacy JSON migration is explicit, previewable and non-destructive.

---

### Task 1: Add the config crate and strict schemas

**Files:**
- Create: `crates/config/Cargo.toml`
- Create: `crates/config/src/lib.rs`
- Create: `crates/config/src/model.rs`
- Modify: `Cargo.toml`
- Test: `crates/config/tests/model.rs`

**Interfaces:**
- Produces: `GlobalConfig`, `SpaceRegistration`, `SpaceConfig`, `RuntimeConfig`, `ConfigError`.
- Consumes: `source::SourceConfig` after mapping `name` to the existing source identifier.

- [ ] **Step 1: Write strict TOML parsing tests**

```rust
#[test]
fn unknown_space_field_is_rejected() {
    let err = SpaceConfig::parse("version=1\n[space]\nname='x'\nnaem='typo'").unwrap_err();
    assert!(err.to_string().contains("space.naem"));
}
```

- [ ] **Step 2: Verify tests fail**

Run: `cargo test -p config --test model`

Expected: FAIL because the crate does not exist.

- [ ] **Step 3: Implement version-1 schemas**

Use `#[serde(deny_unknown_fields)]` on versioned structs. Model global `default_space`, `spaces` and preferences; model space identity, database, workflow, sources and optional link overrides. Add `toml = "0.8"` and `source` dependencies.

- [ ] **Step 4: Run config model tests**

Run: `cargo test -p config`

Expected: PASS for valid examples and field-path errors for invalid examples.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/config
git commit -m "feat(config): define strict global and space schemas"
```

### Task 2: XDG discovery and space selection

**Files:**
- Create: `crates/config/src/discovery.rs`
- Modify: `crates/config/src/lib.rs`
- Test: `crates/config/tests/discovery.rs`

**Interfaces:**
- Produces: `ConfigPaths::discover(env, cwd)`, `SpaceSelector`, `select_space`.
- Consumes: Task 1 schemas.

- [ ] **Step 1: Write selection precedence tests**

Cover explicit registered name, explicit path, upward `notez.toml`, global default, missing selection and `$XDG_CONFIG_HOME` fallback. Use a fake environment map rather than mutating process-global environment in parallel tests.

- [ ] **Step 2: Verify discovery tests fail**

Run: `cargo test -p config --test discovery`

Expected: FAIL because discovery functions are absent.

- [ ] **Step 3: Implement pure discovery functions**

Accept `&BTreeMap<String, OsString>` and `&Path` as inputs. Expand `~` only in declared path fields, canonicalize existing ancestors, and return `NoSpaceSelected`, `UnknownSpace` or `SpaceConfigNotFound` with paths.

- [ ] **Step 4: Run discovery tests**

Run: `cargo test -p config --test discovery`

Expected: PASS without reading the developer's real home config.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/discovery.rs crates/config/src/lib.rs crates/config/tests/discovery.rs
git commit -m "feat(config): discover registered and local spaces"
```

### Task 3: Merge, provenance and built-in link defaults

**Files:**
- Create: `crates/config/src/merge.rs`
- Create: `crates/config/src/defaults.rs`
- Modify: `crates/config/src/lib.rs`
- Test: `crates/config/tests/merge.rs`

**Interfaces:**
- Produces: `load_runtime_config(LoadRequest) -> Result<RuntimeConfig, ConfigError>` and `EffectiveValue<T> { value, origin }`.
- Consumes: global and space paths from Task 2.

- [ ] **Step 1: Write precedence and path-base tests**

Assert built-in profile < global preference < space config < environment < CLI override, and assert `database` plus source paths are resolved relative to space config. Assert an empty space config still enables Org/Markdown/Obsidian defaults.

- [ ] **Step 2: Verify merge tests fail**

Run: `cargo test -p config --test merge`

Expected: FAIL because merge and defaults modules are absent.

- [ ] **Step 3: Implement typed merge and provenance**

Represent origins as `BuiltIn`, `Global(PathBuf)`, `Space(PathBuf)`, `Environment(String)` and `Cli`. Merge optional scalar fields and keyed source entries; merge link overrides field-by-field over immutable built-in profiles.

- [ ] **Step 4: Run all config tests**

Run: `cargo test -p config`

Expected: PASS and snapshots show the origin of every effective value.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/defaults.rs crates/config/src/merge.rs crates/config/src/lib.rs crates/config/tests/merge.rs
git commit -m "feat(config): merge effective configuration with provenance"
```

### Task 4: Integrate configuration before service startup

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Test: `crates/cli/tests/config_cli.rs`

**Interfaces:**
- Produces CLI flags `--space <name-or-path>`, `--config <path>`, commands `config show`, `config validate`, `space list/register/unregister/inspect`.
- Consumes: `load_runtime_config` from Task 3.

- [ ] **Step 1: Write CLI tests with isolated XDG config**

Run the binary with `XDG_CONFIG_HOME` pointed at a temp directory. Assert `--space personal query` opens the registered space, upward discovery works, and invalid TOML exits 2 without creating `.notez/index.sqlite`.

- [ ] **Step 2: Verify CLI tests fail**

Run: `cargo test -p cli --test config_cli`

Expected: FAIL because `--space` only accepts a `PathBuf` defaulting to `.` and config commands are absent.

- [ ] **Step 3: Refactor startup**

Parse selection first, call `load_runtime_config`, print structured configuration errors, then create the configured database parent and `ApplicationService`. `config show --json` serializes values with origins; registration commands atomically rewrite global TOML through a temp file and rename.

- [ ] **Step 4: Run CLI test suite**

Run: `cargo test -p cli`

Expected: PASS for old path-based usage and new registered-space usage.

- [ ] **Step 5: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/commands.rs crates/cli/src/main.rs crates/cli/tests/config_cli.rs
git commit -m "feat(cli): load global and space configuration"
```

### Task 5: Explicit legacy migration

**Files:**
- Create: `crates/config/src/migrate.rs`
- Modify: `crates/config/src/lib.rs`
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Test: `crates/config/tests/migrate.rs`
- Test: `crates/cli/tests/config_migrate_cli.rs`

**Interfaces:**
- Produces: `plan_legacy_migration(space_root) -> MigrationPlan`, `apply_legacy_migration(plan)` and `notez config migrate [--apply]`.
- Consumes: `.notez/sources.json`, `.notez/communities.json` and target space config.

- [ ] **Step 1: Write preview, conflict and preservation tests**

Assert preview performs no writes; apply merges non-conflicting sources; conflicting source names stop before writing; legacy files remain present after success; communities produce a separate domain declaration artifact.

- [ ] **Step 2: Verify migration tests fail**

Run: `cargo test -p config --test migrate`

Expected: FAIL because migration APIs do not exist.

- [ ] **Step 3: Implement plan/apply split**

Parse legacy JSON strictly, generate a serializable change report, write candidate TOML/domain files beside their targets, fsync and atomically rename only after all validations pass. Never delete legacy input.

- [ ] **Step 4: Run config and CLI migration tests**

Run: `cargo test -p config -p cli --test config_migrate_cli`

Expected: PASS; preview leaves the fixture tree byte-for-byte unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/migrate.rs crates/config/src/lib.rs crates/config/tests/migrate.rs crates/cli/src/commands.rs crates/cli/src/main.rs crates/cli/tests/config_migrate_cli.rs
git commit -m "feat(config): migrate legacy space metadata explicitly"
```
