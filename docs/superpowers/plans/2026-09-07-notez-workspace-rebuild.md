# Notez Workspace 重建与统一宿主 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将当前以 `notez-core` 为聚合中心的 workspace 重建为 domain/protocol/ports/engine/infrastructure/surface 分层，并交付统一的 `notez serve`、REST、MCP、CLI、watcher 与 Dioxus 知识工作台。

**Architecture:** 先建立最终依赖图和可编译的 `notez-domain`、`notez-ports`、`notez-engine` 契约，再迁移 storage/fs/format/sync 实现；`notez-composition` 是唯一 native 装配根，REST/MCP/CLI 和 Dioxus 都只调用同一个 Dispatcher。文件与同步对象是事实来源，SQLite 是可重建投影；Space 通过 membership 复用 Source，Document/Object/Relation 提供路径、稳定 ID、位置引用和图谱语义。

**Tech Stack:** Rust 2024、Cargo workspace、Serde/JSON Schema、SQLite/rusqlite、Axum 0.8、Tokio、rmcp、Clap 4、Dioxus 0.7.10、Markdown/Org adapters、notify 6。

## Global Constraints

- 允许旧 CLI 命令、REST 路由和 MCP tool 名称 breaking change，但原始 Markdown/Org、可迁移配置和可恢复知识身份必须保留。
- `notez serve` 提供 REST + MCP + Dioxus SSR；`notez serve --headless` 只提供 REST + MCP；两者共享同一个 Runtime 和 Dispatcher。
- `notez-core` 不作为目标态长期兼容层；最终 workspace 使用 `domain`、`protocol`、`ports`、`engine`、`storage`、`fs`、`format-*`、`sync`、`composition`、`api`、`mcp`、`cli`。
- `domain` 不依赖 SQLite、Axum、Dioxus、Clap 或 MCP；`engine` 不依赖 transport；surface crate 不实现业务规则。
- Markdown/Org 是事实来源；SQLite projection、索引和缓存可删除后重建。
- Space 与 Source 保持多对多；同一 Source 可被多个 Space 写入，但每个公共写操作必须携带 `expected_revision`。
- 不自动向 Markdown/Org 写入隐藏块 ID；显式 anchor/属性形成稳定对象，其余块使用带 locator/fingerprint 的位置引用。
- 未注册绝对路径只允许 ephemeral read-only 解析；写入、watch、同步和持久化引用必须先注册 Source。
- 全局文档关系共享；Space 只增加 scoped view/organization 语义；GraphQuery 必须带 scope 并经过授权过滤。
- watcher 只上报观察事件；服务端重新读取、校验、解析并产生 Change；远程 watcher 首版只允许 loopback/受控内网。
- REST 是 Dioxus Web/Mobile 的 transport；MCP 面向 Agent/IDE/自动化，不作为浏览器 UI transport。
- 所有写操作经过 authorization、revision check、domain validation、Change/journal、writeback、projection、audit；不得以空 revision 绕过并发检查。
- 每个任务必须先运行该任务列出的失败测试，再实现最小代码，最后运行局部测试并提交一个可回滚 commit。
- 编辑函数、struct impl 或 method 前必须用 GitNexus `impact({target: "symbolName", direction: "upstream"})` 检查调用者和风险；高风险结果必须先报告并调整迁移批次。
- 提交前必须运行 GitNexus `detect_changes({scope: "compare", base_ref: "main"})`，确认只影响计划中的 symbols/flows。

---

## 0. 当前文件地图与目标文件布局

当前主要实现位置：

- `crates/core/src/domain/{mod,resource,link,change,journal,audit,conflict,query,schema}.rs`：领域模型、资源、关系、Change、journal 和 audit 类型。
- `crates/core/src/application/{mod,dispatcher,projector,service,ports,context,graph,watch,write_check,writeback}.rs`：Dispatcher、Use Case、投影写入、watch 和写入检查。
- `crates/core/src/application/use_cases/*.rs` 与 `use_cases_impl/*.rs`：协议操作的接口和实现。
- `crates/core/src/source/*.rs`：Source adapter、registry、policy、reader/writer 和 native/git/remote 实现。
- `crates/core/src/storage/{sqlite,journal,blob,cache}.rs`：SQLite projection、journal、blob 和 cache。
- `crates/core/src/document/*.rs`：文档扫描、块解析、安全策略和格式抽象。
- `crates/core/src/sync/*.rs`：manifest、object store、transport、merge 和 relay。
- `crates/composition/src/lib.rs`：当前 Runtime、Source 选择、SQLite 打开、parser 注册和 watcher cache。
- `crates/protocol/src/{request,response,error,schema}.rs`：当前 wire DTO。
- `crates/api/src/{transport,client,config,error_map,auth}.rs`：REST transport/client/auth。
- `crates/mcp/src/{server,http}.rs`：MCP stdio/HTTP mapping。
- `crates/cli/src/{main,commands,handlers,host}.rs`：CLI surface、serve/host/watch。
- `packages/ui/src/{backend,store,notez}.rs`：共享 Backend、Store 与 UI primitives。
- `packages/web/src/{main,host,routes,server,router,pages}.rs`：Dioxus SSR、API/MCP host 和页面。
- `packages/desktop/src/backend.rs` 与 `packages/mobile/src/backend.rs`：embedded/HTTP Backend。

