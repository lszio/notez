# Notez Org Behavior and Rules Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement TYPE/Trait/Schema definitions, Org workflow profiles & TODO state transitions, deterministic rule engine (`classify`, `validate`, `derive`, `react`), Agenda views, inspectable rule diagnostics, and CLI/MCP tool integrations.

**Architecture:** Extend domain types with Type/Schema definitions and Workflow Profiles. Introduce a rule evaluator in `domain`/`application` that handles classification, validation, derivation, and reaction. CLI and MCP expose task mutations (`transition`, `schedule`), agenda queries, and `inspect --rules`.

**Tech Stack:** Rust 1.94, Cargo workspace, serde, serde_yaml/serde_json, thiserror, ulid, rusqlite, clap, chrono.

---

### Task 1: Domain Type System, Traits, and Schema Validation

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/domain/Cargo.toml`
- Create: `crates/domain/src/schema.rs`
- Modify: `crates/domain/src/lib.rs`
- Test: `crates/domain/tests/schema.rs`

**Interfaces:**
- Produces: `TypeRegistry`, `TypeDefinition`, `Trait`, `SchemaField`, `PropertyType`, `SchemaValidator`.

- [ ] **Step 1: Write failing schema integration test**

Create `crates/domain/tests/schema.rs`: verify `project` type with required `area` reference and optional `effort` duration; validate invalid properties return structured errors.

- [ ] **Step 2: Run schema test and verify failure**

Run: `cargo test -p domain --test schema`
Expected: FAIL because `schema` module is missing.

- [ ] **Step 3: Implement TypeRegistry and SchemaValidator**

Implement `TypeRegistry`, `TypeDefinition`, `Trait` enum (`Taskable`, `Schedulable`, `ParaItem`, `Summarizable`), `SchemaField`, `PropertyType` (`String`, `Ref`, `Duration`, `DateTime`, `Enum`), and validation logic.

- [ ] **Step 4: Run domain schema tests**

Run: `cargo test -p domain`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/domain
git commit -m "feat: implement type system and schema validation"
```

---

### Task 2: Org Workflow Profiles and TODO State Transitions

**Files:**
- Create: `crates/document/src/workflow.rs`
- Modify: `crates/document/src/lib.rs`
- Test: `crates/document/tests/workflow.rs`

**Interfaces:**
- Produces: `WorkflowProfile::parse(&str)`, `TodoState`, `StateTransition`, `MutationResult`.

- [ ] **Step 1: Write failing workflow test**

Create `crates/document/tests/workflow.rs`: parse `#+TODO: TODO(t) NEXT(n) PEND(p) WAIT(w@/!) | DONE(d!) QUIT(q@)`; verify transition from `NEXT` to `DONE` returns atomic changes (sets state `DONE`, adds `CLOSED: [timestamp]`, inserts `LOGBOOK` state change entry).

- [ ] **Step 2: Run workflow test and verify failure**

Run: `cargo test -p document --test workflow`
Expected: FAIL because `workflow` module is missing.

- [ ] **Step 3: Implement WorkflowProfile parser and StateTransition**

Implement Org `#+TODO` profile parsing (active states vs done states), state transition validator, and text edit builder for Org property drawers and LOGBOOK drawers.

- [ ] **Step 4: Run document workflow tests**

