# Notez 当前架构与整体重设计评审

> 状态：基于当前代码仓库评审。本文区分“当前实现”和“建议目标态”，不把设计文档中的计划误写成已完成能力。

## 1. 结论先行

Notez 当前已经形成了一个可工作的本地优先知识索引系统，但还不是一个稳定的知识应用平台。核心问题不在于缺少更多功能，而在于三个抽象没有完全闭合：

1. **事实、事件、投影没有形成单一状态模型**：原始文件、SQLite 投影、同步对象、journal、缓存文件分别保存不同部分的状态，写入链路也没有完全统一。
2. **客户端运行模型不稳定**：Web 同时存在 SSR、Dioxus server functions、WASM hydration、原生表单和 progressive-enhancement JavaScript 五种交互机制。它们可以互相兜底，但没有一个明确的客户端状态机。
3. **产品信息架构仍以“资源列表”为中心**：用户真正要完成的是阅读、捕获、关联、修改、回顾和推进工作，而当前界面主要展示扫描结果、文件树和调试信息。

建议不要继续在现有页面上堆叠功能。下一阶段应先冻结一个更小、更稳定的核心模型：

```text
Intent
  -> Command / Query
  -> Domain validation
  -> Change journal
  -> Projection
  -> View model
  -> UI action / next intent
```

所有客户端、CLI、MCP 和 HTTP API 都只做协议翻译和呈现；所有写入都必须经过同一条 Change 管道；所有 UI 都从同一个会话状态和查询结果渲染。

---

## 2. Notez 当前解决的问题

Notez 的核心定位是：

> 一个以本地 Org/Markdown 文件为事实来源、可聚合多个来源、可被人类客户端和 Agent 共同操作的知识系统。

当前优先级大致是：

1. 本地文件可读、可编辑、可移植；
2. 文档、标题、代码块、附件和链接拥有稳定身份；
3. 多个 Source 可以被一个运行时聚合；
4. CLI、MCP、Web 使用尽可能一致的领域语义；
5. 本地变化可以同步、合并并暴露冲突；
6. 为后续 Agent 上下文、代码索引、摘要和工作流提供基础。

这个定位是合理的。真正需要重新考虑的是“系统内部如何表达这些能力”和“用户如何理解这些能力”。

---


## 1.5 里程碑（M1–M3，已落地于 dev 分支）

下表对应 2026-09 在 dev 分支已经实现的形态与边界变更。代码 commit 引用见 README.org 与 git log。

| 里程碑 | 关键交付 | 关键 commit |
|---|---|---|
| M1 服务统一 | =crates/mcp= 从 cli 抽出；stdio 行为不变；9 个原本休眠的 MCP integration 测试被激活并修复了 =source_list= 工具路由 bug | 9be47b3 |
| M1 服务统一 | =composition::native::Runtime= 取代双引擎缓存；ApiState/WebState 委托；进程单例 watcher | cab6f47 |
| M1 服务统一 | =crates/mcp http= feature：rmcp streamable-http-server；web host 在 =/mcp= 挂同一 handler | 09aa357 |
| M1 服务统一 | =packages/web= 单 binary 双模式（默认 web / =NOTEZ_MODE=server= headless）；同进程同时挂 SSR + =/api/v1= + =/mcp= | bfa20f1 |
| M2 设计 tokens | =packages/ui/assets/tokens.css= 为 light/dark/system 主题 canonical；web shell 用 link 引用；desktop/mobile assets/main.css 替换为 tokens；web build.rs 复制 public/ 到 target | 4cfb97d |
| M3 客户端骨架 | =packages/ui::Backend= trait + SpaceRow/ResourceRow；=packages/desktop::EmbeddedBackend= 通过 Runtime 打开引擎 + dispatch；desktop Workspace 视图落地 | 4cfb97d |

里程碑对"目标态"（§6）的影响：

- §3.1 中描述的"两个引擎缓存、ApplicationFacade 仍是服务容器"已不再成立；Runtime 是唯一真相来源。
- §4 中描述的"web 仅靠 Dioxus server functions、UI 与 API 没有共享 state"已部分缓解；web 宿主同时承担 API + MCP。
- §7.10 关于"HTTP API 中间件边界、远程对象穿越"已经按本节实现；auth 仍是循环策略（loopback anonymous / public 必带 token 或 OIDC）。
- 仍未完成：wasm 客户端仍显式禁用 hydration（=crates/web/src/main.rs= 的 =hydrate(false)=），desktop 仍是初版 Workspace 视图，mobile 仍是 starter shell。

## 3. 当前实际架构

### 3.1 工作区结构

当前 workspace 包含：

