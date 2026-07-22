# Notez Communities and Agent Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Communities (`Community` entity with selector + pinned/excluded members), Recipe derivation engine (`summary`, `llms.txt`, `context-pack`, `skill-ir`), SKILL.md exporter (`SKILL.md + references/`), and CLI/MCP `community` and `artifact` commands.

**Architecture:** Add `Community` to `domain`. Build `Recipe` engine in `artifact` to generate `summary`, `llms.txt`, `context-pack`, and `skill-ir`. Update `ApplicationService` to manage communities, derive artifacts with token budgets, and export Skill packages. Expose CLI/MCP community and skill export interfaces.

**Tech Stack:** Rust 1.94, Cargo workspace, serde, serde_json, thiserror, ulid, rusqlite, clap.

---

### Task 1: Community Domain Algebra and Candidate Generation

**Files:**
- Create: `crates/domain/src/community.rs`
- Modify: `crates/domain/src/lib.rs`
- Test: `crates/domain/tests/community.rs`

**Interfaces:**
- Produces: `Community`, `CommunityCandidate`, `CommunitySelector`.

- [ ] **Step 1: Write failing community integration test**

Create `crates/domain/tests/community.rs`: test creating a `Community` with a selector (e.g. `kind=heading`, `title_contains=sync`), pinned member refs, and excluded member refs; evaluate members against a set of resources.

- [ ] **Step 2: Run test and verify missing community module failure**

Run: `cargo test -p domain --test community`
Expected: FAIL because `community` module is missing.

- [ ] **Step 3: Implement Community and CommunitySelector**

Implement `Community`, `CommunityCandidate`, `CommunitySelector` matching logic in `crates/domain/src/community.rs` and export in `lib.rs`.

- [ ] **Step 4: Run domain tests**

Run: `cargo test -p domain`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/domain
git commit -m "feat: add community domain algebra and candidate selector"
```

---

### Task 2: Recipe Engine and Core Artifact Generators

**Files:**
- Create: `crates/artifact/src/recipe.rs`
- Create: `crates/artifact/src/generators.rs`
- Modify: `crates/artifact/src/lib.rs`
- Test: `crates/artifact/tests/recipe.rs`

**Interfaces:**
- Produces: `Recipe`, `RecipeKind` (`Summary`, `LlmsTxt`, `ContextPack`, `SkillIr`), `DerivedArtifact`, `RecipeEvaluator`.

- [ ] **Step 1: Write failing recipe integration test**

Create `crates/artifact/tests/recipe.rs`: generate `summary`, `llms.txt`, and `context-pack` artifacts for a set of resources within a token budget.

- [ ] **Step 2: Run recipe test**

Run: `cargo test -p artifact --test recipe`
Expected: FAIL because `recipe` module is missing.

- [ ] **Step 3: Implement Recipe Engine and Generators**

Implement `RecipeKind` (`Summary`, `LlmsTxt`, `ContextPack`, `SkillIr`), `DerivedArtifact`, and generators enforcing token/character budgets and markdown formatting.

- [ ] **Step 4: Run artifact tests**

Run: `cargo test -p artifact`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/artifact
git commit -m "feat: add recipe derivation engine for summary, llms.txt, and context-pack"
```

---

### Task 3: Skill IR and SKILL.md Exporter

**Files:**
- Create: `crates/artifact/src/skill.rs`
- Modify: `crates/artifact/src/lib.rs`
- Test: `crates/artifact/tests/skill.rs`

**Interfaces:**
- Produces: `SkillIr`, `SkillPackage`, `SkillExporter`.

- [ ] **Step 1: Write failing Skill exporter test**

Create `crates/artifact/tests/skill.rs`: compile resources into `SkillIr` and export a full `SkillPackage` containing `SKILL.md`, `references/`, and `resources.json`.

- [ ] **Step 2: Run skill exporter test**

Run: `cargo test -p artifact --test skill`
Expected: FAIL because `SkillExporter` is missing.

- [ ] **Step 3: Implement Skill IR and Exporter**

