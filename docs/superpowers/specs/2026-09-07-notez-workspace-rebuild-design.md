# Notez Workspace 重建与统一宿主设计

- **日期**：2026-09-07
- **状态**：已批准设计
- **范围**：架构审计后的 workspace 一次性重建、`notez serve`、REST/MCP/CLI、watcher，以及 Dioxus 跨端工作台

## 1. 目标与非目标

### 1.1 目标

本设计为 Notez 建立一个明确的最终依赖图，并支持以下产品形态：

- 一个正式的 `notez` binary；
- `notez serve` 同进程提供 REST、MCP 和 Dioxus Web；
- `notez serve --headless` 只提供 REST 和 MCP；
- CLI 提供服务生命周期、Space、Source、Document、Object、Graph 和 watcher 操作；
- Web、Desktop、Mobile 使用 Dioxus，并共享 Store、Action、ViewModel 和 UI 组件；
- Markdown/Org 文件保持事实来源，SQLite 是可重建投影；
- 支持 Space 内路径访问、稳定 Object ID 访问和块位置引用；
- 支持双链、反向链接、图谱和有证据边界的 LLM Wiki 派生能力；
- 内置 watcher 与独立远程 watcher 使用同一个 ingestion 协议。

### 1.2 非目标

本设计不要求：

- 保留旧 CLI 命令、REST 路由或 MCP tool 名称；允许 breaking change；
- 让浏览器直接调用 MCP 作为 UI transport；
- 通过隐藏 ID、注释或属性自动改写用户原始 Markdown/Org；
- 在首个迁移提交中实现新的外部来源适配器；
- 把 LLM 生成内容当作第二套事实数据库。

接口可以 breaking，但用户的原始文件、可迁移配置和可恢复的知识身份不能因重构而丢失。

## 2. 核心架构

```text
┌──────────────────────────────────────────────────────────────┐
│ Surfaces                                                     │
│ notez CLI │ notez serve │ Dioxus Web │ Desktop │ Mobile      │
│ MCP client / IDE                                             │
└───────────────┬──────────────────────────────────────────────┘
                │ Protocol Request / Response
┌───────────────▼──────────────────────────────────────────────┐
│ Adapters                                                     │
│ REST transport │ MCP transport │ CLI mapping │ UI Backend    │
│ watcher agent / embedded watcher                             │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│ notez-engine                                                 │
│ Dispatcher │ authorization │ use cases │ change pipeline     │
│ projection orchestration │ audit │ ingestion                 │
└───────────────┬──────────────────────┬───────────────────────┘
                │ domain               │ ports
┌───────────────▼──────────────┐ ┌─────▼───────────────────────┐
│ notez-domain                 │ │ notez-ports                  │
│ Object / Source / Space      │ │ SourceReader/Writer          │
│ Command / Query / Change     │ │ Projection / Journal / Audit │
│ Revision / Relation / Graph  │ │ Watch / Blob / Clock         │
│ pure Rust, wasm-compatible   │ │ capability contracts         │
└──────────────────────────────┘ └──────────────┬────────────────┘
                                               │
┌──────────────────────────────────────────────▼───────────────┐
│ Infrastructure                                                 │
│ notez-storage │ notez-fs │ notez-format-org │ notez-format-md │
│ notez-sync │ notez-preview │ watcher implementations          │
└──────────────────────────────────────────────────────────────┘
```

所有外部表面只做参数翻译、传输、认证和呈现。业务语义、授权、revision 检查、Change 构造和投影更新由 `notez-engine` 统一完成。

## 3. 目标 workspace 与依赖规则

```text
crates/
├── domain          # 纯领域模型与算法
├── protocol        # Request/Response/Error/Schema
├── ports           # 应用层依赖的 trait
├── engine          # Dispatcher、Use Case、Change pipeline
├── storage         # SQLite projection、journal、audit、migration
├── fs              # 文件、blob、watcher 的本地实现
├── format-org      # Org parser/writer
├── format-markdown # Markdown parser/writer
├── sync            # folder/object sync 与冲突合并
├── preview         # 预览构建
├── composition     # 唯一 native 装配根
├── api              # REST transport/client
├── mcp              # MCP transport
└── cli              # CLI surface、serve、watch

packages/
├── ui              # 跨端组件、Store、ViewModel、Action
├── web             # Dioxus Web、SSR 和宿主适配
├── desktop         # Dioxus desktop + embedded backend
└── mobile          # Dioxus mobile + remote backend
```

依赖不变量：