目标文件布局：

```text
crates/domain/src/{lib,identity,address,space,source,document,object,relation,graph,change,revision,error}.rs
crates/ports/src/{lib,source,projection,journal,audit,watch,clock,blob}.rs
crates/engine/src/{lib,dispatcher,authorization,commands,queries,projector,ingestion,summary}.rs
crates/storage/src/{lib,sqlite,journal,audit,blob,migrations,rebuild}.rs
crates/fs/src/{lib,local_source,policy,watcher,outbox}.rs
crates/format-org/src/{lib,parser,writer}
crates/format-markdown/src/{lib,parser,writer}
crates/sync/src/{lib,object,manifest,merge,transport,relay}
```

既有 `crates/protocol`、`crates/api`、`crates/mcp`、`crates/composition`、`crates/cli`、`packages/ui`、`packages/web`、`packages/desktop`、`packages/mobile` 按后续任务修改；`crates/core` 只在迁移未完成前作为临时编译桥，不能新增业务能力。

---

## Task 1: 建立基线、影响清单与最终 workspace 骨架

**Files:**
- Create: `crates/domain/Cargo.toml`, `crates/domain/src/lib.rs`
- Create: `crates/ports/Cargo.toml`, `crates/ports/src/lib.rs`
- Create: `crates/engine/Cargo.toml`, `crates/engine/src/lib.rs`
- Create: `crates/storage/Cargo.toml`, `crates/storage/src/lib.rs`
- Create: `crates/fs/Cargo.toml`, `crates/fs/src/lib.rs`
- Create: `crates/format-org/Cargo.toml`, `crates/format-org/src/lib.rs`
- Create: `crates/format-markdown/Cargo.toml`, `crates/format-markdown/src/lib.rs`
- Create: `crates/sync/Cargo.toml`, `crates/sync/src/lib.rs`
- Modify: `Cargo.toml:3-16`
- Test: workspace compile and existing workspace tests

**Interfaces:**
- Produces empty, compilable packages named `notez-domain`, `notez-ports`, `notez-engine`, `notez-storage`, `notez-fs`, `notez-format-org`, `notez-format-markdown`, and `notez-sync`.
- Existing consumers continue using `notez-core` until migration tasks replace their imports.

- [ ] **Step 1: Capture the baseline.**

Run:

```bash
cargo test --workspace
cargo build --workspace
```

Expected: all currently passing tests remain passing; record the number of result lines and any pre-existing warnings in the task commit message.

- [ ] **Step 2: Run impact analysis for the current public seams.**

Run GitNexus impact for `notez_core::domain::Resource`, `notez_core::application::Engine`, `notez_core::application::dispatcher::Dispatcher`, `notez_composition::native::Runtime`, and `notez_ui::Backend`, direction `upstream`. Save the direct callers, affected processes, and risk levels in `journal/2026-09-07-workspace-rebuild-impact.md`; if any result is HIGH/CRITICAL, split its migration into a separate compile gate before editing.

- [ ] **Step 3: Add the new packages with only dependency declarations.**

Each new `Cargo.toml` must use edition 2024 and the narrowest dependencies:

```toml
[package]
name = "notez-domain"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }
ulid = { workspace = true }
```

Use equivalent package names for the other crates; `notez-ports` depends on `notez-domain`, `notez-engine` on `notez-domain`, `notez-ports`, `notez-protocol`, and `notez-storage`/`notez-fs`/format/sync only depend on the contracts they implement.

- [ ] **Step 4: Register new members and verify the empty graph.**

Run:

```bash
cargo metadata --no-deps --format-version 1 >/tmp/notez-metadata.json
cargo check -p notez-domain -p notez-ports -p notez-engine -p notez-storage -p notez-fs -p notez-format-org -p notez-format-markdown -p notez-sync
```

Expected: all new packages compile and no new package depends on `notez-core`.

- [ ] **Step 5: Commit.**

```bash
git add Cargo.toml crates/domain crates/ports crates/engine crates/storage crates/fs crates/format-org crates/format-markdown crates/sync journal/2026-09-07-workspace-rebuild-impact.md
git commit -m "refactor: add final workspace crate boundaries"
```

---

## Task 2: Extract domain identity, address, Space/Source, Document/Object and graph contracts

**Files:**
- Modify: `crates/domain/src/lib.rs`
- Create: `crates/domain/src/identity.rs`
- Create: `crates/domain/src/address.rs`
- Create: `crates/domain/src/space.rs`
- Create: `crates/domain/src/source.rs`
- Create: `crates/domain/src/document.rs`
- Create: `crates/domain/src/object.rs`
- Create: `crates/domain/src/relation.rs`
- Create: `crates/domain/src/graph.rs`
- Create: `crates/domain/src/revision.rs`
- Create: `crates/domain/src/error.rs`
- Test: `crates/domain/tests/address_resolution.rs`, `crates/domain/tests/graph_scope.rs`, `crates/domain/tests/revision.rs`
- Reference: `crates/core/src/domain/{resource,link,change,conflict,query,schema}.rs`

