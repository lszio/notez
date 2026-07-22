# Notez 系统设计

日期：2026-07-22  
状态：已确认  
技术方向：Rust 模块化单内核

## 1. 目标与原则

Notez 是一个本地优先、以 Org-mode 为主要行为语言的知识联邦系统。它管理原生 Org/Markdown 文件，挂载 Git 目录、Obsidian Vault 和未来的 Anytype API 等外部来源，建立跨来源知识图，并通过 CLI、MCP、摘要及 Skill 为人和 Agent 提供统一访问。

核心原则：

1. 原生空间中，Org/Markdown 文件是唯一事实来源；数据库、图、嵌入、摘要和 Skill 均为可重建投影。
2. Org-mode 是一级语义，不只是文本格式。TODO、属性、规划时间、日志、clock、agenda 和归档均进入领域模型。
3. 所有能力建立在少量正交的资源代数之上；领域接口是可解释、可组合的高层抽象。
4. 外部来源采用联邦挂载，默认只读。一次性导入与未来按来源启用的写回是不同能力。
5. 单用户、多设备、离线优先；第一版使用文件夹传输，同步协议不绑定具体 Transport。
6. 第一版不提供图形编辑器，但 CLI 与 MCP 支持完整、安全、可审计的原生空间读写。

## 2. 边界与非目标

第一版包含：原生 Org/Markdown、文档及 heading 双链、类型与规则、Task/PARA、CLI/MCP、Git/Obsidian 只读挂载、附件提取、社区、摘要、Skill、文件夹同步和可重建索引。

第一版不包含：多人协作、P2P/中继同步、外部来源写回、任意段落级链接、图形编辑器、音视频转写、完整 Anytype 适配器和复杂自动知识抽取。

## 3. 总体架构

采用 Rust 模块化单内核，发布为一个 `notez` 可执行文件：

```text
Content Sources
  Native | Git Folder | Obsidian Vault | Anytype API (future)
                         ↓ SourceAdapter
Notez Core
  Lossless Parser | Knowledge Model | Application Services
                         ↓ domain events
Derived Capabilities
  Graph | Search | Attachment | Community | Artifact
                         ↓
Interfaces
  CLI | MCP | SyncTransport
```

采用 Cargo workspace。所有包统一放在 `crates/` 下，目录名不添加 `notez-` 前缀：

```text
crates/
├── domain/       Resource 代数、类型、规则、关系
├── document/     Org/Markdown 保真语法树
├── application/  resolve/query/mutate/relate/derive/execute/inspect
├── storage/      SQLite、Blob、审计、任务
├── source/       Native/Git/Obsidian adapters
├── sync/         manifest、object、merge、transport
├── artifact/     extraction、community、Skill IR
├── cli/          人类与脚本入口
└── mcp/          Agent 入口
```

这些是包和库边界，不是微服务。Cargo package 名可使用简短目录名；若发布到公共 registry 时发生命名冲突，再仅调整 package 名，不改变目录结构。CLI 与 MCP 不直接访问文件或数据库，必须调用同一应用服务。最终仍发布为一个名为 `notez` 的可执行文件。

## 4. 统一知识模型

```text
Space
 ├─ Source
 │   └─ Resource
 │       ├─ Document
 │       ├─ Heading
 │       ├─ Attachment
 │       └─ ExternalObject
 ├─ Relation
 ├─ Community
 └─ Artifact
```

- `Space`：同步、配置和访问边界。
- `Source`：内容来源及能力声明。
- `Resource`：统一可寻址对象，包含稳定 `ref`、主 `TYPE`、traits、properties、revision 和 provenance。
- `Document`：Org/Markdown 文档。
- `Heading`：具有稳定 ID 的标题块，可承载 Org 行为。
- `Attachment`：原始二进制及 MIME、哈希、大小和来源。
- `ExternalObject`：无法自然映射为文档的外部对象。
- `Relation`：有方向、有类型、可带属性的边。
- `Community`：由 selector、固定成员和排除成员定义的实体集合。
- `Artifact`：可删除重建的派生产物。

原生 Org 文档和 heading 使用标准 `ID`；Markdown 文档使用 frontmatter ID，heading 使用最小侵入的 HTML comment ID。路径与标题不是身份。链接解析到稳定 ResourceRef，同时保留原始文本。删除产生 tombstone，以免同步或扫描时误复活。

## 5. 类型系统和规则

每个 Resource 只有一个主 `TYPE`，可以组合多个 Trait。Schema 随 TYPE 定义，避免平级多类型造成字段和规则冲突。

```yaml
types:
  project:
    traits: [taskable, schedulable, para_item, summarizable]
    schema:
      area: { type: ref, target_types: [area], required: true }
      effort: { type: duration }
      outcome: { type: string }
    rules:
      - assert: todo != null
        message: Project must have a TODO state
      - derive: { para: projects }
```

规则种类：

- `classify`：根据路径、标签、属性或来源推断 TYPE。
- `validate`：校验 required、enum、ref、日期与状态转换。
- `derive`：计算 PARA、过期状态或摘要范围；默认只进入索引。
- `react`：响应显式命令，例如完成任务时写 CLOSED 和 LOGBOOK。

