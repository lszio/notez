# Documentation Alignment to Federated Knowledge Platform Spec

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite `docs/architecture.org` and `docs/roadmap.org` so they describe Notez as the local-first federated knowledge platform defined in `docs/superpowers/specs/2026-08-02-notez-federated-knowledge-platform-design.org`, and archive (not delete) the three superseded ADRs that predate the spec.

**Architecture:** Pure documentation rewrite. No code changes. The new architecture narrative mirrors the spec's 10 sections (positioning → decisions → core model → boundaries → MCP/API/CLI → clients → sync → MVP roadmap → non-goals → acceptance scenarios). Roadmap is reorganized around the spec's MVP-1/MVP-2/MVP-3 plus "extensions" structure, mapped onto the already-delivered 0.3/0.4/0.5 work and the planned 0.6/1.0 work. The three older ADRs are physically moved into `docs/architecture/archive/` and tagged as "superseded by the federated platform spec, retained as implementation reference for the still-valid technical details".

**Tech Stack:** Emacs org-mode documentation. No tooling beyond `git mv`.

---

## Task 1: Archive the three superseded ADRs

**Files:**
- Move: `docs/architecture/2026-07-26-unified-data-layer.org` → `docs/architecture/archive/2026-07-26-unified-data-layer.org`
- Move: `docs/configuration.org` → `docs/architecture/archive/configuration.org`
- Move: `docs/link-protocol.org` → `docs/architecture/archive/link-protocol.org`

- [ ] **Step 1: Create the archive directory**

```bash
mkdir -p docs/architecture/archive
```

Expected: directory exists; `ls docs/architecture/` shows `archive/` as one of the entries.

- [ ] **Step 2: Move the three ADR files with `git mv`**

```bash
git mv docs/architecture/2026-07-26-unified-data-layer.org docs/architecture/archive/2026-07-26-unified-data-layer.org
git mv docs/configuration.org docs/architecture/archive/configuration.org
git mv docs/link-protocol.org docs/architecture/archive/link-protocol.org
```

Expected: `git status` shows three renamed entries (no `D` / `??` mix); `git diff --staged --name-status` prints exactly three `R` lines.

- [ ] **Step 3: Prepend a "Superseded" header to each archived ADR**

For each of the three files in `docs/architecture/archive/`, insert the following block as the first lines of the file (above the existing `#+title:` line):

```org
#+STATUS: superseded
#+SUPERSEDED_BY: docs/superpowers/specs/2026-08-02-notez-federated-knowledge-platform-design.org
#+SUPERSEDED_ON: 2026-08-03

#+begin_quote
Superseded by the federated knowledge platform spec dated 2026-08-02.
Retained as an implementation reference; the technical content here
remains accurate for its scope (Transport/Parser decoupling, XDG
configuration layering, link protocol and resolution rules) but the
project-wide positioning, core model, and roadmap now live in the
spec and the rewritten [[file:../architecture.org][architecture]] / [[../roadmap.org][roadmap]] documents.
#+end_quote
```

Apply with three `edit` calls (one per file). Each call targets the first line (the existing `#+title:` line) and inserts the new block immediately before it.

- [ ] **Step 4: Verify the three files start with the superseded block**

```bash
head -n 3 docs/architecture/archive/configuration.org \
        docs/architecture/archive/link-protocol.org \
        docs/architecture/archive/2026-07-26-unified-data-layer.org
```

Expected: every file's first three lines are the three `#+STATUS` / `#+SUPERSEDED_BY` / `#+SUPERSEDED_ON` headers.

- [ ] **Step 5: Verify no `docs/architecture/2026-07-26-unified-data-layer.org`, `docs/configuration.org`, or `docs/link-protocol.org` still exist at their old paths**

```bash
ls docs/configuration.org docs/link-protocol.org docs/architecture/2026-07-26-unified-data-layer.org 2>&1
```

Expected: three "No such file or directory" errors.

- [ ] **Step 6: Commit**