**Interfaces:**
- Produces `SpaceId`, `SourceId`, `DocumentId`, `ObjectId`, `Revision`, `SourcePath`, `BlockLocator`, `ContentFingerprint`, `ObjectAddress`, `PositionedRef`, `SpaceSourceMembership`, `ObjectProjection`, `Relation`, `RelationScope`, `GraphScope`, `GraphQuery`, and `DomainError`.
- `ObjectAddress::Stable(ObjectId)` and `ObjectAddress::Positioned(PositionedRef)` are the only public object selectors; no UI or transport type may reconstruct a locator ad hoc.

- [ ] **Step 1: Write failing identity and address tests.**

```rust
#[test]
fn positioned_address_keeps_source_relative_path_and_fingerprint() {
    let address = ObjectAddress::Positioned(PositionedRef {
        space_id: SpaceId::new("work"),
        source_id: SourceId::new("notes"),
        document_path: SourcePath::parse("journal/today.md").unwrap(),
        locator: BlockLocator::line_span(4, 7),
        fingerprint: ContentFingerprint::from_bytes(b"hello"),
    });
    assert_eq!(address.source_id(), Some(&SourceId::new("notes")));
    assert_eq!(address.document_path().unwrap().as_str(), "journal/today.md");
}

#[test]
fn source_path_rejects_escape_and_absolute_paths() {
    assert!(SourcePath::parse("../secret.md").is_err());
    assert!(SourcePath::parse("/etc/passwd").is_err());
}
```

- [ ] **Step 2: Run the focused tests to verify failure.**

```bash
cargo test -p notez-domain --test address_resolution
```

Expected: compile failure because the new domain types do not exist.

- [ ] **Step 3: Move only pure types from `core::domain` and implement the new contracts.**

Keep parsing, SQLite, file I/O and JSON route concerns out of the new crate. `SourcePath::parse` must normalize separators, reject absolute paths, reject `..`, and return a typed error. `ContentFingerprint` must be deterministic and serializable. `Revision` must distinguish source content revision from application Change cursor.

- [ ] **Step 4: Add relation and graph scope tests.**

```rust
#[test]
fn global_relations_are_not_duplicated_by_space_membership() {
    let relation = Relation::global(ObjectId::new("a"), ObjectId::new("b"), RelationKind::LinksTo);
    assert_eq!(relation.scope, RelationScope::Global);
}

#[test]
fn cross_space_graph_scope_is_explicit() {
    assert!(GraphScope::CrossSpace(vec![SpaceId::new("a"), SpaceId::new("b")]).is_cross_space());
}
```

- [ ] **Step 5: Run all domain tests and compatibility checks.**

```bash
cargo test -p notez-domain
cargo check -p notez-core
```

Expected: domain tests pass and the old core still compiles because this task does not remove its modules.

- [ ] **Step 6: Commit.**

```bash
git add crates/domain
 git commit -m "refactor: define notez domain identity and graph model"
```

---

## Task 3: Extract ports and complete protocol Command/Query/Response contracts

**Files:**
- Modify: `crates/ports/src/lib.rs`
- Create: `crates/ports/src/source.rs`, `projection.rs`, `journal.rs`, `audit.rs`, `watch.rs`, `clock.rs`, `blob.rs`
- Modify: `crates/protocol/src/request.rs`, `response.rs`, `error.rs`, `schema.rs`, `lib.rs`
- Test: `crates/ports/tests/object_safe_ports.rs`, `crates/protocol/tests/contract_parity.rs`
- Reference: `crates/core/src/application/ports.rs`, `crates/core/src/application/wire.rs`, `crates/core/src/application/write_check.rs`

**Interfaces:**
- Produces object-safe ports: `SourceReader`, `SourceWriter`, `ProjectionReader`, `ProjectionWriter`, `ChangeJournal`, `AuditLog`, `WatchIngestion`, `BlobStore`, and `Clock`.
- Produces typed protocol variants `Command`, `Query`, `Response`, `CommandResult`, `ObjectAddress`, `GraphQuery`, `IngestWatchBatch`, and errors `StaleRevision`, `Conflict`, `Forbidden`, `SourceUnavailable`, `Retryable`.

- [ ] **Step 1: Write failing port and protocol contract tests.**

```rust
#[test]
fn every_write_command_has_a_revision_field() {
    for command in sample_write_commands() {
        assert!(command.expected_revision().is_some());
    }
}

#[test]
fn protocol_error_round_trips_as_json_schema() {
    let error = Error::StaleRevision { expected: "a".into(), current: "b".into() };
    let encoded = serde_json::to_string(&error).unwrap();
    assert_eq!(serde_json::from_str::<Error>(&encoded).unwrap(), error);
}
```

- [ ] **Step 2: Run tests and verify failure.**

```bash
cargo test -p notez-protocol --test contract_parity
cargo test -p notez-ports --test object_safe_ports
```

Expected: missing command variants, error variants, and ports cause compilation failures.

- [ ] **Step 3: Define the ports without infrastructure types.**