1. `domain` 不依赖 SQLite、Axum、Dioxus、Clap 或 MCP；只使用必要的序列化/错误基础设施。
2. `protocol` 表达稳定的 wire DTO，不暴露 SQLite row、HTML 页面结构或 CLI 参数类型。
3. `engine` 依赖 `domain`、`ports` 和 `protocol`，不依赖 Axum、MCP、Clap 或 Dioxus。
4. `storage`、`fs`、格式适配器和 `sync` 实现 `ports`，不能反向依赖 surface crate。
5. `composition` 是唯一组装具体基础设施、Runtime、journal、audit 和 watcher 的 native 根。
6. `api`、`mcp` 和 `cli` 只适配入口协议并调用 engine。
7. `ui` 只依赖 protocol、ViewModel、Backend 和自身设计系统；共享组件不依赖平台实现。
8. `web` 可以承载服务宿主，但 Dioxus 页面逻辑不能进入 `engine`。

最终删除旧的聚合 `notez-core`，不保留其作为长期兼容层。

## 4. 领域模型

### 4.1 Space 与 Source

Space 是逻辑工作区，负责组织、可见范围、保存查询、分类、视图和 Space 级授权。Source 是事实接入与写回边界，负责 adapter、路径、格式、revision、能力、扫描、watch 和同步。

二者保留多对多关系：

```text
Space 1 ──── * SpaceSourceMembership * ──── 1 Source
```

```rust
struct Space {
    id: SpaceId,
    name: String,
    policy: SpacePolicy,
}

struct Source {
    id: SourceId,
    kind: SourceKind,
    capabilities: Capabilities,
    config: SourceConfig,
}

struct SpaceSourceMembership {
    space_id: SpaceId,
    source_id: SourceId,
    role: SourceRole,
    visibility: VisibilityPolicy,
    classification: ClassificationPolicy,
    write_policy: WritePolicy,
    default_view: ViewConfig,
}
```

同一个 Source 可以被多个 Space 复用。多个 Space 都可以发起写入，但所有写操作必须带 `expected_revision`，由 Source 当前事实 revision 原子校验。Space membership 负责主体是否有权发起操作；Source 负责操作是否基于最新事实。

`space remove` 默认只解除注册关系，不删除用户原始目录。Source 的本地写入、watch、同步和冲突属于 Source 边界；Space 的标签、PARA 分类、收藏、保存查询和布局默认不写回 Source 正文。

### 4.2 Document、Object 与 Projection

```text
Space
└── Source
    └── Document
        ├── container Object
        ├── heading Object
        ├── paragraph Object
        ├── task Object
        └── code block Object
```

- `Document` 是一个可读写原始文档和文件事实边界；
- `ObjectIdentity` 是知识图谱中的逻辑节点；
- `ObjectProjection` 描述对象在某个 Source/Document/block 中的当前投影；
- `SpaceObjectView` 描述对象在某个 Space 中的标签、分类、可见性和布局；
- 一个 Document 可以包含多个块级 Object；
- 一个 Object 可以有多个 Source projection；
- 同一 Object 被多个 Space 引用时不复制正文。

```rust
struct ObjectIdentity {
    id: ObjectId,
    kind: ObjectKind,
    identity_state: IdentityState,
}

struct DocumentRef {
    source_id: SourceId,
    path: SourcePath,
    revision: Revision,
}

struct ObjectProjection {
    object_id: ObjectId,
    document: DocumentRef,
    locator: BlockLocator,
    source_revision: Revision,
}
```

### 4.3 块级身份与位置引用

Notez 不自动向 Markdown/Org 写入隐藏 ID、注释或属性。块级对象有两种地址：

```rust
enum ObjectAddress {
    Stable(ObjectId),
    Positioned(PositionedRef),
}

struct PositionedRef {
    space_id: SpaceId,
    source_id: SourceId,
    document_path: SourcePath,
    locator: BlockLocator,
    fingerprint: ContentFingerprint,
}
```

显式 Markdown heading anchor、Org `CUSTOM_ID` 或已有属性可以形成稳定 Object。无显式 ID 的段落、任务和代码块仍可通过路径、范围和内容指纹被引用，但该引用必须标记为 derived/positioned，不得伪装为永久 `ObjectId`。

解析位置引用时：

- 先校验 Source、相对路径、locator 和 fingerprint；
- 可识别的局部变化允许尝试重定位；
- 多个候选或无法匹配时返回 `stale` 或 `ambiguous`；
- 不得静默绑定到另一个块；
- 用户可显式确认候选，确认结果作为 identity mapping 保存；
- 稳定 ID 地址只用于显式可验证身份。