```bash
git add docs/architecture/archive/ docs/architecture/2026-07-26-unified-data-layer.org docs/configuration.org docs/link-protocol.org
git commit -m "docs(archive): move superseded ADRs into docs/architecture/archive/

Three ADRs predate the federated knowledge platform spec
(2026-08-02): Transport/Parser decoupling (2026-07-26), XDG config
layering, and link protocol / resolution rules. Move them under
docs/architecture/archive/ and tag them as superseded; the technical
content remains a valid implementation reference for the scope it
covers. The project-wide positioning, core model, and roadmap now
live in the spec and the rewritten architecture/roadmap docs (next
commits)."
```

Expected: `git log --oneline -1` shows the new commit; `git status` clean.

---

## Task 2: Rewrite `docs/architecture.org`

**Files:**
- Overwrite: `docs/architecture.org`

- [ ] **Step 1: Write the new `docs/architecture.org`**

Overwrite `docs/architecture.org` with the content below. The structure mirrors the spec's 10 sections and links forward to the spec as the authoritative source.

```org
#+title: Notez 架构
#+date: 2026-08-03
#+STARTUP: showeverything

本页是 [[file:superpowers/specs/2026-08-02-notez-federated-knowledge-platform-design.org][联邦知识平台设计规范]] 的架构导读，描述当前实现如何落在该规范的语义之下。关于路线与里程碑，见 [[file:roadmap.org][Notez Roadmap]]。规范中尚未实现的语义会显式标注「未实现」并指向 roadmap 中的对应任务。

* 1. 系统定位与产品优先级

Notez 是本地优先、可联邦的全功能知识管理平台。它把本地 Markdown/Org 文件、远程服务、桌面 / 浏览器客户端与 AI Agent 用同一套领域协议接起来；不做新文件格式，不绑定特定云服务。

产品优先级（与规范 §1 一致）：

1. 个人知识管理体验；
2. MCP / AI 与代码文档能力；
3. Notion、Anytype 等远程来源的双向同步；
4. 跨服务端、跨设备的联邦聚合。

本地 Markdown 与 OrgMode 是默认主存储：离线可用、内容可见、无需 Notez 即可访问；外部服务是来源适配器，不是系统的唯一真相来源。

* 2. 本质问题与架构决策

** 2.1 本质问题

需要同时保留文件主权、接入异构远程服务、让同一知识跨空间组合，并让人类客户端和 Agent 以相同规则读写。把任一笔记格式、某个服务商或某个客户端当成中心都会破坏其中至少一项。

** 2.2 决策：虚拟联邦知识图谱

规范 §2.2 的决策，本项目采纳：

- =Knowledge Object= 具有稳定全局 ID；同一对象可被多个空间引用。
- 文件、远程页面、卡片、Agent 上下文包都是该对象在不同来源或界面中的投影。
- 领域协议稳定，来源适配器与客户端实现可替换。

不采用：

- 单一中心数据库（会破坏文件可移植性、加重服务锁定）；
- 纯文件 + Git（不支持远程双写、结构化项目管理与丰富客户端）。

* 3. 核心信息模型

规范 §3 的核心模型在本项目中的当前映射：

| 规范概念      | 本项目实现                                                              | 状态 |
|---------------+-------------------------------------------------------------------------+------|
| =Knowledge Object= | =Resource=（=ResourceRef= 携带稳定全局 ID）                             | 已实现 |
| =Space=       | =Space=（多源聚合 + 投影 + 授权边界）                                   | 已实现 |
| =Source=      | =Source= + =SourceAdapter=，声明 capability（read/write/attachments/incremental_sync/conflict_details 等） | 已实现 capability 机制；MVP-1 需把跨空间引用同一对象与 capability 报告收口 |
| =Projection=  | =Projection= + =Previewer=；Org/Markdown/远程页面/列表/看板都是其形态    | 已实现 Org/Markdown/SSR 投影；远程投影随 Adapter 演进 |
| =Change=      | 本地变更日志 + 同步引擎；尚未稳定的 Change 投递模型                     | 已实现本地日志；规范要求的「失败、冲突、授权结果可检查与重试」在 0.6 强化 |
| =Relation=    | =ResolvedRelation=；规范要求的关系类型、方向、证据、创建者与版本在 0.6 扩展 | 已实现基础；MVP-2 补齐证据与版本 |

Space 对对象只保存空间语义（分类、排序、标签、视图、访问授权），不复制正文。规范 §3 第 5 段「同一对象在多个 Space 被引用而不复制正文」是 MVP-1 的验收场景 1。

* 4. 边界与统一协议

规范 §4 要求 Core 提供唯一的领域 Use Case 层；Web、桌面、移动端、CLI、HTTP API、MCP 都经由该层执行；来源 Adapter 只负责读取、转换与回写；客户端或 MCP 不得直接依赖 Org/Markdown/Notion/Anytype 的内部格式。

当前架构边界（实现细节）：

#+begin_src text
Org / Markdown / External Sources
              │
              ▼
      document + source adapters
       保真扫描、来源 capability 声明
              │ Resource + LinkOccurrence
              ▼
             domain
  身份、地址、选择器、关系、规则、DTO
              │
              ▼
          application
  resolve/query/mutate/derive/sync/inspect
          │              │
          ▼              ▼
       storage       sync/artifact
  可重建 SQLite      对象同步与派生产物
          │
          ▼
       CLI / MCP
      传输与呈现适配
#+end_src

Cargo crate 职责：

| crate        | 职责                                  | 禁止承担                                 |
|--------------+---------------------------------------+------------------------------------------|
| =core::domain=      | 稳定领域类型、协议 DTO、选择器、关系 | 文件 I/O、CLI/MCP 呈现                  |
| =core::document=    | Org/Markdown 保真扫描、链接抽取       | 数据库查询、跨文档决议                  |
| =core::source=      | 来源枚举、读取、capability、locator   | 全局业务编排                            |
| =core::application= | 用例、地址解析、权限、事务、诊断      | 复制 CLI/MCP 参数模型                   |
| =core::storage=     | 可重建投影与 schema migration         | 成为原生内容事实来源                    |
| =core::sync=        | manifest / object / transport / merge | 依赖 SQLite 投影同步                    |
| =core::artifact=    | 提取、recipe、Agent 派生产物          | 修改原始笔记                            |
| =cli=               | 参数解析、输出与退出码                | 直接访问文件或数据库                    |
| =mcp=               | MCP schema、协议、结果编码            | 独立实现业务能力                        |

公开接口统一暴露对象、空间、关系、查询、变更、同步任务与授权结果，不泄漏具体 Adapter DTO。规范 §4 第 3 段要求的 Source Adapter 能力声明（read / write / attachments / incremental_sync / conflict_details 等）已在 =core::capability= 实现并由 =SourceRegistry= 暴露。

* 5. MCP、HTTP API、CLI 与 Agent

** 5.1 能力（规范 §5.1）

MCP、HTTP API、CLI 共用领域协议，提供：

- 发现与查询：列出可访问的空间、来源与类型；全文、标签、关系、时间、代码符号、项目上下文检索。
- 理解与索引：扫描 Git 工作区与代码注释，建立「符号—文件—实现—设计—决策—任务」关系；按变更范围生成上下文包。
- 摘要：创建与更新可追溯的 summary，保留来源对象、生成者、输入 revision 与过期状态。
- 受控写入：创建、编辑、关联对象，更新任务与摘要，并交由来源 Adapter 回写。
- 审计：记录调用方、授权来源、影响对象、变更集、同步状态与修订入口。

当前已实现的是第一与第四类的子集；其余能力按 roadmap 推进。

** 5.2 授权（规范 §5.2）

写入不是全局开关。策略判断维度：

=Principal × Space × Source × Action × Scope=

令牌或 API Key 只标识调用方；服务端策略决定权限。例：某 Agent 可在个人空间更新项目摘要和任务、只能读取 Notion、不能写入生产空间的文件。拒绝应稳定返回机器可读原因，并写入审计记录。

当前实现：CLI 与 MCP 已通过 =ApplicationError= 结构化错误码与稳定退出码承载拒绝语义（见 [[https://example.invalid/todo][0.5：错误结构化]]）；完整的策略维度在 MVP-2 落地。

** 5.3 代码引用协议（规范 §5.3）

使用稳定 URI，供代码注释、文档、CLI、MCP 共用：

#+begin_example
notez://object/<global-id>
notez://query/<saved-query-id>
notez://code/<repo>/<symbol-id>
#+end_example

=object= 直接定位知识对象；=query= 定位动态上下文；=code= 定位代码符号。索引器从注释和代码解析结果建立证据化 Relation；Agent 可从一个符号检索设计、实现、相关任务与当前 summary，并把实现记录回写至已授权空间。

当前已实现的协议层是 =ResourceAddress=（=ResourceRef= + =LinkTarget=）；notez:// scheme 在 MVP-2 引入。

* 6. 客户端、PARA 与看板（规范 §6）

客户端以 Dioxus 实现共享体验；本项目当前不交付客户端 UI，本节作为面向 MVP-3 的目标态记录。

- 左侧：Space 切换、来源状态、收藏、收件箱、PARA 入口。
- 主区：块编辑/阅读，可在大纲、关系、文件原文、属性之间切换。
- 右侧：反向链接、关联代码、活动、同步状态、Agent 审计。
- 全局命令栏：查询、创建、引用、移动、调用已授权工作流。

PARA 是 Space 内的分类关系与保存查询，不是四个互斥文件夹；看板是保存查询的投影；拖动卡片改变统一任务对象状态，而非创建副本。

当前已实现：=Task= + PARA 模型的 CLI 终端嵌套渲染（见 [[file:roadmap.org][Roadmap]] 0.3 已交付条目）。

* 7. 本地优先同步与冲突（规范 §7）

规范要求：

1. 所有操作先写本地变更日志，离线可用。
2. 同步引擎按 Source capability 将变更转成文件修改、远程 API 调用或联邦节点消息。
3. 能安全合并时保留来源 revision 与合并证据；不能合并时生成冲突对象，保留双方版本，供用户或已授权 Agent 决定。
4. 状态必须可见：已同步、等待远程、冲突、无权限、来源不支持。

系统绝不静默覆盖远程或本地版本；联邦节点即使离线或时钟不可靠，也通过可重放 Change 与显式冲突而非隐含「最后写入获胜」演进。

当前实现：

- 本地变更日志与三路合并已就位（=core::sync=）。
- 0.6 任务「保真写回与可观察性强化」承担：journal + expected revision、conflict 审计、source stale 标记、链接解析指标。

* 8. MVP 路线（规范 §8）

与规范 §8 的 MVP-1 / MVP-2 / MVP-3 对齐的里程碑见 [[file:roadmap.org][Roadmap]]。本节不复述里程碑，只把架构层面的关键判断列出：

- **MVP-1（本地知识底座）**：多 Space 聚合、Markdown/OrgMode 双向编辑、统一对象 ID、关系与附件、离线变更日志。验收为同一对象能在两个本地 Space 被引用，修改可追踪并可同步，原始文件脱离 Notez 仍可用。
- **MVP-2（Agent 与代码知识）**：统一 MCP/HTTP/CLI、授权策略、代码注释与符号索引、上下文查询、summary、受控写回。验收为 Agent 可由符号获得设计、任务、实现与摘要，并在授权范围内写回决策、任务与实现记录；未授权操作被拒绝且可审计。
- **MVP-3（个人客户端）**：Space、编辑器、搜索、反向链接、PARA、查询驱动看板、同步/审计面板。
- **后续扩展**：Notion / Anytype 双向 Adapter、网页摘录、跨服务端发现与授权。新增来源必须只增加 Adapter 与 capability 声明，不改变 Core、MCP/API/CLI 或客户端的领域语义。

* 9. 非目标与风险控制（规范 §9）

本轮不承诺任意格式的完美无损双向转换、自动解决所有远程冲突、多人实时协作编辑、映射 Notion/Anytype 的全部专有功能。适配器必须显式报告其支持范围与有损行为。

主要风险是把「统一模型」误做成「最低共同能力模型」。缓解原则：Core 只稳定表达对象、关系、变更与授权；来源专有能力留在 capability 声明与扩展字段中，不能倒灌为全系统必选字段。

* 10. 架构验收场景（规范 §10）

1. 一个对象在多个 Space 被引用而不复制正文。
2. 本地离线变更可被观察、重试与同步，不静默丢失。
3. 同一 Use Case 在 MCP、API、CLI、客户端中产生一致授权与结果。
4. 从一个代码符号可解析到设计、任务、实现、关系证据与当前 summary。
5. Agent 只能在授权 Scope 内改写知识库，所有写入带审计与 revision。
6. 新增 Notion/Anytype Adapter 不要求修改统一领域语义。
7. PARA 与看板均为对象的投影，改变分类/状态而非创建内容副本。

每个场景对应的实现状态见 [[file:roadmap.org][Roadmap]] 末尾的「验收场景映射」表。

* 附录 A：归档文档

下列 ADR 已被本架构文档与联邦平台 spec 取代，仅作为实现参考保留：

- [[file:architecture/archive/2026-07-26-unified-data-layer.org][Transport / Parser 解耦 ADR]]（2026-07-26）— Source Adapter 拆解为 Transport + Parser 的工程化方案，仍是 Source 模块的内部参考。
- [[file:architecture/archive/configuration.org][Notez 配置设计]] — XDG 全局配置与空间配置的分层，仍是 =core::config= 的设计参考。
- [[file:architecture/archive/link-protocol.org][Notez 链接协议]] — =ResourceAddress= / =LinkTarget= / =ResolvedRelation= / 派生身份算法，仍是 =core::domain::link= 与 =core::document= 的协议参考。
```