Ports must pass domain snapshots and typed outcomes. `SourceWriter::apply` accepts a snapshot plus patch list and returns `Applied { new_revision }`, `Stale { current_revision }`, or `Unsupported`; it must not accept `rusqlite::Connection`, Axum requests, or Dioxus state.

- [ ] **Step 4: Extend protocol wire types.**

Add requests for Space, Source, Document, Object, graph, summary and watcher ingestion. Every mutating request includes `principal`, `space_id`/`source_id` where applicable, and `expected_revision: String`; no public default converts missing revisions into unconditional writes.

- [ ] **Step 5: Generate and inspect schemas.**

```bash
cargo test -p notez-protocol
cargo run -p cli -- list-capabilities --json
```

Expected: schemas include the new object/address/graph/watch variants and structured errors.

- [ ] **Step 6: Commit.**

```bash
git add crates/ports crates/protocol
 git commit -m "refactor: establish protocol and infrastructure ports"
```

---

## Task 4: Extract engine Dispatcher, authorization, use cases and change pipeline

**Files:**
- Modify: `crates/engine/src/lib.rs`
- Create: `crates/engine/src/dispatcher.rs`, `authorization.rs`, `commands.rs`, `queries.rs`, `projector.rs`, `ingestion.rs`, `summary.rs`
- Reference/migrate: `crates/core/src/application/{dispatcher,projector,service,write_check,writeback,graph,watch}.rs`
- Test: `crates/engine/tests/write_spine.rs`, `parity.rs`, `watch_ingestion.rs`, `authorization.rs`

**Interfaces:**
- Produces `Dispatcher<P>` where `P` is a ports bundle, `Dispatcher::execute(Command) -> Result<CommandResult, ApplicationError>`, and `Dispatcher::query(Query) -> Result<Response, ApplicationError>`.
- Produces `IngestionService::accept(WatchBatch) -> IngestionReceipt` with `accepted`, `duplicate`, `rejected`, and `retryable` states.
- Produces authorization checks over `Principal × Space × Source × Action × Scope`.

- [ ] **Step 1: Run impact analysis for `Engine`, `Dispatcher`, `Projector`, and `check_revision`.**

Before moving or changing each symbol, run GitNexus upstream impact and record callers. Do not edit a HIGH/CRITICAL symbol until its callers are migrated behind a new engine contract.

- [ ] **Step 2: Write failing write-spine tests.**

```rust
#[test]
fn stale_revision_never_reaches_source_writer() {
    let harness = TestEngine::new_with_revision("r2");
    let result = harness.execute(update_document("r1"));
    assert!(matches!(result, Err(ApplicationError::StaleRevision { .. })));
    assert_eq!(harness.writer_calls(), 0);
}

#[test]
fn accepted_watch_batch_is_idempotent() {
    let harness = TestEngine::new();
    let first = harness.ingest(batch("b1"));
    let second = harness.ingest(batch("b1"));
    assert_eq!(first, IngestionReceipt::Accepted);
    assert_eq!(second, IngestionReceipt::Duplicate);
    assert_eq!(harness.change_count(), 1);
}
```

- [ ] **Step 3: Run focused tests to verify failure.**

```bash
cargo test -p notez-engine --test write_spine --test watch_ingestion
```

Expected: test harness and Dispatcher implementation are absent.

- [ ] **Step 4: Move use-case behavior behind Dispatcher.**

Port the existing use-case implementation from `core/src/application/use_cases_impl` without exposing `Engine<SqliteProjection>` in the public API. The Dispatcher must perform authorization, revision, validation, Change construction, journal append, writeback, projection, and audit in a single ordered path.

- [ ] **Step 5: Add query/address/graph operations.**

`ResolveAddress` must resolve stable IDs and position addresses through ports. `Graph` must accept `GraphScope` and apply authorization before returning nodes/edges. `GetSummary` must return stale state when source revision differs from summary revision.

- [ ] **Step 6: Run engine tests.**

```bash
cargo test -p notez-engine
cargo test -p notez-protocol --test contract_parity
```

Expected: revision, idempotency, authorization, graph scope and protocol parity tests pass.

- [ ] **Step 7: Commit.**

```bash
git add crates/engine
 git commit -m "refactor: move use cases behind unified dispatcher"
```

---

## Task 5: Move SQLite projection, journal, audit, filesystem and format adapters

**Files:**
- Modify: `crates/storage/src/lib.rs`
- Create: `crates/storage/src/sqlite.rs`, `journal.rs`, `audit.rs`, `blob.rs`, `migrations.rs`, `rebuild.rs`
- Modify: `crates/fs/src/lib.rs`
- Create: `crates/fs/src/local_source.rs`, `policy.rs`, `watcher.rs`, `outbox.rs`
- Modify: `crates/format-org/src/lib.rs`, `crates/format-markdown/src/lib.rs`
- Move implementation from: `crates/core/src/storage/*.rs`, `source/{adapter,writer,policy,native,registry}.rs`, `document/{org,markdown,security,content_hash,notez_block}.rs`
- Test: `crates/storage/tests/rebuild.rs`, `crates/fs/tests/path_policy.rs`, `crates/fs/tests/watcher_outbox.rs`, format adapter tests

