# Notez 个人知识工作台重设计

**状态：待用户审阅**  
**日期：2026-09-09**

## 1. 决策与边界

Notez 定位为本地优先的个人知识管理系统。原始文件是事实来源；SQLite、检索索引、关系、Journal、视图都是可重建投影。用户可通过 CLI、MCP、Web、桌面端、移动端使用同一领域能力；移动端可连接远程 Notez 实例。

本设计冻结首期为“本地文本编辑闭环”，不把全部产品愿景混入一个实施批次。

首期包含：

- 本地 Space 的 Markdown、Org、TXT 阅读、源编辑、实时预览；
- PDF、DOCX、PPTX 和其它附件的安全预览与下载，不提供 Office 编辑；
- Space、Source、目录、文档、附件的统一对象导航；
- `index.org` 作为目录对象正文；
- `project`、`area`、`resource`、`archive` 等 PARA 分类；
- 双链索引和右栏关系呈现；
- Journal、活动、revision 与同步状态的可见投影；
- Org / Markdown 中受限的 `notez` 展示块；
- 远程实例只读访问。

首期明确不包含：结构化 block editor、Span Patch 写回、远程受控写入、JavaScript 执行 sandbox、DOCX/PDF/PPTX 编辑、白板编辑、多 actor 同步收敛。这些能力依次依赖稳定的对象/写入/授权/同步基础。

## 2. 核心模型

采用 **Object—Projection—View** 分层。

```text
Object
├── object_id             跨 Source 的稳定身份
├── kind                  directory | document | attachment | block
├── metadata              标题、标签、PARA 分类、用户属性
├── relations             有证据的正向/反向关系
└── primary_projection    可写事实来源；首期是本地文件

Projection
├── source_id
├── locator               Source 内位置
├── revision              原始字节内容 hash / 来源版本
├── capabilities          read | write | preview | execute-query
└── source-specific facts 格式、MIME、文件 stat、解析结果

View
├── ObjectTree            Space / Source / Directory / Document / Attachment
├── ReaderView            阅读或附件预览
├── SourceEditorView      文件原文编辑
├── InspectorView         属性、关系、活动、冲突、同步
└── QueryView             PARA、搜索、`notez` 块结果
```

### 2.1 目录、`index.org` 与 PARA

每个目录均映射为 `Directory` Object；目录不因没有说明文档而失去身份。

- 目录内存在 `index.org` 时，文件是该目录对象的正文 Projection，不额外创建第二个“index 文档对象”。
- `index.org` 的标题、属性、正文描述目录对象；例如 `#+property: notez_kind project`、标签、状态、联系人、时间范围。
- 无 `index.org` 时，目录仍出现在对象树；标题退回目录名，属性为空，分类为 `unmanaged`。
- 子目录、文档、附件是独立 Object，并以 `contains` relation 连接到目录 Object。
- PARA 是目录或文档 Object 的 `classification` 属性，不是另一套目录树：`project | area | resource | archive | unmanaged`。
- “Projects / Areas / Resources / Archives”是保存查询 View；文件树只是 locator 导航 View。

这避免“路径既是身份又是分类”的混乱，也使同一个 Object 在未来可被多个 Space 引用而不复制正文。

### 2.2 Space 与 Source

- **Space**：用户工作的顶层容器，保存 Object 引用、默认 Source、导航/查询/视图策略和授权范围；不拥有正文。
- **Source**：事实来源适配器；首期为 NativeLocalSource，后续可为 RemoteNotez、Notion、Anytype 等。
- 一个 Space 可配置多个 Source；一个 Source 可为多个 Space 提供投影。
- 左栏按 `Space → Source → ObjectTree` 统一呈现，不把“Space 切换”和“Source 文件树”做成两套主导航。
- 远程 Source 首期声明 `read | preview | execute-query`；写能力必须等多维授权、revision 与审计统一后才开放。

## 3. 命令、事件与写入

所有 surface 均只翻译原生输入，不直接操作文件、SQLite 或 Engine 内部类型。

```text
CLI / MCP / HTTP / Web / Desktop / Mobile
  → protocol Request
  → Dispatcher
  → authorization + revision precondition + domain validation
  → Change append
  → source writeback
  → projection update
  → audit outcome
  → typed Response
```

### 3.1 不变量