规则必须确定、可解释。`inspect --rules` 展示命中规则、输入和结果。只有显式 materialize 才把派生属性写回文件。

## 6. Org-mode 一级语义

支持文件属性、heading、property drawer、属性继承、tags、tag inheritance、可配置 TODO、priority、scheduled、deadline、repeater、checkbox、progress cookie、LOGBOOK、clock、archive、refile、agenda 与 column view 所需属性。

空间级 workflow profile 提供默认值，文件级 `#+TODO` 可以覆盖：

```org
#+TODO: TODO(t) NEXT(n) PEND(p) WAIT(w@/!) | DONE(d!) QUIT(q@)
```

状态转换是受 workflow 约束的 Mutation，不是任意字段赋值。它可以原子写入状态、完成时间、日志和审计。

PARA 是声明式组织策略，不是四套存储类型。`PARA` 可由 TYPE、属性、标签和目录规则推导；稳定属性承载语义，目录用于展示。Task 与 PARA 提供高层接口，但编译为统一资源操作：

```text
task.capture / transition / schedule / agenda / clock
para.create / move / review / archive
```

## 7. 资源代数

接口遵循 SICP 的基本元素、组合方法、抽象方法与闭包性。

基本元素：

- `ResourceRef`：稳定引用。
- `Selector`：声明式选择资源。
- `Projection`：字段与关系展开方式。
- `MutationOperation`：set、unset、append、insert、move、transition 等操作。
- `Recipe`：可复用的派生策略。
- `Plan`：可原子预览或执行的操作组合。

核心原语：

```text
resolve(input)
query(selector, projection)
read(ref, projection)
mutate(target, operations, options)
relate(from, relation, to, options)
derive(scope, recipe, options)
execute(plan, options)
inspect(target)
```

查询返回 ResourceRef，ResourceRef 可继续输入其他原语；Selector 可用于查询、批量修改、派生和同步范围。Task、PARA、同步和 Skill 接口是领域宏，不形成第二套实现。

所有写操作支持 `dry_run`、`expected_revision`、`idempotency_key` 和 actor。结果统一返回 `applied | preview | conflict`、revision、changes、warnings 和 audit ID。稳定错误码包括 `ENTITY_NOT_FOUND`、`REVISION_CONFLICT`、`SOURCE_READ_ONLY`、`AMBIGUOUS_TARGET` 和 `JOB_FAILED`。

## 8. CLI 与 MCP

CLI 直接表达资源代数：

```bash
notez resolve "SICP"
notez query --kind document --tag lisp --expand links_to:1
notez read note:abc
notez mutate note:abc --set status=done --expect rev:123
notez relate note:abc links_to section:def
notez derive community:xyz --recipe llms-txt
notez execute --plan plan.yaml --dry-run
notez inspect source:obsidian-main
```

复杂 Selector、Recipe 和 Plan 接受 JSON/YAML。查询支持 JSON 输出；机器结果写 stdout，诊断写 stderr。便捷命令如 `notez task agenda` 仅是宏。

MCP 暴露同一组正交工具，并增加适合 Agent 的 token-budget projection。MCP Resources 提供稳定、可缓存读取：

```text
notez://spaces/{space_id}
notez://entities/{entity_id}
notez://communities/{community_id}
notez://communities/{community_id}/llms.txt
notez://artifacts/{artifact_id}
notez://jobs/{job_id}
```

模糊名称必须先 `resolve`。命中多个实体时不得静默修改。

## 9. 保真文档处理

```text
Raw Text ⇄ Lossless Syntax Tree ⇄ Semantic Resources
```

语义模型不负责整份文档再生成。Mutation 定位语法节点并生成最小文本补丁，保留空白、注释、属性顺序和用户格式。无修改 round-trip 必须逐字节一致。

Org 为主格式。Markdown 第一版支持 CommonMark/GFM、YAML frontmatter、Wiki Link 和 heading ID，并通过 `format_profile` 声明方言。Obsidian 挂载时识别 Wiki Link 与 embed，但不改写源文件。

文件监控使用静默窗口、内容哈希和原子保存检测。解析失败时保留上一版有效索引，暴露诊断，绝不覆盖异常源文件。

## 10. Source Adapter

```text
describe()                  类型与 capabilities
scan(cursor)                增量枚举 SourceItem
read(external_ref)          内容与 revision
watch(checkpoint)           可选变化流
resolve(locator)            外部位置到稳定引用
import(items, destination)  转为原生资源
```

未来能力：`prepare_write`、`commit_write`、`history`。适配器必须显式声明 `read/watch/history/import/write/transactions`，不能用空实现伪装能力。

每个外部资源保存 source ID、external ID、locator、revision 和原始未映射属性。Git 使用 blob/tree/commit 修订信息；Obsidian 映射 frontmatter、Wiki Link 和 embed；Anytype 通过对象、类型及关系映射接入，第一版只提供契约和测试桩。

## 11. 附件管道

```text
Blob → Extraction → Segment → Artifact
```

Blob 采用内容寻址、哈希去重。Extraction 记录处理器、版本、参数和状态。Segment 保存文本及页码、坐标或时间戳等定位信息。

