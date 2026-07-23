# Notez 接口、链接与配置演化设计

日期：2026-07-23
状态：已确认
范围：链接模型、公共接口覆盖、全局与空间配置、数据迁移

## 1. 本质问题

Notez 当前把稳定资源身份与文本链接目标视为同一概念：`ResourceRef` 固定为
`<kind>:<ULID>`，`ResourceRelation.target_ref` 又强制要求 `ResourceRef`。结果是扫描器
只能保留成功解析的 ID 链接；文件路径、标题、WikiLink、锚点和 URL 等合法文本链接会被
忽略。缺少显式 ID 的资源还会在扫描时随机生成 ULID，使重建投影可能改变资源身份。

接口存在另一类同源问题：应用服务、CLI 和 MCP 分别维护能力及参数，能力覆盖和行为逐渐
漂移。应用层已有的 PARA、社区列表、source sync/writeback、relay sync、rebuild 等能力
没有在所有公共入口一致暴露，部分同名操作的语义也不一致。

配置目前分散在 `.notez/sources.json`、`.notez/communities.json`、CLI 参数和硬编码默认值
中。JSON 解析失败会静默退回默认值，无法可靠诊断；同时缺少全局空间注册表，用户必须反复
提供空间路径。

本设计的核心冲突是：稳定身份必须严格、唯一且适合写操作，而文本寻址必须宽容、多态、
可保真并允许暂时未解析。二者必须在领域模型中分离。

## 2. 设计目标与非目标

目标：

1. 无损保留 Org、Markdown 和 Obsidian 文本中的多种链接表达。
2. 将原始链接、解析过程和已确认关系分层，未解析或歧义链接不得丢失。
3. 让 CLI、MCP 和未来接口共享同一套应用协议及完整能力目录。
4. 提供 XDG 全局配置和可移动的空间配置，并内置常见链接格式的默认 profile。
5. 支持从现有数据与 JSON 配置迁移，保持文件仍是原生空间的事实来源。

非目标：

- 不把标题或路径升级为永久身份。
- 不在本次设计中增加图形界面或网络服务。
- 不要求用户为常见文本格式逐条配置链接规则。
- 不把 communities 等领域数据混入运行配置。

## 3. 核心链接模型

### 3.1 稳定身份

`ResourceRef` 只表示已经确定的资源身份。它继续用于数据库主键、写操作目标、同步对象和
已解析关系，不承载文件、标题或 URL 等临时 locator。

有显式文本 ID 的资源使用该 ID。无显式 ID 的资源使用确定性派生身份：由 source identity、
规范化 source-relative locator 和资源结构位置生成。派生算法必须带版本并固定规范化规则，
避免重复扫描生成不同身份。标题不参与身份生成。

### 3.2 文本目标

新增 `LinkTarget`，其稳定协议形态为带类型的联合值：

```text
Id     { value, kind_hint? }
File   { path, fragment? }
Title  { title, fragment? }
Url    { url }
Custom { scheme, value, fragment? }
```

每个目标同时保留原始字符串 `raw`、可选显示文本和来源格式。类型化字段用于解析与查询，
`raw` 用于保真、诊断和未来重新解析。未知 scheme 使用 `Custom`，而不是丢弃。

### 3.3 链接出现与关系投影

`LinkOccurrence` 记录：

- source `ResourceRef`；
- source 文件 locator 及文本 span；
- `LinkTarget`；
- relation kind；
- format profile；
- resolution status、候选和诊断。

解析状态为 `unresolved | resolved | ambiguous | external | invalid`。URL 默认属于
`external`，除非某 source adapter 显式声明能够映射它。

`ResolvedRelation` 仅在目标唯一命中时生成，包含 source ref、relation、target ref、
occurrence ID 和解析证据。歧义不得静默选择，未解析不得伪造目标。

### 3.4 默认解析 profiles

Notez 内置以下行为，用户通常无需配置：

- Org：`id:`、`file:`、`file::heading`、URL 和自定义 scheme。
- Markdown：inline link、相对/绝对文件链接、fragment 和 URL。
- Obsidian：path、basename、title、alias、heading 和 block link/embed。

解析使用 source 和当前文档上下文。Obsidian 默认按精确相对路径、alias、basename/title
的顺序生成候选；一旦多个候选同优先级命中，结果即为 ambiguous。可选 `[links]` 配置只
覆盖冲突策略或自定义方言，并与内置 profile 局部合并。

## 4. 统一应用协议与完整接口

应用层提供统一 `NotezService` 能力边界。每项能力定义明确的 Request/Response DTO、
权限、只读/写入属性和错误集合。CLI 与 MCP 只负责传输适配，不重复领域解析和业务逻辑。

所有可以接收资源目标的请求统一使用 `ResourceAddress`：

```text
Ref(ResourceRef) | Locator(LinkTarget)
```

读取与 resolve 可以返回候选及解析证据；写操作只有在地址唯一解析后才能执行，否则返回
`ENTITY_NOT_FOUND` 或 `AMBIGUOUS_TARGET`。响应统一包含结果数据、warnings、trace ID，
写响应还包含 revision、changes 和 audit ID。错误使用稳定 code、message、details 和
可选 remediation，不依赖 CLI 文本。

公共能力目录至少覆盖：

| 能力组 | 操作 |
| --- | --- |
| resource | resolve、query、read、inspect、mutate、relate |
| link | list、resolve、diagnose、reindex |
| task/para | agenda、transition、overview |
| source | add、list、sync、writeback、capabilities |
| attachment | add、extract、segments |
| community/artifact | create、list、derive、freshness、skill export |
| sync | push、pull、relay、conflicts |
| space/system | scan、rebuild、doctor、jobs |
| config/registry | show、validate、space list/register/unregister/inspect |