Implement `SkillIr` compilation from community resources, generate clean `SKILL.md` frontmatter & prompt instructions, and structure output directory layout.

- [ ] **Step 4: Run artifact tests**

Run: `cargo test -p artifact`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/artifact
git commit -m "feat: implement Skill IR and SKILL.md exporter"
```

---

### Task 4: Community and Artifact Application Services

**Files:**
- Create: `crates/application/src/community_app.rs`
- Modify: `crates/application/src/service.rs`
- Modify: `crates/application/src/lib.rs`
- Test: `crates/application/tests/community_app.rs`

**Interfaces:**
- Produces: `ApplicationService::create_community`, `ApplicationService::derive_artifact`, `ApplicationService::export_skill`.

- [ ] **Step 1: Write failing community application test**

Create `crates/application/tests/community_app.rs`: test creating a community, deriving a `summary` / `llms.txt` artifact, and exporting a Skill package to disk.

- [ ] **Step 2: Run application test**

Run: `cargo test -p application --test community_app`
Expected: FAIL because `create_community` is missing.

- [ ] **Step 3: Implement Community & Artifact Application Services**

Implement `create_community`, `list_communities`, `derive_artifact` (evaluating recipes against space resources), and `export_skill`.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/application
git commit -m "feat: implement community management and artifact derivation application services"
```

---

### Task 5: CLI and MCP Extensions for Communities and Artifacts

**Files:**
- Modify: `crates/cli/src/commands.rs`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/mcp/src/server.rs`
- Test: `crates/cli/tests/community_cli.rs`
- Test: `crates/mcp/tests/community_mcp.rs`

**Interfaces:**
- Produces: CLI commands `community create/list`, `derive <RECIPE>`, `skill export`; MCP tools `community_create`, `derive_artifact`, `export_skill`.

- [ ] **Step 1: Write failing CLI and MCP community tests**

Test CLI `notez community create --name "DevSync"`, `notez derive llms-txt`, `notez skill export`; test corresponding MCP tools.

- [ ] **Step 2: Run CLI and MCP community tests**

Run: `cargo test -p cli --test community_cli` and `cargo test -p mcp --test community_mcp`
Expected: FAIL because community subcommands/tools are missing.

- [ ] **Step 3: Implement CLI subcommands and MCP tools**

Add clap `community`, `derive`, `skill` subcommands; register `community_create`, `derive_artifact`, `export_skill` in MCP stdio server.

- [ ] **Step 4: Run workspace tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cli crates/mcp
git commit -m "feat: expose community management, artifact derivation, and skill export over cli and mcp"
```

---

### Task 6: Acceptance Script, Documentation, and Final Verification

**Files:**
- Create: `scripts/acceptance-artifacts.sh`
- Modify: `README.md`

**Interfaces:**
- Produces: `scripts/acceptance-artifacts.sh` script testing communities, artifact derivation, SKILL.md export, and rebuild consistency.

- [ ] **Step 1: Write artifacts acceptance script**

The script creates a community, derives `summary` and `llms.txt`, exports a Skill package, rebuilds index, and compares output consistency.

- [ ] **Step 2: Run artifacts acceptance script**

Run: `bash scripts/acceptance-artifacts.sh`
Expected: PASS (`artifacts acceptance: PASS`).

- [ ] **Step 3: Update README.md**

Document Communities, Recipe Artifacts (`summary`, `llms.txt`, `context-pack`, `skill-ir`), SKILL.md Exporter, and CLI/MCP commands.

- [ ] **Step 4: Final verification checks**

Run: `cargo fmt --all -- --check`
Run: `cargo clippy --workspace --all-targets -- -D warnings`
Run: `cargo test --workspace`
Run: `bash scripts/acceptance-core.sh`
Run: `bash scripts/acceptance-rules.sh`
Run: `bash scripts/acceptance-federation.sh`
Run: `bash scripts/acceptance-attachments.sh`
Run: `bash scripts/acceptance-artifacts.sh`
Expected: All exit 0.

- [ ] **Step 5: Commit**

```bash
git add README.md scripts docs
git commit -m "test: add artifacts acceptance script and update documentation"
```