```text
crates/
├── core/                    notez-core 库，仍是大型聚合 crate
├── protocol/                notez-protocol，请求结构和 JSON Schema
├── composition/             native 组合根
├── preview/                 notez-preview 预览器
├── adapters/orgmode/        Org parser adapter
└── adapters/markdown/       Markdown parser adapter

packages/
├── web/                     notez dioxus fullstack SSR + 宿主二进制（双模式 web / headless server）
├── ui/                      共享设计系统 + Backend trait + AppStore + tokens.css
├── desktop/                 Dioxus desktop + EmbeddedBackend（composition Runtime in-process）
└── mobile/                  Dioxus mobile 骨架（HttpBackend 待 M3.5）

已删除的 =packages/api=（旧 echo starter）与 =packages/landing=（静态页）从 workspace 移除；不再以 module 形式被任何 crate 引用。

| `document` | Org/Markdown 解析、链接抽取、工作流 | 解析逻辑已部分纯化，但仍与 core 紧密耦合 |
| `source` | SourceAdapter、Transport、Parser registry、Native/Git/Obsidian adapter、SourceWriter | Source、Transport、Parser、Writer 的边界仍不完全稳定 |
| `storage` | SQLite projection、BlobStore、缓存、SQLite journal/audit adapter | projection、journal、cache 仍集中在同一基础设施模块 |
| `application` | ApplicationFacade、UseCase traits、Dispatcher、Projector、watch、graph、task、artifact orchestration | `ApplicationFacade` 仍承担过多运行时状态和转发职责 |
| `sync` | FolderTransport、ObjectStore、Manifest、HeadsTracker、ThreeWayMerger、SyncEngine | 已引入本地 sync-state 和共同祖先查找，但 head 仍使用合成 snapshot 值 |
| `artifact` | 附件提取、recipe、skill export | 与应用层和本地文件系统仍存在耦合 |

### 3.3 核心数据模型

当前最重要的数据概念是：

```text
Resource
├── ResourceRef       当前 Source 内的稳定引用
├── ObjectIdentity    跨 Source 的对象身份
├── source_id         当前投影来源
├── primary_source_id 正文主来源
├── locator           原始文件或来源内位置
├── revision          内容或来源修订值
└── properties        格式/工作流属性

LinkOccurrence        原始链接出现位置和文本范围
ResolvedRelation      解析后的关系
ConflictRecord        同步冲突记录
Change                 写操作事件
Projection             SQLite 等可重建读模型
```

这个模型的重要优点是：路径和标题不再被直接当成身份，原始文件仍可脱离 Notez 使用。

仍需明确的边界：

- `ResourceRef` 是 Source 内身份还是全局身份？当前两者通过 `ObjectIdentity` 并存，语义容易混淆。
- `revision` 是内容 hash、来源 revision，还是应用写入版本？当前不同路径使用方式不完全一致。
- `properties` 仍是 `BTreeMap<String, String>`，格式特有结构和领域属性尚未分层。
- Relation 的证据、方向、创建者和版本虽然有字段，但不是所有写入路径都经过统一的事件和审计链。

### 3.4 当前写入链路

部分写入已经采用：

```text
Protocol Request
  -> ApplicationDispatcher
  -> UseCase
  -> Projector
  -> EventJournal
  -> ProjectionWrite
  -> AuditLog
```

目前已接入 Projector 的重要路径包括 resource upsert/delete 和扫描中的 projection replacement。其它写路径仍存在直接写 `ProjectionWrite` 的路径，尤其是：

- 任务状态变更；
- 链接解析和诊断写入；
- attachment segment 写入；
- sync conflict 写入；
- 某些 source/writeback 路径。

因此当前不能把“Change journal 是所有状态的唯一脊柱”当成已经完成的事实。它是正在形成的结构。

### 3.5 当前同步链路

当前 folder sync 的数据结构是：

```text
shared-folder/
├── manifests/<logical-path>.json
├── objects/<content-hash>
├── heads/<actor>
└── tombstones/
```

已实现的改善：

- 每个设备通过 `.notez/sync-state.json` 记住每个路径最后已知的 hash；
- push 可以把设备实际已知的内容作为 causal parent；
- pull 会在 incoming parent 中查找本地对象作为共同祖先；
- 不同设备对不同区域的修改可以进行三方合并；
- 无法合并时生成标准冲突标记和 `ConflictRecord`；
- CLI 提供 `sync resolve --path --keep mine|theirs`。

仍存在的结构限制：

- manifest 按 logical path 覆盖，不能表达同一时间多个 actor 的并行 head；
- `HeadsTracker` 仍写入 `snap_<pushed_files>` 一类合成值，不是真正的 content-addressed snapshot；
- 冲突裁决后没有完整地把裁决作为 Change 事件广播给其它设备；
- 当前合并算法以行位置为主，不是基于稳定区块或文本 diff 的真正三方编辑模型；
- sync journal、projection journal 和 source file history 还不是同一个可回放事件流。

---

## 4. 当前 Web 与交互模型

### 4.1 当前 Web 运行时

Web 当前同时包含以下机制：

```text
SSR HTML
  + Dioxus server functions
  + WASM client build
  + hydration bootstrap
  + native HTML forms
  + custom Axum routes
  + progressive-enhancement JavaScript