**Interfaces:**
- `SqliteProjection` implements the projection ports but is not imported by domain or engine public types.
- `LocalSource` implements `SourceReader`/`SourceWriter` and resolves only normalized source-relative paths.
- `EmbeddedWatcher` emits `WatchBatch`; `DurableOutbox` stores batches for retry without opening a SQLite connection inside the watcher callback.
- Org/Markdown adapters implement parser/writer ports and preserve source bytes unless a command explicitly applies a patch.

- [ ] **Step 1: Run impact analysis for `SqliteProjection`, `NativeSourceAdapter`, `SourcePolicy`, `WatchService`, and parser traits.**

Record upstream callers before moving each symbol; migrate callers through ports rather than adding new cross-crate imports to `notez-core`.

- [ ] **Step 2: Write failing infrastructure tests.**

```rust
#[test]
fn projection_can_be_deleted_and_rebuilt_from_source() {
    let source = fixture_source();
    let first = scan_and_query(&source);
    delete_projection_database(&source);
    let rebuilt = scan_and_query(&source);
    assert_eq!(first.objects, rebuilt.objects);
    assert_eq!(first.relations, rebuilt.relations);
}

#[test]
fn local_source_rejects_symlink_escape_and_oversized_files() {
    let source = fixture_source_with_escape_and_large_file();
    assert!(source.read("../outside.md").is_err());
    assert!(source.read("large.md").is_err());
}
```

- [ ] **Step 3: Run tests to verify failure.**

```bash
cargo test -p notez-storage --test rebuild
cargo test -p notez-fs --test path_policy
```

Expected: new implementations are not yet present.

- [ ] **Step 4: Move storage implementations and migrations.**

Preserve existing SQLite schema data where possible, add explicit projection schema version, and implement `rebuild_from_sources()` that clears only projections/indexes while retaining migration-readable journal/audit data. Ensure journal cursor and audit rows use typed domain Change IDs.

- [ ] **Step 5: Move local source/policy/watcher code.**

`SourcePolicy::allows` must be called by scan, search candidate collection, file tree, preview, and Janet context. `EmbeddedWatcher` only buffers normalized events. `DurableOutbox` acknowledges a batch only after the remote API returns accepted or duplicate.

- [ ] **Step 6: Move format adapters and run their existing parser/patch tests.**

```bash
cargo test -p notez-format-org
cargo test -p notez-format-markdown
cargo test -p notez-storage -p notez-fs
```

Expected: parser round trips, source policy, projection rebuild and outbox idempotency pass.

- [ ] **Step 7: Commit.**

```bash
git add crates/storage crates/fs crates/format-org crates/format-markdown
 git commit -m "refactor: extract storage filesystem and format infrastructure"
```

---

## Task 6: Move sync, preview, config and composition Runtime to the final contracts

**Files:**
- Modify: `crates/sync/src/{lib,object,manifest,merge,transport,relay}.rs`
- Modify: `crates/preview/src/lib.rs` and `crates/preview/Cargo.toml`
- Modify: `crates/composition/src/lib.rs`, `crates/composition/Cargo.toml`
- Move config modules from: `crates/core/src/config/*.rs`
- Test: sync tests, composition runtime tests, preview decoupling tests

**Interfaces:**
- `notez-sync` implements sync ports over `Change`, object/snapshot heads and conflict records; conflict resolution is itself an engine Command.
- `composition::native::Runtime` owns one engine cache, one watcher service, one source registry, one projection/journal/audit set per configured runtime, and parser registration.
- `Runtime::open_source(SourceId)` returns a typed `RuntimeHandle` exposing Dispatcher, not raw `Engine<SqliteProjection>`.

- [ ] **Step 1: Run impact analysis for `Runtime`, `open_selected`, `SyncEngine`, `ThreeWayMerger`, and preview builders.**

Record all CLI/API/Web/Desktop callers; do not change the public return type until adapters have a replacement `RuntimeHandle` available.

- [ ] **Step 2: Write failing Runtime and sync tests.**

```rust
#[test]
fn same_source_id_reuses_one_runtime_handle() {
    let runtime = test_runtime();
    let a = runtime.open_source("notes").unwrap();
    let b = runtime.open_source("notes").unwrap();
    assert!(std::sync::Arc::ptr_eq(&a.dispatcher(), &b.dispatcher()));
}

#[test]
fn conflict_resolution_emits_a_change() {
    let result = sync_fixture().resolve_conflict("doc.md", KeepSide::Mine).unwrap();
    assert!(result.change_id.is_some());
}
```

- [ ] **Step 3: Move sync and preview implementation.**

Keep preview builders read-only and dependent on domain/protocol views, never on Axum or Dioxus. Make sync heads content-addressed at the new contract boundary; preserve old metadata through migration adapters.

- [ ] **Step 4: Replace composition raw Engine exposure with RuntimeHandle.**

Update composition callers in API, CLI, MCP and packages to call Dispatcher methods. Runtime remains the sole place that opens SQLite, registers Org/Markdown parsers, attaches journal/audit, and starts an embedded watcher.

- [ ] **Step 5: Run all core/infrastructure tests.**

```bash
cargo test -p notez-sync -p notez-preview -p notez-composition
cargo test --workspace
```