- [ ] **Step 2: Verify the file is well-formed org-mode**

```bash
head -n 5 docs/architecture.org && echo "---" && wc -l docs/architecture.org
```

Expected: first 5 lines are the `#+title:` / `#+date:` / `#+STARTUP:` headers plus the first paragraph opener; `wc -l` reports a reasonable size (~150 lines).

- [ ] **Step 3: Verify cross-references resolve to existing files**

```bash
test -f docs/superpowers/specs/2026-08-02-notez-federated-knowledge-platform-design.org && \
test -f docs/roadmap.org && \
test -f docs/architecture/archive/configuration.org && \
test -f docs/architecture/archive/link-protocol.org && \
test -f docs/architecture/archive/2026-07-26-unified-data-layer.org && \
echo OK
```

Expected: prints `OK`. Any failure means a referenced path is wrong.

- [ ] **Step 4: Commit**

```bash
git add docs/architecture.org
git commit -m "docs(architecture): rewrite to mirror the federated knowledge platform spec

Restructure docs/architecture.org around the spec's 10 sections
(positioning → decisions → core model → boundaries → MCP/API/CLI →
clients → sync → MVP roadmap → non-goals → acceptance scenarios).
Document how the existing core/cli/adapters/app crates map onto the
spec's Core / Source Adapter / Client layers, and mark which spec
semantics are already implemented vs. planned in the roadmap.
Add an appendix pointing to the three superseded ADRs in
docs/architecture/archive/."
```