Run: `cargo test -p document`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/document
git commit -m "feat: add org workflow profile and TODO transitions"
```

---

### Task 3: Deterministic Rule Engine and Diagnostics

**Files:**
- Create: `crates/domain/src/rule.rs`
- Modify: `crates/domain/src/lib.rs`
- Modify: `crates/application/src/service.rs`
- Test: `crates/application/tests/rules.rs`

**Interfaces:**
- Produces: `RuleEngine`, `Rule`, `RuleKind` (`Classify`, `Validate`, `Derive`, `React`), `RuleTrace`, `InspectResult`.

- [ ] **Step 1: Write failing rule engine integration test**

Create `crates/application/tests/rules.rs`: evaluate classification rules (e.g. heading under `Projects/` folder or with `TYPE=project` is classified as `project`), derivation rules (compute `para=projects`), and verify `service.inspect_rules(r_ref)` returns full evaluation trace.

- [ ] **Step 2: Run rules integration test**

Run: `cargo test -p application --test rules`
Expected: FAIL because `RuleEngine` is missing.

- [ ] **Step 3: Implement RuleEngine and RuleTrace**

Implement deterministic rule evaluation order: `classify` -> `validate` -> `derive` -> `react`. Store rule traces in `InspectResult`.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/domain crates/application
git commit -m "feat: add deterministic rule engine and inspect trace"
```

---

### Task 4: Task & PARA Application Services and Agenda Views

**Files:**
- Create: `crates/application/src/task_para.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/agenda.rs`

**Interfaces:**
- Produces: `ApplicationService::agenda`, `ApplicationService::transition_task`, `ApplicationService::para_overview`, `AgendaView`, `ParaOverview`.

- [ ] **Step 1: Write failing Agenda and PARA integration test**

Create `crates/application/tests/agenda.rs`: query tasks with `SCHEDULED` or `DEADLINE` properties; transition task state; query PARA overview (projects, areas, resources, archives).

- [ ] **Step 2: Run agenda test**

Run: `cargo test -p application --test agenda`
Expected: FAIL because `agenda` and `transition_task` are missing.

- [ ] **Step 3: Implement Task & PARA methods in ApplicationService**

Implement date filtering for Agenda view, atomic file update on task transition, and PARA entity grouping.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: implement agenda view and task/para application services"
```

---

### Task 5: CLI and MCP Extensions for Rules, Agenda, and Task Transitions

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/cli/tests/rules_cli.rs`
- Test: `crates/mcp/tests/rules_mcp.rs`

**Interfaces:**
- Produces: CLI commands `task transition`, `agenda`, `inspect --rules`; MCP tools `task_transition`, `agenda`, `inspect_rules`.

- [ ] **Step 1: Write failing CLI and MCP rules tests**

Test CLI `notez task transition <REF> --to DONE`, `notez agenda --json`, `notez inspect --rules <REF>`; test corresponding MCP tools.

- [ ] **Step 2: Run CLI and MCP rules tests**

Run: `cargo test -p cli --test rules_cli` and `cargo test -p mcp --test rules_mcp`
Expected: FAIL because CLI/MCP subcommands are missing.

- [ ] **Step 3: Implement CLI subcommands and MCP tools**

Add clap subcommands `task`, `agenda`, `inspect` (with `--rules`); register `task_transition`, `agenda`, `inspect_rules` in MCP stdio server.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli crates/mcp
git commit -m "feat: expose task transition, agenda, and rule inspect over cli and mcp"
```

---

### Task 6: Acceptance Script, Documentation, and Final Verification

**Files:**
- Create: `scripts/acceptance-rules.sh`
- Modify: `README.md`

**Interfaces:**
- Produces: `scripts/acceptance-rules.sh` script testing rules, task transitions, agenda, and rebuild consistency.

- [ ] **Step 1: Write rules acceptance script**

The script runs CLI task transitions, agenda queries, rule inspection, rebuilds index, and compares outputs.

- [ ] **Step 2: Run rules acceptance script**

Run: `bash scripts/acceptance-rules.sh`
Expected: PASS (`rules acceptance: PASS`).

- [ ] **Step 3: Update README.md**

Document TYPE/Trait schema system, rule inspection, TODO transitions, and Agenda CLI/MCP commands.

- [ ] **Step 4: Final verification checks**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace`
Run: `bash scripts/acceptance-core.sh`
Run: `bash scripts/acceptance-rules.sh`
Expected: All exit 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts docs
git commit -m "test: add rules acceptance script and update documentation"
```