1. 每个可写请求必须携带显式 `RevisionPrecondition`：`MustMatch { revision }` 用于更新、删除、状态迁移和写回；`MustNotExist` 用于创建。二者都不是隐式覆盖；空 revision、缺失前置条件和强制覆盖都是 `InvalidRequest`。
2. 写回成功之前，projection 不能报告成功；projection 失败后，journal 保留可恢复状态。
3. `Change` 和 audit 都记录 principal、目标、revision precondition、结果及 trace id。
4. 所有 surface 对相同 Request 返回同一 typed Response / Error；不允许 Web 专有写入绕过 protocol。
5. 每个进程只有一个 `composition::native::Runtime`：一个 Engine cache 和一组 watcher，供 web/API/MCP/host supervisor 共用。
6. Journal 的 accepted Change 是唯一领域写入记录；扫描外部文件变化产生 `SourceObservation` / `ImportChange`，不能只清表重扫而没有解释。

### 3.2 首期核心命令

- `ReadObject`、`QueryObjects`、`ReadDirectory`；
- `CreateDocument`：在 Directory Object 下创建文件并建立 contains relation，使用 `MustNotExist`；
- `UpdateDocument`：仅 `.md` / `.org` / `.txt`，全文件源编辑，使用 `MustMatch { revision }`；
- `SetClassification`：更新 `project/area/resource/archive/unmanaged`，使用 `MustMatch { revision }`；
- `ScanSource`、`ListRelations`、`ListActivity`、`ListSyncStatus`；
- `ExecuteDisplayBlock`：只读运行 `notez` block。

## 4. 工作台与交互

首期采用三栏工作台，视觉以 SilverBullet 的紧凑、阅读优先、键盘友好为方向。

```text
┌─────────────────────┬──────────────────────────────┬──────────────────────┐
│ Context             │ Work surface                 │ Inspector            │
│                     │                              │                      │
│ Space               │ Reader / Source Editor       │ Object properties    │
│ └ Source            │ Markdown / Org / TXT         │ PARA classification  │
│   └ Directory tree  │ Attachment Preview            │ Outgoing / backlinks │
│ PARA saved queries  │ `notez` block preview         │ Revision / activity  │
│ Search              │                              │ Sync / conflict      │
└─────────────────────┴──────────────────────────────┴──────────────────────┘
```

### 4.1 左栏：统一 Context

- Space 是顶层选择；其下列出 Source 与同步/只读状态。
- Directory、Document、Attachment 一律使用 ObjectTree，不另造“文件树”和“知识树”。
- PARA、双链、最近活动、搜索是保存查询入口，结果仍指向同一 Object。
- 小屏幕：左栏变为抽屉；Inspector 从右栏变为可关闭抽屉；中栏保持唯一主工作面。

### 4.2 中栏：Reader 与 Source Editor

- 默认 Reader；显式 Edit 进入 Source Editor。
- `.md` / `.org` / `.txt`：textarea/代码编辑器展示原文，250ms debounce 的实时预览复用 Reader 渲染器。
- Web 保存统一使用原生表单提交到 protocol 路由；失败保留用户文本，revision conflict 显示当前 revision 与“重新加载 / 显式覆盖”动作。
- `.pdf`、`.docx`、`.pptx`、图片、表格、归档：按 MIME/extension 选择 Previewer；首期只读。
- 后续编辑器注册表按 `extension → EditorCapability` 增长，不能在页面里散落 if/else。

### 4.3 右栏：Inspector

同一对象的属性、来源、revision、PARA 分类、双链、活动、同步、冲突都在一个 Inspector 中呈现。内部 ObjectId 与 adapter 参数只在 debug 展开项出现。

## 5. `notez` 展示块

Org 与 Markdown 可以包含 `notez` 代码块。代码块是文档正文的一部分：编辑代码块即编辑文档；执行只是预览，不是独立的写入口。

Markdown：

````text
```notez id="open-projects" output="objects"
{:query {:classification "project" :state "open"}
 :render {:type "objects"}}
```
````

Org：

```text
#+begin_src notez :id open-projects :output objects
{:query {:classification "project" :state "open"}
 :render {:type "objects"}}
#+end_src
```

### 5.1 执行环境