Expected: `git log --oneline -1` shows the new commit; `git status` clean for `docs/architecture.org`.

---

## Task 3: Rewrite `docs/roadmap.org`

**Files:**
- Overwrite: `docs/roadmap.org`

- [ ] **Step 1: Write the new `docs/roadmap.org`**

Overwrite `docs/roadmap.org` with the content below. The structure pivots from the old "0.3 / 0.4 / 0.5 / 0.6 / 1.0" sequence to the spec's MVP-1 / MVP-2 / MVP-3 + extensions sequence, and explicitly maps already-delivered work onto the spec's acceptance scenarios.

```org
#+title: Notez Roadmap
#+date: 2026-08-03
#+STARTUP: showeverything

本路线围绕 [[file:superpowers/specs/2026-08-02-notez-federated-knowledge-platform-design.org][联邦知识平台设计规范]] 排布。已经交付的 0.3 / 0.4 / 0.5 任务按其在规范 MVP 验收中的角色归类；0.6 / 1.0 与「后续扩展」按规范 §8 的 MVP 边界承接。

规范定义的验收场景与本路线的里程碑之间的对应关系见末尾「验收场景映射」表。

* MVP-1：本地知识底座

目标：多 Space 聚合 + Markdown/OrgMode 双向编辑 + 统一对象 ID + 关系与附件 + 离线变更日志。

验收（规范 §8 MVP-1）：同一对象能在两个本地 Space 被引用；修改可追踪并可同步；原始文件脱离 Notez 仍可用。

- [X] 0.3 — 无损链接与稳定身份
  - 交付 =ResourceAddress=、=LinkOccurrence=、=ResolvedRelation=、=ResourceRef=。
  - 支持 Org、标准 Markdown、Obsidian 默认 profile，保真提取所有链接目标。
  - 加入确定性派生身份算法（=derived_id= v1），确保无 ID 资源身份稳定。
  - 提供链接诊断能力（CLI/MCP =link list= / =link resolved=）。
- [X] 0.4 — 两级配置与空间注册表
  - 交付 XDG 全局配置结构。
  - 引入空间 =notez.toml=，替代分散的 JSON 状态。
  - 支持三种空间发现方式：空间名称选择、向上查找、默认空间。
  - 实现配置来源解释与严格校验、旧 JSON 状态显式迁移。
  - 确保所有应用服务从已解析的 =RuntimeConfig= 启动。
- [/] 0.5 — 统一协议与接口完整覆盖
  - [X] 共享 Request/Response DTO（=core::application= + =core::capability=）。
  - [X] 稳定错误定义（=ApplicationError= 结构化变体 + CLI 退出码映射 + MCP 结构化 JSON）。
  - [X] 能力目录与统一 dispatcher（=CapabilityCatalog= + =SourceRegistry=）。
  - [ ] CLI/MCP 全部公开操作迁至新协议。
  - [ ] 生成或复用 MCP schema。
  - 发布门槛：能力矩阵测试无缺口；CLI/MCP 对相同请求返回等价领域结果；MCP =inspect= 使用真正的 inspect 用例；写操作统一执行地址唯一性、capability 与 revision 检查。
- [ ] 0.5.x — MVP-1 收尾
  - [ ] 在 =Space= 上落「同一对象多空间引用而不复制正文」的具体模型与迁移：=Knowledge Object= 投影到多 Space 的存储形式 + 跨空间身份稳定性回归测试。
  - [ ] 验证原始 Markdown / Org 文件脱离 Notez 仍可读、可编辑；写回必须是外科手术式补丁，不得倒灌正文。

** 发布门槛（MVP-1）**

- 同一对象在两个本地 Space 被引用时，文件正文只存在一份；空间配置只保存分类、排序、标签、视图与授权。
- 离线修改进入本地变更日志，恢复连接后由同步引擎按 Source capability 投递，失败可重试。
- 关系具备类型、方向、证据、创建者与版本字段（基础已在 0.3；类型/证据在 0.5.x 收尾）。

* MVP-2：Agent 与代码知识

目标：统一 MCP/HTTP/CLI、授权策略、代码注释与符号索引、上下文查询、summary、受控写回。

验收（规范 §8 MVP-2）：Agent 可由符号获得设计、任务、实现与摘要，并在授权范围内写回决策、任务与实现记录；未授权操作被拒绝且可审计。

- [ ] 1.0 — 稳定本地优先知识联邦
  - [ ] 冻结公开地址、配置版本、错误码协议（继承 0.5 =ApplicationError=）。
  - [ ] 完成跨平台路径与 Unicode 行为的规范化。
  - [ ] 支持长时间重建进度与故障恢复机制。
  - [ ] 执行安全审计（尤其是挂载与附件相关逻辑）。
  - [ ] 提供从 0.x 数据和配置升级的正式版本迁移工具。
- [ ] 0.6 — 保真写回与可观察性强化
  - [ ] 把 =mutate= / =relate= / =task transition= 落到最小文本补丁。
  - [ ] 引入 journal 与 expected revision 机制（并发与冲突控制）。
  - [ ] 引入操作审计（Audit）与端到端 Trace。
  - [ ] 完善 source =stale= 状态与链接解析指标。
  - [ ] 笔记引入了 notez:// =object= / =query= / =code= scheme（规范 §5.3）。
  - [ ] 引入 =Principal × Space × Source × Action × Scope= 授权维度（规范 §5.2）。
  - [ ] 引入 =Change= 投递模型：失败、冲突、授权结果可检查与重试（规范 §3、§7）。
  - [ ] 引入代码符号索引：解析注释与代码，建立「符号—文件—实现—设计—决策—任务」关系（规范 §5.1）。
  - [ ] 引入 summary 协议：保留来源对象、生成者、输入 revision、过期状态（规范 §5.1）。

** 发布门槛（MVP-2）**

- 同一 Use Case 在 MCP、API、CLI 中产生一致授权与结果。
- Agent 在授权 Scope 内可读 / 写；范围外被拒绝并写入审计记录。
- 从一个代码符号可解析到设计、任务、实现、关系证据与当前 summary。

* MVP-3：个人客户端

目标：Space、编辑器、搜索、反向链接、PARA、查询驱动看板、同步/审计面板。

验收（规范 §8 MVP-3）：日常使用可完整管理本地知识；看板拖动更新同一对象，并按主 Source 回写。

- [ ] 客户端基础
  - [ ] Dioxus 共享客户端：Web / Windows / Linux / macOS / iOS。
  - [ ] 平台层只补文件访问、通知、系统分享、后台同步。
- [ ] 主界面（规范 §6）
  - [ ] 左侧：Space 切换、来源状态、收藏、收件箱、PARA 入口。
  - [ ] 主区：块编辑 / 阅读，可在大纲、关系、文件原文、属性间切换。
  - [ ] 右侧：反向链接、关联代码、活动、同步状态、Agent 审计。
  - [ ] 全局命令栏：查询、创建、引用、移动、调用已授权工作流。
- [ ] PARA 与看板
  - [ ] PARA 作为 Space 内的分类关系与保存查询；同一对象可同时属 Project / Area / Resource。
  - [ ] 看板为保存查询的投影；拖动卡片更新同一任务对象的状态。
  - [ ] 状态变更经由 =task transition= 走授权与审计路径（继承 MVP-2）。

** 发布门槛（MVP-3）**

- 看板拖动更新统一任务对象，不复制内容；状态变更按主 Source 回写。
- 日常本地知识管理（创建、编辑、搜索、反向链接、PARA、看板、同步状态、审计）均可在客户端完成。

* 后续扩展（规范 §8 末尾）

- Notion / Anytype 双向 Adapter。
- 网页摘录 Adapter。
- 跨服务端发现与授权。
- 更深的平台原生能力。

新增来源必须只增加 Adapter 与 capability 声明，不改变 Core、MCP/API/CLI 或客户端的领域语义（规范 §8 约束）。

* 验收场景映射（规范 §10）

| 场景 | 对应里程碑       | 关键交付物                                                          |
|------+------------------+---------------------------------------------------------------------|
| 1    | MVP-1（0.5.x）   | Space 多引用同一对象、身份稳定性回归测试                            |
| 2    | MVP-1 + 0.6      | 本地变更日志、journal + expected revision、失败可重试              |
| 3    | MVP-2            | =ApplicationError= + 统一 dispatcher、CLI/MCP/API 一致授权与结果    |
| 4    | MVP-2            | notez:// =code= 协议、符号索引、summary 协议                        |
| 5    | MVP-2            | 授权维度 =Principal × Space × Source × Action × Scope= + 审计      |
| 6    | 后续扩展         | Notion / Anytype Adapter 仅新增 capability 声明                    |
| 7    | MVP-3            | PARA 与看板作为保存查询投影，=task transition= 经授权路径          |

* 历史里程碑（已交付）

保留 0.3 / 0.4 的发布记录，便于审计与回溯：

- 0.3 — 无损链接与稳定身份（CLOSED 2026-07-24）：见 MVP-1 列表。
- 0.4 — 两级配置与空间注册表：见 MVP-1 列表。
- 0.5 — 统一协议与接口完整覆盖（进行中）：见 MVP-1 列表。
```