Expected: all migrated tests pass; any remaining `notez-core` imports are listed explicitly for Task 7 and are not new dependencies from target crates.

- [ ] **Step 6: Commit.**

```bash
git add crates/sync crates/preview crates/composition
 git commit -m "refactor: assemble runtime from final infrastructure crates"
```

---

## Task 7: Rebuild REST, MCP and CLI around Dispatcher and formalize `notez serve`

**Files:**
- Modify: `crates/api/src/{transport,client,config,error_map,auth,lib}.rs`
- Modify: `crates/mcp/src/{server,http,lib}.rs`
- Modify: `crates/cli/src/{commands,main,host,lib}.rs` and `crates/cli/src/handlers/*`
- Create: `crates/cli/src/serve.rs`, `crates/cli/src/space.rs`, `crates/cli/src/source.rs`, `crates/cli/src/document.rs`, `crates/cli/src/object.rs`, `crates/cli/src/graph.rs`, `crates/cli/src/watch_remote.rs`
- Test: `crates/api/tests/dispatch.rs`, `crates/mcp/tests/protocol_parity.rs`, `crates/cli/tests/{serve,space,document,object,graph,watch_remote}.rs`

**Interfaces:**
- REST handlers accept/return protocol `Request`/`Response`; `ApiState` holds `RuntimeHandle`/Dispatcher and never an independent engine cache.
- MCP tool handlers map tool arguments to the same protocol requests used by REST; stdio and streamable HTTP share the same handler.
- CLI exposes `serve [--headless]`, service `status/stop/restart/logs`, `space`, `source`, `document`, `object`, `graph`, `summary`, and `watch --remote` commands.

- [ ] **Step 1: Run impact analysis for `notez_api::serve`, `protocol_router`, MCP handler entrypoints, `run_serve`, and CLI `commands::Commands`.**

The current `run_serve` and `packages/web` host are high fan-out symbols; record their callers and migrate through a new `ServeConfig`/`RuntimeHandle` contract rather than editing handlers in place.

- [ ] **Step 2: Write failing parity tests.**

```rust
#[tokio::test]
async fn rest_mcp_and_cli_use_equivalent_document_request() {
    let request = Request::GetDocument(GetDocumentRequest::by_path("work", "notes/today.md"));
    let rest = dispatch_rest(request.clone()).await;
    let mcp = dispatch_mcp("document_get", request.clone()).await;
    assert_eq!(rest.into_response(), mcp.into_response());
}

#[test]
fn serve_headless_does_not_mount_dioxus_routes() {
    let app = build_service_router(ServeConfig { headless: true, ..test_config() });
    assert_route_status(&app, "/api/v1/healthz", 200);
    assert_route_status(&app, "/", 404);
}
```

- [ ] **Step 3: Run focused tests to verify failure.**

```bash
cargo test -p notez-api --test dispatch
cargo test -p notez-mcp --test protocol_parity
cargo test -p cli --test serve
```

Expected: the new formal serve config, protocol parity harness and command variants are missing.

- [ ] **Step 4: Implement unified service mounting.**

`ServeConfig` must include bind address, `headless`, source/config selection, watcher enabled flag and auth mode. Both modes build one Runtime, one Dispatcher, REST routes and MCP handler; only web mode merges Dioxus SSR routes. Graceful shutdown stops watcher, drains ingestion, flushes journal/outbox, then closes listeners.

- [ ] **Step 5: Implement Space/Source/Document/Object/Graph CLI mappings.**

Each handler constructs one protocol request and prints typed JSON or human output. `space remove` detaches membership and never deletes the source directory. Absolute paths are resolved through SourceRegistry; unregistered paths use only read-only ephemeral resolution.

- [ ] **Step 6: Implement remote watcher ingestion.**

`notez watch --remote URL --space SPACE` uses filesystem events, normalizes them into `WatchBatch`, persists retryable batches in the outbox, POSTs `/api/v1/spaces/{space}/events`, and treats `accepted`/`duplicate` as terminal success. No request can upload arbitrary final projection data.

- [ ] **Step 7: Run surface tests and smoke commands.**

```bash
cargo test -p notez-api -p notez-mcp -p cli
cargo run -p cli -- serve --help
cargo run -p cli -- space --help
cargo run -p cli -- document --help
cargo run -p cli -- object --help
cargo run -p cli -- graph --help
```

Expected: all surface tests pass and help output lists the new operations.

- [ ] **Step 8: Commit.**

```bash
git add crates/api crates/mcp crates/cli
 git commit -m "feat: unify serve REST MCP CLI and watcher surfaces"
```

---

## Task 8: Rebuild shared Dioxus Backend, Store and knowledge-workbench routes

**Files:**
- Modify: `packages/ui/src/backend.rs`, `store.rs`, `lib.rs`, `notez.rs`
- Create: `packages/ui/src/address.rs`, `actions.rs`, `view_models.rs`, `errors.rs`, `graph.rs`
- Modify: `packages/web/src/{backend,host,server,router,routes,model}.rs` and `packages/web/src/pages/*.rs`
- Modify: `packages/desktop/src/backend.rs`, `packages/mobile/src/backend.rs`
- Test: `packages/ui/tests/backend_contract.rs`, `packages/web` route/SSR tests, desktop/mobile compile tests

