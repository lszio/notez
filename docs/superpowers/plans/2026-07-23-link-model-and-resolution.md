# Link Model and Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve and resolve ID, file, title, URL, Markdown, Org and Obsidian links without conflating textual addresses with stable resource identity.

**Architecture:** Add typed link occurrences to `domain`, extract them losslessly in `document`, resolve them in `application`, and project occurrences plus uniquely resolved relations in `storage`. Keep `ResourceRef` strict and derive deterministic refs for resources without explicit IDs.

**Tech Stack:** Rust 2024, serde, sha2, ulid, rusqlite, cargo test

## Global Constraints

- Native Org/Markdown files remain authoritative; SQLite remains rebuildable.
- `ResourceRef` only represents stable identity.
- Every supported textual link becomes a `LinkOccurrence`; only a unique match becomes a `ResolvedRelation`.
- Ambiguous and unresolved links are preserved and diagnosable.
- Common Org, Markdown and Obsidian links work without user configuration.

---

### Task 1: Domain link protocol

**Files:**
- Create: `crates/domain/src/link.rs`
- Modify: `crates/domain/src/lib.rs`
- Test: `crates/domain/tests/link.rs`

**Interfaces:**
- Produces: `LinkTarget`, `LinkOccurrence`, `LinkStatus`, `ResolutionCandidate`, `ResolvedRelation`, `ResourceAddress`.
- Consumes: existing `ResourceRef` and serde conventions.

- [ ] **Step 1: Write serialization and parsing tests**

```rust
#[test]
fn resource_address_keeps_locator_distinct_from_ref() {
    let address = ResourceAddress::parse("file:notes/design.org::Interfaces").unwrap();
    assert!(matches!(address, ResourceAddress::Locator(LinkTarget::File { .. })));
    assert!(ResourceRef::parse("file:notes/design.org").is_err());
}

#[test]
fn occurrence_round_trips_raw_target() {
    let target = LinkTarget::parse("[[Design#API|contract]]", LinkFormat::Obsidian).unwrap();
    assert_eq!(target.raw(), "[[Design#API|contract]]");
    assert_eq!(serde_json::from_str::<LinkTarget>(&serde_json::to_string(&target).unwrap()).unwrap(), target);
}
```

- [ ] **Step 2: Run the failing domain test**

Run: `cargo test -p domain --test link`

Expected: FAIL because `domain::link` and its exported types do not exist.