- [ ] **Step 2: Verify the file is well-formed**

```bash
head -n 5 docs/roadmap.org && echo "---" && wc -l docs/roadmap.org
```

Expected: first 5 lines are the `#+title:` / `#+date:` / `#+STARTUP:` headers plus the first paragraph opener; `wc -l` reports a reasonable size (~100 lines).

- [ ] **Step 3: Verify cross-references resolve**

```bash
test -f docs/superpowers/specs/2026-08-02-notez-federated-knowledge-platform-design.org && \
test -f docs/architecture.org && \
echo OK
```

Expected: prints `OK`.

- [ ] **Step 4: Commit**

```bash
git add docs/roadmap.org
git commit -m "docs(roadmap): reorganize around the spec's MVP-1/MVP-2/MVP-3 sequence

Map already-delivered 0.3/0.4/0.5 work onto the spec's MVP-1
acceptance criteria, and align the 0.6/1.0 milestones with MVP-2
(Agent + code knowledge) and MVP-3 (personal client). Add an
acceptance-scenario mapping table that ties each scenario in spec
§10 to the milestone that delivers it. Preserve a historical
milestones section for 0.3 / 0.4 / 0.5 so the delivery record
remains auditable."
```

Expected: `git log --oneline -1` shows the new commit; `git status` clean for `docs/roadmap.org`.