**Interfaces:**
- `ui::Backend` exposes `list_spaces`, `resolve_address`, `get_document`, `get_object`, `search`, `backlinks`, `graph`, `activity`, `sync_status`, and `execute(Command)` with typed protocol results.
- `AppStore` owns route, active Space, selection, query, cache, pending commands, notifications, sync state and typed errors.
- `WebBackend` uses REST; `DesktopBackend` uses `RuntimeHandle`; `MobileBackend` uses REST; no page calls a `#[server]` function directly.

- [ ] **Step 1: Run impact analysis for `ui::Backend`, `AppStore`, Web server functions, and each Desktop/Mobile Backend implementation.**

Record all pages implementing the current 3-method trait. Migrate the trait with an adapter layer before deleting old methods so all three clients remain compilable at each commit.

- [ ] **Step 2: Write failing Backend contract tests.**

```rust
#[tokio::test]
async fn all_backends_resolve_path_and_id_with_same_semantics() {
    for backend in test_backends() {
        let path = backend.resolve_address(path_address()).await.unwrap();
        let by_id = backend.resolve_address(path.object_address()).await.unwrap();
        assert_eq!(path.object_id(), by_id.object_id());
    }
}

#[test]
fn store_reduces_command_conflict_into_typed_error_state() {
    let store = AppStore::default();
    store.reduce(Response::Conflict(conflict_fixture()));
    assert!(matches!(store.error_state(), UiError::Conflict { .. }));
}
```

- [ ] **Step 3: Run tests to verify failure.**

```bash
cargo test -p ui --test backend_contract
cargo check -p web -p desktop -p mobile
```

Expected: current Backend lacks the new operations and ViewModels.

- [ ] **Step 4: Implement shared selectors, actions, reducers and ViewModels.**

Use `ObjectAddress`, `DocumentSelector`, `GraphQuery`, and protocol Commands. Keep platform-specific navigation and file dialogs outside `packages/ui`. All errors retain `NotFound`, `ReadOnly`, `StaleRevision`, `Conflict`, `Unauthorized`, `SourceUnavailable`, and `Internal { trace_id }` states.

- [ ] **Step 5: Replace Web page-level server functions with WebBackend calls.**

Keep SSR data loading in one backend adapter. REST remains the browser transport; MCP is not called from browser code. Progressive enhancement may submit a transport fallback but cannot maintain a second Store or duplicate filtering implementation.

- [ ] **Step 6: Add workbench routes and views.**

Implement `/space/:space`, `/space/:space/path/*path`, `/space/:space/object/:object_id`, `/space/:space/graph`, `/space/:space/activity`, `/object/:object_id`, `/search`, `/settings/sources`, and `/settings/sync`. Reader, Block editor, Source editor, Search, Backlinks, Graph, Activity and Sync consume shared ViewModels.

- [ ] **Step 7: Run UI and surface tests.**

```bash
cargo test -p ui
cargo test -p web
cargo build -p desktop
cargo build -p mobile --features mobile
```

Expected: Backend contract, route, SSR and cross-platform compilation tests pass.

- [ ] **Step 8: Commit.**

```bash
git add packages/ui packages/web packages/desktop packages/mobile
 git commit -m "feat: build shared dioxus knowledge workbench"
```

---

## Task 9: Add migration commands and remove the old core crate

**Files:**
- Create: `crates/storage/src/migrations.rs` tests and `crates/cli/src/handlers/migrate.rs`
- Modify: `crates/cli/src/commands.rs`, `crates/cli/src/main.rs`
- Modify: `Cargo.toml`
- Delete after all imports are removed: `crates/core/Cargo.toml`, `crates/core/src/**`
- Test: `crates/storage/tests/migration.rs`, `crates/cli/tests/migrate_cli.rs`, full workspace tests

**Interfaces:**
- Commands: `notez migrate inspect`, `plan`, `apply`, `verify`, plus `--dry-run`.
- Migration preserves source paths, readable files, recoverable Object IDs, journal/audit data and sync metadata; projection/index can be rebuilt.

- [ ] **Step 1: Run impact analysis for every remaining `notez_core` import.**

Use GitNexus callers plus `rg -n 'notez_core|notez-core' crates packages Cargo.toml` and create a checklist grouped by target crate. No deletion is allowed while any production import remains.

- [ ] **Step 2: Write failing migration tests.**

```rust
#[test]
fn dry_run_never_changes_source_bytes() {
    let fixture = legacy_fixture();
    let before = read_all_source_bytes(&fixture);
    migration_plan(&fixture).unwrap();
    assert_eq!(before, read_all_source_bytes(&fixture));
}

#[test]
fn rebuild_marks_unrecoverable_block_identity_stale() {
    let result = migrate_fixture_with_moved_block().unwrap();
    assert!(result.identities.iter().any(|i| i.state == IdentityState::Stale));
}
```

- [ ] **Step 3: Implement inspect/plan/apply/verify.**