处理器协议为 `accepts(mime)`、`extract(blob, options)` 和 `inspect(job)`。第一版支持纯文本、PDF、HTML、Office 和图片 OCR。外部工具在隔离临时目录运行，限制输入大小、时间、内存和输出。失败只影响 Extraction，不影响原始 Blob 和笔记。

## 12. 社区、摘要与 Skill

Community 由 `Selector + pinned members + excluded members` 定义。自动算法基于链接密度、标签重叠和语义相似度生成带证据的候选，用户确认后才成为稳定社区。

Artifact Recipe 声明 inputs、ordering、token budget、provenance 和生成策略。核心产物：

- `summary`：供人和 Agent 阅读的社区概览。
- `llms.txt`：稳定入口与资源索引。
- `context-pack`：带正文、关系、定位和预算的结构化 JSON。
- `skill-ir`：平台无关的 Agent Skill 中间表示。

第一版提供 `SKILL.md + references` 导出器。Skill Package 包含 SKILL.md、references、resources.json、provenance.json 和 checks.json。生成流程为 Select、Compile、Validate、Publish。

生成内容分为 generated、curated 和 overrides。刷新先产生 candidate 和语义 diff；确定性索引可自动更新，LLM 改写的指令语义默认需要确认。生命周期为 `fresh → stale → rebuilding → candidate → published`。

高层接口 `skill.build/check/diff/publish/refresh` 编译到 derive/query/execute。

## 13. 同步

第一版为单用户、多设备、离线优先。共享文件夹只传输不可变对象：

```text
heads/       每设备当前快照
manifests/   不可变内容清单
objects/     按哈希寻址的加密对象
tombstones/  删除记录与保留窗口
```

Manifest 包含 space ID、actor ID、parent snapshots、logical path、content hash 和元数据。设备工作目录不是共享文件夹的直接镜像。

合并规则：单边修改快进；不同文件修改合并 manifest；同文件非重叠修改执行保真三方合并；同一区域并发修改、删除与修改竞争或 ID 碰撞生成显式冲突并保留双方。附件按哈希去重。索引、嵌入与 Artifact 不参与同步。

`SyncTransport` 隔离存取与监听机制。未来 Folder 可替换为 Relay 或 P2P，而 manifest、object 和 merge 语义保持稳定。ChangeRecord 用于审计和同步辅助，不取代文件事实来源，为未来混合事件模型留出演进接口。

## 14. 一致性、错误与安全

文件写入使用同目录临时文件、fsync 和原子替换；写入前检查 expected revision。跨多文件 Plan 先生成全部补丁和预期 revision，再进入提交阶段；若平台无法保证真正原子，则使用可恢复 journal 并明确报告部分提交状态。

长任务持久化状态，支持重试、取消和诊断。派生失败不回滚原始内容。外部来源不可用时保留最后有效投影并标记陈旧。

MCP 写操作受空间、来源和操作类型能力限制。日志默认不记录正文。附件处理防护路径穿越、符号链接逃逸、压缩炸弹、恶意 MIME、超时与无限输出。

## 15. 可观察性

规则命中、适配器扫描、文件修改、任务、派生和同步合并携带 trace ID。统一入口：

```bash
notez inspect <target>
notez space doctor
notez job list
notez artifact stale
notez sync conflicts
```

`inspect` 必须能回答资源来自哪里、当前 revision 是什么、哪些规则命中、哪些 Artifact 依赖它、为什么陈旧，以及最近一次写入或同步发生了什么。

## 16. 测试策略

- Golden tests：覆盖真实 Org/Markdown 语料及格式保真。
- Property tests：无修改 round-trip、补丁局部性、Selector 组合和 ID 稳定性。
- Contract tests：CLI/MCP 等价性及 Source/Transport/Extractor 适配器契约。
- State-machine tests：TODO transition、Skill 生命周期和任务状态。
- Fault-injection tests：中断、重复、乱序、损坏对象和并发同步。
- Rebuild tests：删除全部索引后仅从文件与配置恢复。
- Security tests：路径、附件、资源限制与权限边界。

## 17. 演进路径

1. 建立 Resource 代数、保真 Org、原生索引和 CLI/MCP 纵向切片。
2. 增加类型规则、Task/PARA、Markdown 和外部挂载。
3. 增加附件、Community、Artifact 与 Skill IR。
4. 增加文件夹同步与冲突工具。
5. 增加 Anytype、写回能力、中继/P2P 和细粒度实体。
6. 若多人协作成为真实需求，再以 ChangeRecord 为迁移接缝引入操作日志或 CRDT，而不是提前把文件降级为投影。

## 18. 验收标准

第一版完成时，用户能够用 Emacs 编辑原生 Org 文件，用 CLI/MCP 安全查询和修改，执行自定义 TODO、Task 与 PARA 工作流，挂载 Git/Obsidian 内容，建立跨源链接，提取附件文本，生成社区 summary/llms.txt/Skill，并在两台离线设备间通过共享文件夹同步。删除本地 SQLite 和所有派生产物后，系统能从原始文件、配置和必要同步元数据恢复正确知识图。