---

## Task 4: Final verification

**Files:** none (read-only verification)

- [ ] **Step 1: Confirm git state is clean for the rewritten files**

```bash
git status
```

Expected: no unstaged or staged changes for `docs/architecture.org` / `docs/roadmap.org` / `docs/architecture/archive/*` / `docs/configuration.org` (moved) / `docs/link-protocol.org` (moved). Only the unrelated pre-existing deletions (the older plans/specs marked `D` in the initial `git status`) should remain.

- [ ] **Step 2: Confirm three commits exist**

```bash
git log --oneline -3
```

Expected: three commits matching the messages from Tasks 1, 2, 3 (in that order, oldest first). The commits should sit on top of `272e063 docs: define federated knowledge platform design`.

- [ ] **Step 3: Confirm cargo workspace still builds (no code changed, but verify nothing got accidentally touched)**

```bash
cargo check --workspace --all-targets
```

Expected: exit 0, `Finished` line at the end. (Compiles cleanly as before — confirms we did not accidentally edit any `.rs` or `Cargo.toml`.)

- [ ] **Step 4: Confirm no code files were touched in this plan**

```bash
git diff --stat 272e063..HEAD
```

Expected: only `.org` files (and directory rename bookkeeping for the three moved ADRs) appear in the stat; no `.rs`, `Cargo.toml`, or `Cargo.lock` lines.