`inspect` reports detected versions and source paths; `plan` reports actions without writes; `apply` writes only versioned metadata and migrated projection state; `verify` rescans and compares object/relation counts, file bytes and journal cursors. Failed apply leaves the old state intact.

- [ ] **Step 4: Remove all production `notez-core` dependencies.**

Update Cargo manifests, imports and re-exports so API/MCP/CLI/Composition/Packages depend on target crates. Keep compatibility conversion code only in migration modules, not in runtime paths.

- [ ] **Step 5: Delete old core and run complete checks.**

```bash
cargo test --workspace
cargo build --workspace
./scripts/check-surfaces.sh
```

Expected: no `notez-core` workspace member or production import remains; all configured surfaces build.

- [ ] **Step 6: Commit.**

```bash
git add Cargo.toml crates packages scripts
 git commit -m "refactor: remove aggregate core after workspace migration"
```

---

## Task 10: Verification, impact review, documentation and release gate

**Files:**
- Modify: `docs/current-architecture-and-redesign.md`
- Modify: `docs/roadmap.org`
- Modify: `README.org`, `packages/web/README.md`, `packages/desktop/README.md` as needed
- Create: `docs/architecture/notez-workspace-rebuild.md` if the architecture overview needs a stable short reference
- Test: all workspace and surface checks

**Interfaces:**
- Documentation describes the implementation actually present, not the earlier target state.
- Release gate documents `serve`, `--headless`, Space/Source/Object commands, path/ID URI rules, watcher network restriction and Dioxus backend boundaries.

- [ ] **Step 1: Run the full test and build matrix.**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
./scripts/check-surfaces.sh
```

Expected: formatting, clippy, unit/integration tests, all binaries and Web/Desktop/Mobile surfaces pass.

- [ ] **Step 2: Run service smoke tests.**

```bash
cargo run -p cli -- serve --headless --bind 127.0.0.1:8765
curl -fsS http://127.0.0.1:8765/api/v1/healthz
curl -fsS http://127.0.0.1:8765/api/v1/capabilities
```

In a second shell, run the MCP initialize request against `/mcp`, then stop the process with the CLI service command. Expected: health/capabilities/MCP all return successful typed responses; headless mode has no Dioxus page route.

- [ ] **Step 3: Run cross-surface behavior checks.**

Use one fixture Source attached to two Spaces and verify:

```text
space list/register/attach
source-relative path read
stable Object ID read
positioned block read
backlinks and scoped graph
write with current revision
stale write rejection
watch batch accepted then duplicate
projection delete and rebuild
```

Expected: one Object/Relation fact is visible in both authorized Spaces; stale writes never overwrite source bytes; duplicate watcher batches create no duplicate Change.

- [ ] **Step 4: Run GitNexus change detection before any release commit.**

Run `detect_changes({scope: "compare", base_ref: "main"})`. Confirm changed symbols and execution flows are limited to the planned workspace, service, watcher, protocol and Dioxus surfaces. Investigate unexpected flows before committing.

- [ ] **Step 5: Update architecture and roadmap documents.**

Mark only implemented milestones as complete. Explicitly document any remaining hydration limitation, remote watcher authentication limitation, sync limitations and migration caveats. Do not describe a planned feature as shipped.

- [ ] **Step 6: Commit the release documentation.**

```bash
git add docs README.org packages/web/README.md packages/desktop/README.md
 git commit -m "docs: record rebuilt workspace and service release contract"
```

- [ ] **Step 7: Final branch verification.**

```bash
git status --short
git log --oneline -12
git diff main...HEAD --stat
```

Expected: only intended commits and files are present; pre-existing user modifications to `docs/current-architecture-and-redesign.md`, `docs/roadmap.org` and `journal/` are not accidentally overwritten.

---

## Coverage Review

- **Architecture/crate boundaries:** Tasks 1, 2, 3, 4, 5, 6, 9.
- **Space/Source membership and write concurrency:** Tasks 2, 4, 5, 7.
- **Document/Object/Relation/Graph and path/ID/position addressing:** Tasks 2, 3, 4, 8.
- **File-first storage and rebuild/migration:** Tasks 5, 6, 9, 10.
- **Unified write spine, journal, audit and conflict:** Tasks 3, 4, 5, 6.
- **`notez serve` Web/headless:** Task 7 and Task 10.
- **REST/MCP/CLI parity:** Tasks 3, 7, 10.
- **Embedded/remote watcher:** Tasks 5, 7, 10.
- **Dioxus Web/Desktop/Mobile workbench:** Task 8 and Task 10.
- **LLM Wiki summary projection:** Tasks 2, 4, 7, 8; summary generation remains a typed derived capability and never becomes a second fact store.
- **Security and authorization:** Tasks 4, 5, 7, 8, 10.

## Self-review Results

- No unresolved placeholder is used as an implementation step.
- Every target symbol used by a later task is introduced in an earlier task or identified as an existing symbol in the file map.
- The plan preserves the approved distinction between Space and Source, global and Space-scoped relations, stable and positioned addresses, and REST versus MCP responsibilities.
- Existing user modifications are explicitly protected in Task 10.
- The old `notez-core` crate is deleted only after all target crates and surface adapters compile and the migration tests pass.