### 4.4 Source-relative 路径与本地路径

规范化内部定位只使用 `source_id + relative_path`。本地绝对路径是输入形式，不是跨机器身份：

```text
/home/me/notes/today.md
  -> SourceRegistry.resolve_path()
  -> source_id + today.md
```

所有路径必须经过 Source policy，禁止 `..` 逃逸、符号链接绕过 root、未注册路径隐式持久化和任意本地文件暴露。

未注册的本地绝对路径可以创建进程内 ephemeral read-only Source，用于一次性读取、预览和临时分析；它不能写入、watch、同步、持久化 Object/Relation、跨请求引用或作为 API/MCP 稳定目标。需要编辑、长期双链或 watcher 时，必须显式注册 Source。

注册命令默认将 Source 加入当前 Space；`--detached` 可只注册不加入 Space。

## 5. 关系、双链、图谱与 LLM Wiki

### 5.1 Relation

文档中的 `[[...]]`、显式 `notez://object/...` 和其它引用统一经过：

```text
raw link -> LinkOccurrence -> target resolution -> ResolvedRelation -> projection
```

默认解析优先级：显式稳定 Object URI、当前 Space 内路径、当前 Space 内别名/标题、配置的跨 Space alias，最后保留 unresolved link。

```rust
enum RelationScope {
    Global,
    Space(SpaceId),
}

struct Relation {
    from: ObjectId,
    to: ObjectId,
    kind: RelationKind,
    scope: RelationScope,
    evidence: Option<EvidenceSpan>,
    source_revision: Option<Revision>,
}
```

源文档语义链接产生全局关系；Space 标签、收藏、PARA 和保存查询是 Space 投影或 Space-scoped 关系。基础关系全局共享，删除 Space 不删除 Object 或全局 Relation；源文档变化导致的关系失效必须显式标记 stale/unresolved。

### 5.2 Graph API

```rust
enum GraphScope {
    Space(SpaceId),
    Source(SourceId),
    Object(ObjectId),
    CrossSpace(Vec<SpaceId>),
}
```

GraphQuery 支持邻居、反向链接和路径查询，默认带授权过滤。Space 图、Source 图、Object 局部图和显式跨 Space 图都使用同一关系事实，不复制图谱语义。

### 5.3 LLM Wiki

LLM Wiki 是 Object/Document/Relation 上的派生能力：

```text
GraphQuery + evidence selection
  -> context
  -> summary generation
  -> stale-aware SummaryProjection
```

生成摘要至少记录 subject Object、source revision、claims、evidence、生成器元数据和 stale 状态。原文变化后摘要标记 stale；生成内容默认不覆盖源文件。任何写回必须经过普通 Command、revision 检查、journal 和 audit。

## 6. 统一 Command/Query 与写入管道

协议核心包含：

```rust
enum Command {
    RegisterSpace(RegisterSpace),
    RemoveSpace(RemoveSpace),
    RegisterSource(RegisterSource),
    AttachSource(AttachSource),
    ScanSpace(ScanSpace),
    IngestWatchBatch(IngestWatchBatch),
    CreateDocument(CreateDocument),
    UpdateDocument(UpdateDocument),
    CreateObject(CreateObject),
    TransitionTask(TransitionTask),
    ResolveConflict(ResolveConflict),
}

enum Query {
    ListSpaces(ListSpaces),
    InspectSpace(InspectSpace),
    ListDocuments(ListDocuments),
    GetDocument(GetDocument),
    ResolveAddress(ResolveAddress),
    GetObject(GetObject),
    Backlinks(BacklinksQuery),
    Graph(GraphQuery),
    GetSummary(GetSummary),
    WatchStatus(WatchStatus),
}
```

所有写操作遵循：

```text
Command
  -> principal / Space / Source authorization
  -> expected revision check
  -> domain validation
  -> Change construction
  -> append journal
  -> source patch/writeback when required
  -> apply/rebuild projection
  -> audit outcome
  -> invalidate affected queries
```

禁止公共写入口使用缺失或空的 `expected_revision` 绕过并发控制。结果至少包含 `Applied`、`StaleRevision`、`Conflict`、`Forbidden` 和 `Unsupported` 等结构化状态。

## 7. `notez serve`、REST、MCP 与 CLI

### 7.1 宿主

同一个 binary 提供：

```text
notez serve
├── REST /api/v1/*
├── MCP /mcp
├── Dioxus SSR/Web
└── embedded watcher（可配置关闭）

notez serve --headless
├── REST /api/v1/*
├── MCP /mcp
└── embedded watcher（可配置关闭）
```