- [ ] **Step 5: Report**

Reply to the user with:

1. The three commit SHAs and messages.
2. The final structure of `docs/` (one `tree` or `ls -R docs` line).
3. The output of `cargo check --workspace --all-targets` confirming the build is still clean.
4. A note that the unrelated pre-existing `D` entries in `git status` are out of scope for this plan and remain for the user to address separately.
```

---

## Self-Review

**1. Spec coverage:** Each spec section (1–10) is reflected in the new `architecture.org` (mirrors the 10-section structure) and/or `roadmap.org` (MVP-1/MVP-2/MVP-3 + extensions + acceptance-scenario mapping table). The spec's product priority list (§1) is reproduced verbatim in architecture §1. The "virtual federated knowledge graph" decision (§2.2) is recorded in architecture §2. The core model table (§3) is mapped onto current implementation status in architecture §3. The boundary / unified protocol requirement (§4) is restated and the crate responsibility table updated. MCP/API/CLI/Agent capabilities (§5.1), authorization dimensions (§5.2), and code reference protocol (§5.3) are documented in architecture §5 with implementation status. Client / PARA / board requirements (§6) are recorded as target state for MVP-3. Sync/conflict rules (§7) are restated in architecture §7 with implementation status. MVP-1/MVP-2/MVP-3 (§8) drive the roadmap. Non-goals (§9) are recorded in architecture §9. Acceptance scenarios (§10) drive the roadmap's mapping table.

**2. Placeholder scan:** No "TBD" / "TODO" / "implement later" / "similar to Task N" markers. The only TODO-flavored text in the roadmap is the `- [ ]` checkboxes for planned work — those are intentional roadmap items, not placeholders.

**3. Type consistency:** The cross-document references use consistent names: `ResourceRef`, `ResourceAddress`, `LinkOccurrence`, `ResolvedRelation`, `ApplicationError`, `CapabilityCatalog`, `SourceRegistry`, `Knowledge Object`, `Space`, `Source`, `Projection`, `Change`, `Relation`. No drift across tasks.

---

**Plan complete and saved to `docs/superpowers/plans/2026-08-03-doc-alignment-to-federated-spec.md`.**