- [ ] **Step 3: Implement the protocol types**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkTarget {
    Id { raw: String, value: String, kind_hint: Option<ResourceKind> },
    File { raw: String, path: String, fragment: Option<String> },
    Title { raw: String, title: String, fragment: Option<String> },
    Url { raw: String, url: String },
    Custom { raw: String, scheme: String, value: String, fragment: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ResourceAddress { Ref(ResourceRef), Locator(LinkTarget) }
```

Implement `LinkTarget::parse`, `LinkTarget::raw`, and `ResourceAddress::parse` without accepting locators as `ResourceRef`. Define occurrences with source ref, source locator, byte span, display text, relation, format, status and candidates.

- [ ] **Step 4: Run domain tests**

Run: `cargo test -p domain`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/link.rs crates/domain/src/lib.rs crates/domain/tests/link.rs
git commit -m "feat(domain): model textual link targets"
```

### Task 2: Deterministic derived resource identity

**Files:**
- Create: `crates/document/src/identity.rs`
- Modify: `crates/document/src/lib.rs`
- Modify: `crates/document/src/org.rs`
- Modify: `crates/document/src/markdown.rs`
- Test: `crates/document/tests/identity.rs`

**Interfaces:**
- Produces: `derive_resource_ref(version: u8, source_id: &str, relative_locator: &str, kind: ResourceKind, structural_key: &str) -> ResourceRef`.
- Consumes: `ResourceRef`, `ResourceKind`, SHA-256 and normalized source-relative locators.

- [ ] **Step 1: Write repeatability tests**

```rust
#[test]
fn missing_ids_are_stable_across_scans() {
    let first = MarkdownScanner::scan_with_root(&file, root, "native").unwrap();
    let second = MarkdownScanner::scan_with_root(&file, root, "native").unwrap();
    assert_eq!(first.resources.iter().map(|r| r.r#ref).collect::<Vec<_>>(),
               second.resources.iter().map(|r| r.r#ref).collect::<Vec<_>>());
}
```

- [ ] **Step 2: Verify the test fails**

Run: `cargo test -p document --test identity`

Expected: FAIL because scanners currently call `Ulid::new()`.

- [ ] **Step 3: Implement versioned derivation**

Hash `v1\0{source_id}\0{normalized_locator}\0{kind}\0{structural_key}` with SHA-256, take the first 16 bytes as `u128`, and construct `Ulid::from(value)`. Normalize path separators to `/`; reject locators that escape the source root. Use heading ancestry plus same-title ordinal and block syntax position as structural keys.

- [ ] **Step 4: Run scanner tests twice**

Run: `cargo test -p document && cargo test -p document --test identity`

Expected: both commands PASS and repeated scans return equal refs.

- [ ] **Step 5: Commit**

```bash
git add crates/document/src/identity.rs crates/document/src/lib.rs crates/document/src/org.rs crates/document/src/markdown.rs crates/document/tests/identity.rs
git commit -m "feat(document): derive stable refs for implicit resources"
```

### Task 3: Lossless format-specific link extraction

**Files:**
- Create: `crates/document/src/link_scan.rs`
- Modify: `crates/document/src/org.rs`
- Modify: `crates/document/src/markdown.rs`
- Test: `crates/document/tests/link_scan.rs`

**Interfaces:**
- Produces: `scan_org_links(line, line_offset) -> Vec<ParsedLink>` and `scan_markdown_links(line, line_offset, profile) -> Vec<ParsedLink>`.
- Consumes: Task 1 link types.

- [ ] **Step 1: Add table-driven extraction tests**

```rust
for (input, expected_type) in [
    ("[[id:01J00000000000000000000002][Merge]]", "id"),
    ("[[file:design.org::Interfaces][Design]]", "file"),
    ("[site](https://example.com)", "url"),
    ("[[Design#API|contract]]", "title"),
] {
    let links = scan_links(input, LinkFormat::Auto).unwrap();
    assert_eq!(links.len(), 1, "{input}");
    assert_eq!(links[0].target.type_name(), expected_type);
    assert_eq!(&input[links[0].span.clone()], links[0].target.raw());
}
```

- [ ] **Step 2: Verify the extraction test fails**

Run: `cargo test -p document --test link_scan`

Expected: FAIL because current scanners only return parsed ID refs.

- [ ] **Step 3: Implement extractors and change `ScannedDocument.links`**

Replace `Vec<ResourceRelation>` with `Vec<LinkOccurrence>`. Preserve byte spans and raw text. Org handles `id:`, `file:`, URL and custom scheme; Markdown handles inline links; Obsidian profile additionally handles wiki links, aliases, headings, blocks and embeds.

- [ ] **Step 4: Run document and source tests**

Run: `cargo test -p document -p source`

Expected: PASS with updated assertions checking occurrences rather than prematurely resolved relations.

- [ ] **Step 5: Commit**

```bash
git add crates/document/src/link_scan.rs crates/document/src/org.rs crates/document/src/markdown.rs crates/document/tests crates/source/tests
git commit -m "feat(document): preserve links across text formats"
```

### Task 4: Link projection schema

**Files:**
- Modify: `crates/domain/src/query.rs`
- Modify: `crates/storage/src/sqlite.rs`
- Test: `crates/storage/tests/link_projection.rs`

**Interfaces:**
- Produces on `ProjectionStore`: `replace_links`, `list_links`, `replace_resolved_relations`.
- Consumes: occurrences and resolved relations from Tasks 1 and 3.

- [ ] **Step 1: Write persistence tests**

Persist one unresolved file occurrence and one resolved ID occurrence, reopen SQLite, then assert raw target, span, status, candidates and target ref round-trip exactly.

- [ ] **Step 2: Verify schema tests fail**

Run: `cargo test -p storage --test link_projection`

Expected: FAIL because `link_occurrences` and `resolved_relations` do not exist.

- [ ] **Step 3: Add schema and store methods**

Create `link_occurrences(id TEXT PRIMARY KEY, source_ref TEXT, source_locator TEXT, span_start INTEGER, span_end INTEGER, target_json TEXT, display_text TEXT, relation TEXT, format TEXT, status TEXT, candidates_json TEXT, source_id TEXT)` and `resolved_relations(occurrence_id TEXT PRIMARY KEY, source_ref TEXT, relation TEXT, target_ref TEXT, evidence_json TEXT, source_id TEXT)`. Keep the old `relations` table readable during transition.

- [ ] **Step 4: Run storage tests**

Run: `cargo test -p storage`

Expected: PASS, including reopening a file-backed database.

- [ ] **Step 5: Commit**

```bash
git add crates/domain/src/query.rs crates/storage/src/sqlite.rs crates/storage/tests/link_projection.rs
git commit -m "feat(storage): project raw and resolved links"
```

### Task 5: Context-aware resolution and diagnostics

**Files:**
- Create: `crates/application/src/link_resolver.rs`
- Modify: `crates/application/src/lib.rs`
- Modify: `crates/application/src/service.rs`
- Test: `crates/application/tests/link_resolution.rs`
- Test: `tests/golden_suite.rs`

**Interfaces:**
- Produces: `LinkResolver::resolve(&LinkOccurrence, &[Resource]) -> ResolutionDecision`; application methods `list_links`, `resolve_links`, `diagnose_link`, `reindex_links`.
- Consumes: storage APIs from Task 4 and built-in profiles.

- [ ] **Step 1: Write unique, ambiguous and external resolution tests**

Create two `Design.md` resources in different directories. Assert exact relative file link resolves, bare title is ambiguous, missing target remains unresolved, and HTTPS is external.

- [ ] **Step 2: Verify application tests fail**

Run: `cargo test -p application --test link_resolution`

Expected: FAIL because no resolver exists.

- [ ] **Step 3: Implement deterministic candidate ranking**

Generate candidates by exact ref, normalized relative path, alias, basename and exact title. Resolve only when the highest priority has one candidate. Persist candidates and evidence for every decision, then update scan reports with per-status counts.

- [ ] **Step 4: Run focused and golden suites**

Run: `cargo test -p application --test link_resolution && cargo test --test golden_suite`

Expected: PASS with preserved unresolved/ambiguous links and stable identities.

- [ ] **Step 5: Commit**

```bash
git add crates/application/src/link_resolver.rs crates/application/src/lib.rs crates/application/src/service.rs crates/application/tests/link_resolution.rs tests/golden_suite.rs
git commit -m "feat(application): resolve and diagnose textual links"
```

## Task 5 实施现状与缺口 (2026-07-24 追加)

复查代码后，T1–T4 已经在 dev 分支落地 (f4e5039 之前的工作)。T5 只完成
`crates/application/src/link_resolution.rs` 中的最简候选匹配，且尚未提供 plan
要求的四个公共方法、也没有写入 candidates 证据。下列缺口必须在 0.3 收尾时全部
解决。

### 5-A storage 层：持久化 candidates 与诊断字段
- 在 `link_occurrences` 表新增 `candidates_json TEXT NOT NULL DEFAULT '[]'` 与
  `last_resolved_at TEXT`，并在 `replace_links` / 重建时写入。`ProjectionStore`
  trait 增 `list_link_occurrences_with_diagnostics(&self, source_ref)` 返回
  `(LinkOccurrence, Vec<ResourceRef>, ResolutionStatus, Option<String>)`。
- `resolved_relations` 表保留现状 (单 target_ref) 因为唯一命中；候选只在
  `link_occurrences.candidates_json` 留痕。

### 5-B application 层：补齐四个公共方法 + LinkResolver
- 新增 `crates/application/src/link_resolver.rs`，导出 `LinkResolver` 与
  `ResolutionDecision { status, target_ref, candidates, evidence }`。
- 在 `ApplicationService` 增：
  - `list_links(&self, source_ref: &ResourceRef) -> Vec<LinkOccurrence>`
  - `resolve_links(&mut self, source_ref: &ResourceRef) -> Vec<ResolvedRelation>`
  - `diagnose_link(&self, source_ref: &ResourceRef) -> Vec<LinkDiagnostic>`
  - `reindex_links(&mut self, space_root: &Path) -> LinkReindexReport`
- 引入 `ResourceAddress` 入口：让 `read / query / resolve` 接受 `&ResourceAddress`。
  旧 `&ResourceRef` 方法保留为薄包装，避免一次性破坏。
- 决议策略：按 plan §3.4 顺序 (exact ref → normalized path → alias →
  basename/title)；写库时把 `candidates` 和 `evidence` 一并存入
  `link_occurrences.candidates_json`。
- `LinkReindexReport` 含 scanned/resolved/unresolved/ambiguous/external 计数。

### 5-C CLI：补齐诊断与重索引命令
- `notez link diagnose <ref>` 输出按 status 分组的诊断 JSON。
- `notez link reindex [--space <path>]` 重新对扫描出的链接走 resolver，
  输出 `LinkReindexReport`。
- `notez resolve <address>` 接受 `ResourceAddress` 字面量 (`document:...`、
  `file:...`、`[[Title]]` 等)。
- 已有 `link list / link resolved` 行为不变，但 `link list` 现在带 status。

### 5-D MCP：补齐 4 个 link 工具 + 修正 inspect
- `tools/list` 新增：
  - `link_list { ref }`
  - `link_resolve { ref }`
  - `link_diagnose { ref }`
  - `link_reindex { space? }`
- `resolve` 工具接受 `address: string` (覆盖原 `query` 字段)，内部走
  `ResourceAddress::parse`。
- `inspect` 工具改为调用真正的 inspect 操作：输入 `{ ref? }`，输出
  `{ resource, occurrences_by_status, resolved_relations, trace_id }`。
  不再退化为 `query(Selector::new())`。

### 5-E golden suite
- 新建 `tests/golden_suite.rs`，固定一个 fixture 空间 (两个 `Design.md`、一个
  `Design.org`、一个空目录、URL、ID link、显式歧义标题)。
- 断言：
  1. 同名文件 ID link 解析唯一。
  2. 裸标题在两处出现 → status=ambiguous，candidates 长度 >= 2。
  3. 不存在 target → status=unresolved。
  4. HTTPS → status=external，target_ref=None。
  5. 重复扫描 → 所有 ref 与 status 完全稳定。

### 5-F 验收门槛 (合并入 0.3 publish gate)
- 全部测试 `cargo test --workspace` PASS。
- `notez link diagnose` 与 MCP `link_diagnose` 对同 fixture 输出等价 JSON。
- `link_reindex` 后 `unresolved/ambiguous/resolved` 计数与首次扫描完全一致。
- 旧 `relations` 视图仍可读；CLI `link resolved` 与 MCP `link_resolve` 返回
  等价。