- 首期解释器为 Janet；JavaScript 仅作为未来独立 sandbox 的预留语言名，不执行。
- `ctx` 仅暴露：当前 `space`、可读 `source` 元数据、当前 object、参数、typed read-only Query API。
- 不提供文件系统、网络、进程、数据库、动态加载、SourceWriter、写请求或 host object。
- 执行配置绑定 `principal + source capabilities + code_hash + input revisions + params`；超时与结果大小均有硬限制。
- HTML 结果必须 sanitizer 后才进入 `dangerous_inner_html`；执行失败只显示结构化状态，不破坏正文阅读。

### 5.2 唯一输出 envelope

```clojure
{:type "objects" :items ["document:01..." "directory:01..."]}
{:type "html"    :value "<section>...</section>"}
{:type "text"    :value "..."}
```

- `objects` 只允许引用 Dispatcher/Source 已解析的 ObjectRef；UI 将它们渲染成可导航对象列表。
- `html` 必经 sanitizer，禁止 script、事件属性和远程不受控资源。
- `text` 作为等宽文本预览；首期不支持任意 JSON、组件或写命令。

## 6. Surface 边界

| Surface | 首期职责 | 禁止事项 |
|---|---|---|
| CLI | 读、扫描、源编辑、查询、诊断；以人类文本或 JSON 呈现 typed Response | 直接写文件或 SQLite |
| MCP | protocol 到 tool schema 的翻译；Agent 读、查询、受控本地编辑 | 手写第二套 DTO / 绕过 Dispatcher |
| HTTP | typed Request/Response、认证、trace、限流 | 把 HTML 页面当 API |
| Web | 三栏工作台、Reader/Editor/Inspector、预览 | 直接 fs 写入、第二份客户端状态源 |
| Desktop | 本地 EmbeddedBackend，复用同一 ViewModel | 自建 Engine cache |
| Mobile | Remote HTTP Backend，远程只读 | 假装可本地读写 |

## 7. 实施切片与验收

### Slice 0：写入与 Runtime 正确性

- host supervisor 使用单一 Runtime；
- protocol / dispatcher / CLI 强制 revision；
- Change/audit 对成功、拒绝、冲突、失败可查询；
- 删除或标记 wire 已暴露但永远 Unsupported 的 dashboard update stub。

验收：两个并发 CLI 写同一 revision，只有一个成功；host 内 Web/API/MCP 看到同一 watcher/Engine 状态。

### Slice 1：ObjectTree 与目录对象

- 加 Directory Object、contains relation、`index.org` 正文绑定；
- 统一 Space / Source / ObjectTree query；
- PARA 属性与保存查询；
- 现有文件树迁移为 locator navigation projection。

验收：无 `index.org` 的目录仍可被导航和分类；创建 `index.org` 不生成重复 Object；同一对象在树、PARA、双链视图中 identity 一致。

### Slice 2：本地文本工作台

- 三栏 Web shell；
- Reader / Source Editor / Inspector；
- Markdown / Org / TXT 实时预览和 conflict-safe save；
- 附件 Previewer registry；
- desktop/mobile 共用 typed ResourceKind 和 Backend read contract。

验收：编辑 Markdown、Org、TXT 能预览、保存、冲突恢复；PDF/DOCX/PPTX 可预览或安全下载；Space 与多个 Source 在同一左栏显示。

### Slice 3：`notez` 展示块

- Org/Markdown block parser；
- Janet 只读 runtime + typed query context；
- objects/html/text envelope、sanitizer、cache key、timeout；
- Reader 与 Editor preview 共用 BlockRenderer。

验收：一个 PARA 查询块展示 Object 列表；一个 HTML block 被 sanitizer；非法能力、超时、未知 envelope 返回结构化块错误，不影响正文。

### 后续切片

1. 真实 content-addressed snapshot、多 actor heads、可回放 sync；
2. block editor、AST/span patch 与格式感知 SourceWriter；
3. Principal × Space × Source × Action × Scope 授权和远程受控写入；
4. JavaScript sandbox、DOCX/PDF/PPTX 编辑、白板；
5. 完整搜索、引用、Graph、跨实例对象 federation。

## 8. 不做的设计

- 不让 SQLite 成为正文事实；
- 不让路径、标题、View Row 充当稳定 Object 身份；
- 不以目录树取代 PARA 或双链模型；
- 不让 Janet/JS 直接写文件、SQLite、网络或进程；
- 不让 server function、Axum route、inline JS 同时持有编辑状态；
- 不为 desktop/mobile 单独创造 DTO、ResourceKind 或写路径；
- 不在同步真实化前承诺多设备冲突自动收敛。