```

入口位于 `packages/web/src/main.rs`：

- server feature 使用 `dioxus::serve` 和 `dioxus::server::router(app)`；
- 自定义 `/api/sources/*` 路由与 Dioxus router 合并；
- client feature 使用 `LaunchBuilder` / `dioxus::launch`；
- `packages/web/public/index.html` 同时包含 CSS、hydration loader 和原生 JavaScript fallback。

> **已过时** — 当前路由与信息架构以 `docs/ui-refactoring-v2.org`
> 为准（v2 笔记工作台）。下列表格保留作为历史快照：

```text
/                                      space picker
/source/<encoded>                      space home
/source/<encoded>/list                 resource list
/source/<encoded>/resource/<ref>       resource detail
/source/<encoded>/graph                graph view
/source/<encoded>/preview/<locator>   file/attachment preview
```

当前路由（v2）：

```text
/                                          configurable home dashboard
/source/:encoded                           per-space landing (journal/index/files)
/source/:encoded/journal                   daily journal (today + recent)
/source/:encoded/note/:encoded_ref         note workbench (read/edit/source + right rail)
/source/:encoded/files?:..query            all-files browser
/source/:encoded/activity                  journal stream
/source/:encoded/graph                     full-space force graph
/source/:encoded/preview/:encoded_locator  loose-file preview
```

v2 信息架构（来自 `docs/ui-refactoring-v2.org`）：

- Source picker（顶部下拉）、文件树侧栏、活动流合并在主页
  可配置挂件仪表盘上；
- Daily journal（`/journal`）作为一等目的地，与文件树并排展示；
- 笔记工作台的三栏布局：左侧文件树 + 搜索、顶部空间名与新建
  菜单、笔记正文（读/编辑/源模式单标签页切换）+ 右侧大纲/反链/
  属性/局部图（SSR 确定性，固定 prop 传入，不再依赖 hydration
  context 信号）；
- 可配置主页仪表盘由 `~/.config/notez/web.toml` 控制（见
  `ui-refactoring-v2.org` 第 3/8 节）；
- watch start/stop / scan / 命令面板等仍存在，但归属在"配置"
  详情面板内。

v2 已聚焦到"日常笔记 + 日志 + 反链图谱"主流程，下一阶段继续往
wikilink 与块级引用演进。

### 4.3 已知交互问题

#### Dioxus hydration

浏览器控制台已经观察到：

```text
RawInterpreter.hydrate_node
TypeError: Cannot read properties of undefined (reading 'toString')
```

同时出现 server-function payload 反序列化类型不匹配。结果是：

- WASM 文件可以加载；
- 部分 router/history 行为可以工作；
- 元素级 `oninput`、`onchange`、`onkeydown` 不可靠；
- 受控输入可能被重新渲染覆盖；
- 页面退回依赖原生表单和 JavaScript fallback。

当前文档不应把“完整 hydration”描述成已完成能力。

#### 交互机制重复

例如命令面板同时有：

- Dioxus `Signal<bool>` 控制打开状态；
- inline JavaScript 控制 CSS class；
- Dioxus `onkeydown` 处理选择；
- JavaScript fallback 处理搜索和回车；
- server function 负责搜索结果。

这造成同一状态有多个写入者，任何一个机制失效都会出现“视觉打开但交互不工作”或“输入被重置”。

#### 列表过滤

当前过滤逻辑在 Rust 组件中是合理的：

```text
q_filter + kind_filter + sort_key
  -> visible rows
  -> showing count
```

但事件来源不稳定，因此又增加了原生 JS 按 DOM 文本过滤。现在是两个过滤实现并存，而不是一个明确的状态源。

#### 命令面板

命令面板在空间页面上可以搜索资源，但它仍承担过多职责：搜索、分组、键盘选择、路由生成、关闭状态、server function loading。它应当被重设计为统一的 Intent Palette，而不是一个特殊的搜索弹窗。

#### 错误呈现

服务器错误在若干页面被压缩成通用 `Internal`。用户需要知道的是：

```text
找不到 Source
Source 无权限
Source 不支持此能力
文件已变化
同步冲突
服务暂不可用
```

这些状态必须直接进入 UI 状态模型，而不是只剩一段字符串。

### 4.4 当前 UI 组件状态

`packages/ui` 已有：

- `NzButton`；
- `NzButtonGhost`；
- `NzInput`；
- `NzBadge`；
- `NzCard`。

CSS token 已在 Web HTML 中定义，包括：

- 颜色；
- 字体；
- 间距；
- 圆角；
- 阴影；
- light/dark theme。

但 `packages/web` 当前并没有把 `packages/ui` 作为主组件来源，Web 页面仍大量直接书写 RSX 和 class。下一步应先统一组件来源，再继续增加组件。

---

## 5. 用 SICP 重新审视 Notez

SICP 对本项目最有价值的不是某个具体设计模式，而是三条判断标准：

1. 基本元素是什么？
2. 元素如何组合？
3. 组合结果如何被抽象成更高层的语言？

### 5.1 基本元素

建议冻结以下稳定元素：

```text
ObjectId          跨 Source 的知识对象身份
ProjectionRef     某个 Source 中某个投影的位置
Locator           Source 内位置
Span              原始文本证据范围
Relation          有方向、有证据的关系
Command           产生变化的意图
Query             读取状态的意图
Change            已接受的变化
Revision          并发控制版本
Capability        能力和边界声明
Principal/Scope   权限主体与范围
```

`Resource` 不应继续同时充当“原始实体”“投影行”“页面卡片”“写入 DTO”。这些应是不同层的表示。

### 5.2 组合手段

建议明确四种组合：

```text
Object = content identity + source projections
Space  = object references + classification + view policy
View   = Query + layout + interaction policy
Workflow = Commands + state transition rules
```

因此：

- PARA 是关系和保存查询的组合，不是目录树；
- 看板是 Query 的布局投影，不是另一种任务存储；
- 文件树是 Locator 的导航投影，不是知识对象本身；
- 反向链接是 Relation 的反向查询，不是额外关系副本。

### 5.3 抽象手段

当前仍有多个“看起来相同但其实不同”的抽象：

- `Resource` 与 `ResourceRow`；
- `SourceConfig` 与 `SourceInstanceConfig`；
- CLI args、MCP args、protocol request；
- server function result、HTTP response、页面 view model；
- SQLite projection 与 sync object store。

建议的边界是：

```text
Domain types       不知道 JSON、HTML、SQLite、clap
Protocol DTOs      只表达跨边界消息
Application        解释 Command/Query，依赖 ports
Infrastructure     实现 ports
View models        只为某个界面优化，不回流到 domain
```

### 5.4 流

SICP 的 stream 思维适合 Notez，但必须明确“哪个流是真流”：

```text
User/Agent intent
  -> accepted Change stream
  -> projection folds
  -> sync delivery stream
  -> UI state stream
```

原始文件不是 Change stream 的替代品。文件是外部事实来源；扫描文件变化时也应产生一个可观察的 `SourceObservation` 或 `ImportChange`，而不是只重建 SQLite。

---

## 6. 建议目标架构

### 6.1 目标分层

```text
┌─────────────────────────────────────────────────────────────┐
│ Surface                                                      │
│ Web / Desktop / Mobile / CLI / MCP / HTTP                   │
│ 参数翻译、会话、呈现、平台能力                               │
└──────────────────────────────┬──────────────────────────────┘
                               │ Protocol Request / Response
┌──────────────────────────────▼──────────────────────────────┐
│ Application Engine                                            │
│ Dispatcher / Authorization / Use Case / Projector / Audit    │
└───────────────┬─────────────────────────────┬────────────────┘
                │ domain                       │ ports
┌───────────────▼──────────────┐  ┌───────────▼────────────────┐
│ Domain                         │  │ Ports                      │
│ Object / Relation / Revision   │  │ SourceReader / SourceWriter │
│ Command / Query / Change       │  │ ProjectionReader / Projector│
│ Selector algebra / merge       │  │ Journal / Blob / Clock      │
│ pure wasm-compatible           │  │ Watcher / Authorizer        │
└───────────────────────────────┘  └───────────┬────────────────┘
                                               │
┌──────────────────────────────────────────────▼───────────────┐
│ Infrastructure                                                 │
│ SQLite / filesystem / Org / Markdown / notify / folder sync    │
└───────────────────────────────────────────────────────────────┘
```

### 6.2 推荐 crate 目标态

```text
notez-domain       纯领域模型和算法，wasm-compatible
notez-protocol     跨表面 Request/Response、错误和 schema
notez-engine       Dispatcher、UseCases、Projector、审计
notez-storage      SQLite projection、journal、迁移
notez-fs            文件读写、blob、watcher
notez-format-org   Org parser + surgical writer
notez-format-md    Markdown parser + surgical writer
notez-preview      preview builders
notez-sync         基于 Change/Object/Head 的同步
notez-composition  唯一 native 装配根
notez-cli          CLI adapter
notez-mcp          MCP adapter
notez-web          HTTP/server-function adapter + client
notez-ui           共享设计系统和 View primitives
```

不必一次完成 Cargo 拆分。当前 `notez-core` 可以先作为过渡实现，但模块依赖必须按目标边界收紧。

### 6.3 协议模型

建议让 protocol 拥有明确的请求和响应类型，而不是目前“请求在 protocol、响应仍在 core”的半拆状态：

```rust
pub enum Request {
    QueryResources(QueryResourcesRequest),
    ReadResource(ReadResourceRequest),
    TransitionTask(TransitionTaskRequest),
    Scan(ScanRequest),
    Sync(SyncRequest),
}

pub enum Response {
    QueryResources(QueryResourcesResponse),
    Resource(ResourceView),
    Transition(TaskTransitionResponse),
    Scan(ScanResponse),
    Sync(SyncResponse),
}
```

CLI、MCP、HTTP、Web 都只负责：

```text
native input -> protocol request -> dispatcher -> protocol response -> native output
```

协议应使用稳定的 wire DTO，而不直接暴露 SQLite 行结构或 HTML 字符串。

### 6.4 单一写入管道

所有写操作必须遵守：

```text
Command
  -> authorization
  -> expected revision check
  -> domain validation
  -> Change construction
  -> append journal
  -> apply projector
  -> source writeback if required
  -> audit outcome
  -> invalidate affected queries
```

其中 source writeback 与 projection 更新需要有明确的事务语义：

- 文件写回失败：projection 不应假装成功；
- projection 失败：journal 中保留失败/待重放状态；
- 冲突：生成可见的 Conflict object，而不是只写 marker；
- 审计：拒绝、成功、冲突、重试都必须有记录。

### 6.5 SourceWriter

`TextPatch` 是正确方向，但目标接口应更加明确：

```text
SourceSnapshot {
  source_id
  locator
  revision
  bytes
}

Patch {
  span
  expected_text
  replacement_text
}

SourceWriter.apply(snapshot, patches)
  -> Applied { new_revision }
  | Stale { current_revision }
  | Unsupported
```

Org 和 Markdown writer 各自负责格式语义；engine 不拼接格式字符串。

---

## 7. 界面与交互重新设计

### 7.1 从“文件浏览器”转向“工作台”

当前 UI 首先展示文件、资源和投影。目标 UI 应首先回答：

1. 我现在在哪个 Space？
2. 我正在处理什么？
3. 这个对象的上下文是什么？
4. 我下一步能做什么？
5. 当前修改是否安全、是否同步、是否冲突？

目标布局：

```text
┌──────────┬───────────────────────────────┬───────────────┐
│ Context  │ Work surface                  │ Inspector     │
│          │                               │               │
│ Spaces   │ Inbox / Search / Reader /     │ Properties    │
│ Sources  │ Editor / Board / Graph        │ Backlinks     │
│ Inbox    │                               │ Activity      │
│ Saved    │                               │ Sync state    │
│ Queries  │                               │ Audit         │
└──────────┴───────────────────────────────┴───────────────┘
```

左栏不是单纯目录树，而是“上下文选择器”；中间区根据用户意图选择一种工作表面；右栏是对象检查器。

### 7.2 核心导航

建议固定四个一级入口：

```text
Inbox       尚未归档或待处理内容
Search      全局搜索和查询
Today       日程、截止日期、近期变化
Space       当前知识空间和来源
```

PARA、Graph、Board、Files 都作为 Space 内的 View，而不是一级产品概念。

### 7.3 对象详情

对象详情应按“阅读优先”组织：

```text
标题与状态
正文/块内容

来源证据
- Source
- 文件路径
- revision
- 原始位置

关系
- backlinks
- outgoing links
- related code

活动
- 最近 Change
- 同步状态
- conflict / audit
```

不要默认展示所有内部字段。`object_id`、`primary_source_id` 等应在 inspector 或 debug view 中显示。

### 7.4 编辑体验

编辑器必须区分三种模式：

1. **Reader**：安全阅读，默认模式；
2. **Block editor**：编辑标题、属性、任务状态和结构化块；
3. **Source editor**：显示原始 Org/Markdown 文本，保留完整文件主权。

所有结构化操作都生成 Span Patch；用户明确进入 Source editor 时才允许直接编辑原文。

### 7.5 命令面板

命令面板应从“搜索弹窗”升级成统一 Intent Palette：

```text
输入：sync

Search resources
Open saved query
Create note
Move to Space
Change task state
Show backlinks
Run authorized workflow
Open source file
```

搜索和命令使用同一套 `Command/Query` 协议。键盘选择只改变 palette state，Enter 只发出一个明确的 Intent。

### 7.6 交互状态机

每个页面不应自行持有互相独立的状态。建议一个页面会话 Store：

```text
AppStore
├── active_space
├── route
├── query
├── selection
├── resource_cache
├── pending_commands
├── notifications
├── sync_status
└── error_state
```

数据流：

```text
User event
  -> Action
  -> Reducer: local state transition
  -> Effect: protocol request
  -> Response
  -> Reducer
  -> View
```

原生 JS fallback 只能作为没有 WASM 的降级层，不能与 Store 同时成为第二个状态源。

### 7.7 错误和反馈

统一 Toast/Inline status/Conflict panel：

| 状态 | UI 表达 |
|---|---|
| Loading | skeleton 或明确 loading label |
| Empty | 下一步动作，而不是空白区域 |
| NotFound | 对象可能已删除/移动的说明 |
| ReadOnly | 明确说明来源能力和替代动作 |
| RevisionConflict | 展示本地/当前版本和重新加载动作 |
| SyncConflict | 展示双方版本、保留对象、裁决入口 |
| Unauthorized | Principal、Source、Action、Scope |
| Internal | 用户信息 + 可复制诊断 ID |

---

### 7.8 文档驱动的自定义卡片

当前首页的 `WIDGET_IDS`、`web.toml` 和 `HomePage` 分支只能配置固定组件，不能表达用户自己的查询、参数、远程对象或输出格式。因此“卡片”不应继续作为 Web 组件集合扩展，而应成为一种文档声明和 View projection。

#### 稳定模型

```text
CardDefinition
├── id                  文档内稳定 ID
├── title               展示标题
├── placement           dashboard / note / query
├── source_scope        当前 Source 或显式 SourceRef
├── input_schema        参数名、类型、默认值、是否必填
├── program             Janet 代码和代码 hash
├── output_schema       json / html / object / list
├── layout              order / size / visibility
└── policy              required capability、cache、refresh

CardExecution
├── principal           OIDC subject / local principal
├── source_revision     输入数据版本
├── params              用户或页面参数
├── objects             Engine 预取的结构化对象
└── trace_id

CardResult
├── card_id
├── state               ready / empty / denied / failed / stale
├── output_type         json / sanitized_html / object / list
├── value               typed JSON payload
├── object_refs         可继续交给 Protocol 的远程或本地对象引用
├── generated_at
└── cache_key
```

卡片的事实来源应是 Source 中的 Markdown/Org 文档，首页只是选择并布局这些定义的投影。首页可以保留一个 dashboard 配置文件，但它只能保存卡片顺序、布局和可见性，不能保存查询逻辑或第二套 widget 类型。旧的固定卡片作为内置 `CardDefinition` 迁移，最终删除 `WIDGET_IDS` 分支。

#### Janet code block wire contract

Markdown 使用 fenced block，Org 使用 `src` block；语言名必须是 `janet`，属性位于 fence/header 中：

````markdown
```janet card="inbox" output="list" refresh="60s"
{:input {:limit {:type "integer" :default 20}}
 :run (fn [ctx]
        {:type "list"
         :items (notez/query ctx {:kind "document" :limit (:limit (:params ctx)))})}
 :output {:type "list"}}
```
````

实现上不依赖 Janet 返回任意宿主对象，而要求执行结果符合明确的 envelope：

```janet
{:type "json" :value ...}
{:type "html" :value "<p>..."}
{:type "object" :ref "notez://object/..."}
{:type "list" :items [...]}
```

约束如下：

1. `input_schema` 和 `output_schema` 在执行前校验；缺失字段、未知输出类型和超限结果直接产生结构化错误。
2. `ctx` 只包含经过权限检查的 `params`、结构化 `objects`、Source 元数据和只读 Protocol 查询函数。Janet 不访问文件系统、网络、数据库、进程或 SourceWriter。
3. `html` 必须经过 HTML sanitizer；默认推荐 `json`/`list`，不能把 Janet 字符串自动当作 HTML。
4. `object` 只能引用已经由 Source Adapter/Engine 解析的对象；远程对象仍使用 `ResourceRef`/`ObjectIdentity`，不能在 Janet 中伪造身份。
5. 执行缓存必须至少绑定 `card_id + code_hash + source_revision + params + principal/policy`。含权限数据的卡片不能共享未经 principal 隔离的缓存。
6. 执行失败不会破坏正文渲染，`CardResult.state` 必须区分 `denied`、`stale`、`failed` 和 `empty`，供所有客户端统一呈现。

这会把当前 `render_janet_blocks()` 的职责拆成三个端口：`BlockParser` 只解析声明，`CardExecutor` 只执行受限程序，`CardRenderer` 只渲染经过验证的 `CardResult`。正文预览和首页卡片共享前两个端口，但使用不同的布局 projection。

### 7.9 Source 级忽略与文件事实边界

当前 native scanner 只按 `.md`/`.org` 过滤，并在部分路径上跳过隐藏目录；这不足以处理源码仓库、构建产物、依赖目录和用户明确不希望进入 Notez 的文件。忽略规则必须属于 Source policy，而不是散落在 scanner、tree、preview 和 UI 中。

建议的 v2 配置：

```toml
[scan]
include = ["**/*.md", "**/*.markdown", "**/*.org"]
exclude = [
  ".git/**",
  ".notez/**",
  "target/**",
  "node_modules/**",
  "vendor/**",
  "dist/**",
  "build/**",
  "**/*.rs",
  "**/*.ts",
  "**/*.tsx",
  "**/*.js",
]
follow_symlinks = false
max_file_size = 1048576
```

规则语义：

- `include` 决定哪些文件有资格成为可索引文档；
- `exclude` 在 include 之后、读取文件之前应用，使用 source-relative glob；
- `.notez`、`.git`、隐藏路径、符号链接循环和超出大小限制的文件默认拒绝；
- include/exclude 只影响该 Source，不影响其它 Source，也不改变用户原始文件；
- 文件浏览、搜索候选、主页 index 候选、附件预览和 Janet 上下文必须调用同一个 `SourcePolicy::allows(locator, metadata)`；
- 明确的 include 优先级高于默认 exclude，但不得绕过安全目录、符号链接和大小限制；
- 每次扫描报告忽略原因计数，方便用户判断“文件没有出现”是规则结果还是解析失败。

配置模型应扩展 `SourceConfig`，并由 composition root 传入 `Engine`/`NativeSourceAdapter`。不能继续在 `scan_native()` 中重新构造一个丢失 `include_paths`/`exclude_paths` 的临时配置；否则配置存在但不会影响扫描。未来 Source Adapter 都实现同一个 policy 端口，远程 Source 则把远程能力声明为 policy/capability，不伪装成本地文件。

### 7.10 远程对象、API 与中间件边界

远程 Notez server 是一个 Protocol endpoint，不是 Web 页面接口。调用链固定为：

```text
HTTP request
  -> transport middleware
  -> principal + trace + source policy
  -> deserialize Request
  -> authorization by operation/capability
  -> Engine / Dispatcher
  -> typed Response or typed Error