两种模式共享一个 `composition::Runtime`、Dispatcher、Space registry、journal、audit 和 projection store。`--headless` 只控制是否挂载 Dioxus 页面，不创建第二套服务实现。

建议的启动顺序：加载 RuntimeConfig、构建 Runtime、挂载 Dispatcher、按配置启动内置 watcher、挂载 REST/MCP，非 headless 时挂载 Dioxus SSR，最后进入 graceful shutdown loop。

### 7.2 CLI

服务生命周期与业务操作分离：

```text
notez serve [--headless] [--bind ...]
notez status | stop | restart | logs

notez space list
notez space register <path> [--name ...] [--detached]
notez space inspect <space>
notez space remove <space>
notez space scan <space>

notez source register <path> [--detached]
notez source attach <source> <space>

notez document list|get|create|update|move|delete <space> ...
notez object get|backlinks|related <object-id>
notez graph neighbors|path <object-id> ...
notez summary get <object-id>

notez watch --remote <url> --space <space>
notez space watch <space> start|stop|status
```

CLI 仅负责参数、输出和退出码映射；嵌入式操作或远程 REST 操作都必须落到同一个 protocol contract。

### 7.3 REST 与 MCP

Web 和远程客户端使用 REST；Agent、IDE 和自动化使用 MCP。两者在服务端进入相同 Dispatcher：

```text
REST JSON / MCP arguments / CLI args
  -> protocol request
  -> Dispatcher
  -> typed protocol response/error
```

示例资源：

```text
GET /api/v1/spaces/{space}/documents
GET /api/v1/spaces/{space}/documents/by-path/{path}
GET /api/v1/spaces/{space}/objects/{object_id}
GET /api/v1/objects/{object_id}
GET /api/v1/objects/{object_id}/backlinks
GET /api/v1/objects/{object_id}/graph
POST /api/v1/spaces/{space}/events
```

MCP tool 以 `space_list`、`source_register`、`document_get`、`document_update`、`object_get`、`object_backlinks`、`graph_query`、`summary_get` 和 `watch_ingest` 等协议能力为中心，不以 Dioxus 页面 HTML 为主要结果。

首版远程 watcher 仅允许 loopback 或受控内网。认证扩展点预留 anonymous-loopback、service-token 和 OIDC principal，但无认证部署不得被文档描述为公网安全方案。

## 8. Watcher 与 ingestion

watcher 是独立的事件采集模块/worker，不是另一套写入引擎。它支持两种部署：

```text
内置：filesystem watcher -> IngestionPort -> Dispatcher
远程：notez watch -> durable outbox -> POST /api/v1/spaces/{space}/events
```

事件只表达文件系统观察，不直接提交最终正文：

```json
{
  "batch_id": "01J...",
  "space": "notes",
  "events": [
    {
      "kind": "modified",
      "locator": "journal/2026-09-07.org",
      "observed_revision": "sha256:...",
      "observed_at": "2026-09-07T10:00:00Z"
    }
  ]
}
```

服务端验证来源和 locator、按 `batch_id` 幂等、重新读取文件、比较 revision、解析并构建 Change，再更新 journal/projection/audit。远程 watcher 在 retryable 失败时将批次持久化到 outbox；重复批次返回 duplicate；永久拒绝保留诊断记录。watcher 不持有 SQLite connection，也不直接修改投影。

## 9. Dioxus 工作台

### 9.1 共享客户端层

`packages/ui` 提供 Backend、AppStore、reducer/effects、route/selector、ViewModel、组件和 tokens。共享层不依赖 Axum、MCP、SQLite、Clap 或本地绝对路径。

Backend 的能力至少覆盖：Space 列表、Document/Object 读取、地址解析、Graph/Backlinks、Command 执行、watch/sync 状态和结构化错误。

实现为：

```text
WebBackend     -> REST
DesktopBackend -> embedded Runtime/Dispatcher
MobileBackend  -> remote REST
```

所有端遵循：

```text
UI event -> Action -> reducer -> Backend request -> response -> reducer -> ViewModel -> render
```

### 9.2 信息架构与工作面

一级入口：Inbox、Search、Today、Space。主要路由：

```text
/space/{space}
/space/{space}/path/{path}
/space/{space}/object/{object_id}
/space/{space}/graph
/space/{space}/activity
/object/{object_id}
/search
/settings/sources
/settings/sync
```

工作面包含 Reader、Block editor、Source editor、Search、Backlinks、Graph、Activity 和 Sync。所有页面通过 `ObjectAddress`、`DocumentSelector` 和 `GraphQuery` 访问数据，禁止自行拼 SQL、文件路径或内部 locator。