能力目录是公共接口覆盖的事实来源。每项公开能力必须声明 CLI 和 MCP 暴露方式；确实不适合
某入口时必须显式标记原因。自动化测试比较能力目录、CLI 命令和 MCP tools，防止遗漏。
MCP input schema 应从共享 DTO/schema 生成或直接复用，不能继续手写一份易漂移的 schema。

## 5. 两级配置模型

### 5.1 全局配置

路径优先使用 `$XDG_CONFIG_HOME/notez/config.toml`，未设置时使用
`~/.config/notez/config.toml`。

```toml
version = 1
default_space = "personal"

[spaces.personal]
path = "~/Notes/personal"

[spaces.work]
path = "~/Notes/work"
config = "notez.toml"

[preferences]
output = "human"
log_level = "warn"
```

全局配置只负责空间名称与位置、默认空间和跨空间用户偏好。它不保存某空间的 sources、
workflow 或内容规则。`~` 仅在配置路径字段中由配置加载器显式展开，不能依赖 shell。

### 5.2 空间配置

默认路径为 `<space>/notez.toml`：

```toml
version = 1

[space]
name = "personal"
database = ".notez/index.sqlite"

[workflow]
todo = ["TODO", "NEXT", "WAIT"]
done = ["DONE", "QUIT"]

[[sources]]
name = "vault"
kind = "obsidian"
path = "../vault"
read_only = true
```

空间配置负责 workflow、sources、索引位置、同步和可选格式覆盖。相对路径统一基于空间配置
文件所在目录解析，使空间可以整体移动。运行数据库、缓存、任务状态和诊断仍放在 `.notez/`。

communities 属于用户领域数据，不进入全局或空间运行配置；它应迁移到独立声明文件或原生
文档，并通过领域接口管理。

### 5.3 发现与覆盖

空间选择顺序：

1. `--space` 指定的注册名称或路径；
2. 从当前目录向上查找 `notez.toml`；
3. 全局 `default_space`；
4. 否则返回明确错误，不隐式创建空间。

有效配置的覆盖顺序：内置默认值和链接 profiles、全局用户偏好、空间配置、环境变量、
本次 CLI 参数。CLI 覆盖不回写文件；持久修改使用专门的 config/source/space registry
命令。

配置结构默认拒绝未知字段。解析失败必须报告文件、字段路径和原因并终止，不能静默回退。
顶层 `version` 用于迁移。`config show` 输出合并后的有效值及各值来源，`config validate`
同时校验全局配置、空间配置、路径和 profile 覆盖。

## 6. 数据与配置迁移

SQLite 新增 `link_occurrences` 和 `resolved_relations`。现有 `relations` 数据转换为 resolved
occurrence，并通过兼容视图或过渡查询接口保留旧读取行为。数据库是可重建投影，因此迁移
失败时可以明确要求备份后重建，而不能修改原始笔记。

首次加载新空间配置时，可显式执行迁移命令：

- `.notez/sources.json` 合并进 `notez.toml` 的 `[[sources]]`；
- `.notez/communities.json` 转为独立领域声明；
- 原 JSON 文件保留备份，迁移成功前不删除；
- 冲突字段生成报告，不静默覆盖。

迁移不在普通启动过程中自动写文件。普通启动可检测旧格式并给出可执行的迁移提示。

## 7. 错误处理与可观察性

链接扫描、候选生成和最终决议携带 trace ID。`link diagnose` 和 `inspect` 应展示原始目标、
采用的 profile、候选、每个候选的命中证据及未选择原因。重建后可比较 unresolved、ambiguous
和 resolved 数量，便于发现规则变化。

配置加载报告每层文件位置、最终空间根目录和覆盖来源。敏感配置值在日志和 inspect 输出中
脱敏。source 不可用不删除最后有效投影，但标记 stale；无效配置则在初始化服务前失败。

## 8. 测试与验收

测试至少包括：

1. Org `id:`、`file:`、`file::heading`、普通 URL 和自定义 scheme。
2. Markdown inline link、相对文件、fragment、URL。
3. Obsidian path、basename、title、alias、heading、block 和 embed。
4. 未解析、歧义、跨 source、大小写、路径规范化和 source-relative 解析。
5. 无显式 ID 资源跨重复扫描及数据库重建保持相同派生身份。
6. CLI/MCP 能力矩阵、共享 DTO 和 MCP schema 一致性。
7. 全局配置 XDG 发现、命名空间选择、向上查找、默认空间和覆盖顺序。
8. 空间配置相对路径、未知字段、错误诊断及旧 JSON 显式迁移。
9. 旧 relations 查询在迁移期保持可用，原始链接文本可完整回放。

验收标准是：支持的文本链接全部进入 `LinkOccurrence`；唯一命中才产生
`ResolvedRelation`；重复重建不改变确定性身份；能力目录中的公开操作在 CLI/MCP 中完整
覆盖；用户仅凭全局空间名称即可操作空间；常见链接无需任何用户配置。

## 9. 演化路径与风险

`LinkTarget::Custom` 和带版本 profile 为新格式、新 source adapter 留出扩展点，同时保持
`ResourceRef` 稳定。未来可以增加远程 URI、DOI 或应用专有 scheme，而无需改变关系主键。

主要风险是确定性身份算法一旦发布便成为持久协议。实现前必须固定规范化、大小写、Unicode、
路径移动和结构位置规则，并给算法版本。文件移动后的身份延续需要显式 move detection 或
写入稳定 ID，不能假装路径派生身份天然解决 rename。

第二个风险是“完整接口”导致一次改动范围过大。实施时应先建立共享协议与覆盖测试，再按
能力组逐步迁移 CLI/MCP；旧入口在迁移窗口内适配到新服务，避免两套业务逻辑并存。