```

中间件顺序建议固定为：request-id/access log、认证、授权、Source policy、限流/超时、dispatcher。Authentik 只负责 OIDC 身份认证和 claims；Notez 自己根据 `sub`、group/scope、Source capability 和操作类型做授权。不能把“JWT 有效”直接等同于“可以读写所有 Source”。

远程对象只通过 `ResourceRef`、`ObjectIdentity` 和 typed `Response` 穿越边界。Card/Janet 可以请求远程对象，但必须由 Engine 先完成 Source Adapter、认证和授权；Janet 永远不能直接向远程 URL 发请求。远程对象不可用时返回 `Unavailable`/`stale`，不能伪造空列表掩盖上游故障。

### 7.11 迁移验收

自定义卡片完成的判断不是“页面能显示一张卡”，而是以下性质同时成立：

1. 同一个 `CardDefinition` 可由首页、笔记正文和远程客户端请求；
2. CLI/Web/MCP/HTTP 对同一 Card/Query 产生相同的 typed `CardResult`；
3. 卡片输入、输出、权限、缓存和失败状态可被 `/schema` 与 Inspector 查询；
4. 修改 Source ignore policy 后，扫描、搜索、文件树和 Janet 输入的结果一致；
5. 删除或移动文档后，卡片显示 `stale`/`empty`，而不是旧缓存或伪造结果；
6. 未认证、无 Source capability、危险 Janet API 和未 sanitizer 的 HTML 都在执行前被拒绝。

---

## 8. 迁移顺序

### 阶段 A：冻结事实与协议

- / 把当前真实架构与目标架构分开维护（本文档 + =docs/architecture.org=）；
- ✅ 删除过时 README 和路由描述（README 描述更新到 M1-M3；ui-refactoring-v1 仍作参考）；
- ✅ 给 protocol 补齐 typed Response（=crates/protocol/src/response.rs= 已为 =ResourcePage=、=Scan=、=AttachmentRef=、=Conflicts= 等变体生成 JSON Schema）；
- / 为所有 server function 建立请求/响应契约测试（web 页面 =#[server]= 还未迁到 =ui::Backend= trait，部分用例通过 MCP/HTTP 间接覆盖）；
- ✅ 明确 =ResourceRef=、=ObjectIdentity=、=revision= 的语义（文档 §3.3）。

验收：CLI、MCP、HTTP 对同一 Request 产生相同领域结果。

### 阶段 B：完成写入脊柱

- 所有写路径接入 Projector；
- journal 记录成功、失败、冲突和重试；
- source writeback 与 projection 更新建立事务状态；
- audit 成为正式查询模型；
- 删除所有绕过 dispatcher 的公共写入口。

验收：每一次写入都可按 Change id 回放和解释。

### 阶段 C：同步模型收敛

- 用 content-addressed snapshot 替代 `snap_<count>`；
- 支持多个 actor heads，不覆盖并行历史；
- manifest 改为 snapshot graph，而不是 path 的最后一行 JSON；
- 合并和裁决都生成 Change；
- 在 UI 中呈现同步状态和冲突对象。

验收：离线分叉、合并、裁决、再次同步后，两个设备最终收敛且历史可解释。

### 阶段 D：Web 客户端状态模型

- ✅ SSR 首屏保留（web binary 默认走 dioxus::serve）；
- / 让 Web client 只有一个 Store；
- / server function 只作为 protocol transport；
- / hydration 失败时不再产生第二套 Dioxus/JS 状态逻辑（hydration 当前显式 =hydrate(false)=，待修复）；
- ✅ 将 progressive enhancement 限定为可替换的 transport fallback；
- / 优先完成 Reader、Search、Inspector 三个工作面。

验收：

- 页面加载后无 hydration console error；
- 输入、选择、键盘导航不丢状态；
- SPA 路由与浏览器前进后退一致；
- 查询状态可从 URL 恢复；
- server function 错误不会被错误地反序列化成其它 DTO。

### 阶段 E：设计系统和跨平台
- ✅ 将 tokens 从 Web HTML 提取到共享 UI 资源（=packages/ui/assets/tokens.css=，web shell 用 link 引用，desktop/mobile assets/main.css 替换为 tokens）；
- / Web 实际使用 =packages/ui= 组件（Nz* 组件已被 web 页面采纳中）；
- ✅ desktop/mobile 复用同一 View model 和 action（=ui::Backend= trait + EmbeddedBackend 实现；mobile HttpBackend 待 M3.5）；
- ✅ 删除 desktop/mobile 的 starter blog surface（已替换为带 tokens + Nz 组件的工作台）。

验收：相同的 Reader、Search、Inspector 在 Web/Desktop/Mobile 使用相同交互契约。

---

## 9. 必须避免的设计

1. 不再把 `ApplicationFacade` 当成所有服务的容器；
2. 不再让 UI DTO 反向构造或伪造领域身份；
3. 不再让 SQLite 同时充当事实来源、事件来源和缓存；
4. 不再用多个名字表示同一个 Use Case；
5. 不再让 server function、Axum route、inline JS 各自拥有同一状态；
6. 不再通过增加默认空实现来让测试替身“通过编译”；
7. 不再把 Graph、Tree、Board 当成独立数据模型；
8. 不再用全量文件覆盖代替格式感知的 Span Patch；
9. 不再把“构建成功”当成 Web 交互完成；
10. 不再把计划文档中的目标态写成当前已实现能力。

---

## 10. 最终判断

当前 Notez 的后端方向仍是可保留的：本地文件主权、稳定身份、Source adapter、可重建投影、Change/Journal、三方合并和协议统一都值得继续。

2026-09 里程碑（M1–M3）已把 §1 列出的三个核心问题的前两个推进到「已落地」：

- ✅ 「事实、事件、投影形成单一状态模型」 — =composition::Runtime= 接管引擎缓存与 watcher；CLI/MCP/HTTP/Web 复用同一 Runtime；protocol 已是 typed enum。
- ◐ 「客户端运行模型不稳定」 — web 宿主双模式落地（SSR + axum API/MCP），但 wasm 客户端仍禁用 hydration，desktop 是初版 Workspace 视图，mobile 仍是 starter shell。
- ◐ 「产品信息架构以资源列表为中心」 — 共享 Nz* 组件与 =ui::Backend= trait 落地，desktop Workspace 是首批非文件浏览器的工作面。

需要继续重考虑的：

- **核心状态模型**：从多个局部状态改为 Command/Change/Projection 单一脊柱（M0.5/0.6 未变）；
- **应用边界**：从大型 Facade 改为显式 Dispatcher、UseCase 和 Ports（API/Web 已委托，cli/handlers 仍存面模型）；
- **客户端模型**：从 SSR + hydration + JS 补丁的混合状态，改为 SSR 首屏 + 单一 Store + 协议 transport（M3.5/mobile + wasm 客户端待 M4）；
- **信息架构**：从文件/资源浏览器改为围绕 Inbox、Search、Reader、Editor、Inspector 的知识工作台（v2 UI 文档已定，落地进行中）；
- **设计系统**：从 Web 内嵌 CSS 改为 tokens 驱动、跨平台复用的真正 UI 层（tokens 已抽出 + 共享，三端 link 同一 css，Nz 组件采纳进行中）。

最优先的下一步不是继续增加功能，而是完成 **web =#[server]= 函数迁移到 =ui::Backend= trait** + **mobile HttpBackend 落地** + **wasm 客户端 hydration 修复**。在此基础上，新的交互和 UI 才不会继续建立在不稳定的运行时之上。