- Reader 默认安全阅读，展示来源证据和地址状态；
- Block editor 通过带 revision 的 Command 产生 patch；
- Source editor 允许明确编辑原文；
- Search 返回可打开的稳定或位置地址；
- Backlinks/Graph 共用 Relation 查询；
- Activity/Sync 展示 Change、ingestion、audit、conflict 和 retry 状态。

### 9.3 SSR 与 hydration

SSR 是首屏和无 hydration 情况下的可用基线。Web 的正式数据 transport 是 REST，不再让大量 `#[server]` 函数承担各页面独立数据层。hydration 可作为增量增强；在其不稳定期间，不允许 Dioxus signal、inline JavaScript 和 server function 同时维护同一状态。progressive enhancement 只能作为可替换的 transport fallback。

## 10. 数据迁移

提供：

```text
notez migrate inspect
notez migrate plan
notez migrate apply
notez migrate verify
```

迁移覆盖旧 `notez.toml`、registered source path、SQLite projection、journal/audit、可恢复 Object identity 和 sync metadata。SQLite projection 可删除后重建；原始 Markdown/Org 不被迁移工具格式化或覆盖。无法恢复的块级身份标记为 derived/stale。迁移失败不删除旧状态，支持 dry-run 和迁移后 scan/index/relation rebuild/verification。

## 11. 测试与验收

### 11.1 测试分层

- `domain`：identity、locator、relation、graph、revision 属性测试；
- `engine`：Command/Query、授权、写入脊柱、冲突、幂等、journal；
- infrastructure：Source policy、parser/writer、SQLite rebuild、watch ingestion；
- surface：REST/MCP/CLI parity、错误映射、serve/headless smoke；
- UI：reducer/action、Backend contract、路由/地址解析、SSR snapshot、跨端 ViewModel。

### 11.2 必须成立的性质

1. CLI、REST、MCP 对同一请求产生等价 typed response；
2. 删除并重建 SQLite 后，文档、对象、关系和图谱可恢复；
3. 同一 Source 被多个 Space 使用时不复制 Object/Relation 事实；
4. 旧 revision 永不覆盖新内容；
5. 重复 watcher batch 不产生重复 Change；
6. 所有路径解析都受 Source policy 约束；
7. 稳定 ID、路径地址和位置地址能解析到同一对象，或明确返回 stale/ambiguous；
8. Web/Desktop/Mobile 对同一 Action 具有相同结果语义；
9. 迁移前后原始文件字节保持不变，除非用户明确执行编辑；
10. Agent 只能通过授权的 protocol 能力获得对象、图谱和摘要证据。

## 12. 迁移顺序

### 子项目一：平台与核心

创建最终 crates，迁移 domain/protocol/ports，建立 engine Dispatcher 和 Change pipeline，迁移 storage/fs/format/sync，实现 Space/Source/Document/Object/Relation、地址解析和 migration tool，最后删除旧 `core`。

验收：核心测试、投影重建、路径/ID resolver、revision/conflict 和写入脊柱通过。

### 子项目二：服务与入口

实现正式 `notez serve` binary、Web/headless 两种模式、REST、MCP stdio/HTTP、CLI Space/Source/Document/Object/Graph 命令、embedded/remote watcher、统一错误和服务管理。

验收：CLI/REST/MCP parity、watch ingestion 幂等、安全边界和服务 smoke tests 通过。

### 子项目三：Dioxus 工作台

实现共享 Store/Backend/ViewModel，接入 Web REST、Desktop embedded、Mobile remote，完成 Reader/Editor/Search/Backlinks/Activity/Graph/Sync，并统一 SSR、路由、错误和冲突面板。

验收：三端 Action contract 相同；Web 通过 REST 完成工作台流程；服务端 `/mcp` 可被 Agent 使用。

## 13. 关键风险与缓解

- **删除 `core` 的迁移量大**：用最终依赖图、crate 编译闸门和 contract tests 分段验证。
- **块级对象不写回 ID**：强制区分 stable、derived、stale、ambiguous，禁止静默重绑定。
- **多个 Space 并发写同一 Source**：所有公共写操作强制 `expected_revision` 和原子 patch。
- **远程 watcher 无认证**：仅限 loopback/受控网络，保留 token/OIDC 扩展点。
- **Dioxus hydration 不稳定**：REST + SSR 是独立可用基线，hydration 不作为服务可用性前置条件。
- **跨 Space 图谱泄漏**：GraphQuery 显式 scope，并在 engine 层做授权过滤。
