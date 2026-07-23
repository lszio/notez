# Notez 链接协议

## 1. 身份、地址与关系

- `ResourceRef`：已确认且稳定的资源身份，用于主键、写操作和同步。
- `LinkTarget`：文本中出现的原始目标，可以是 ID、文件、标题、URL 或自定义 scheme。
- `LinkOccurrence`：链接在某资源和文本 span 中的一次出现。
- `ResolvedRelation`：某次出现唯一解析成功后形成的关系投影。
- `ResourceAddress`：公共接口可接受的 `ResourceRef` 或 `LinkTarget`。

这五个概念不得合并。标题和路径可能变化或歧义；它们可以寻址，但不是身份。

## 2. LinkTarget 形态

```text
Id     { value, kind_hint? }
File   { path, fragment? }
Title  { title, fragment? }
Url    { url }
Custom { scheme, value, fragment? }
```

所有形态保留 `raw` 和可选显示文本。未知 scheme 进入 `Custom`。解析状态包括
`unresolved`、`resolved`、`ambiguous`、`external` 和 `invalid`。

## 3. 默认格式 profile

| 格式 | 默认支持 |
| --- | --- |
| Org | `id:...`、`file:path`、`file:path::heading`、URL、自定义 scheme |
| Markdown | `[label](path)`、`[label](path#fragment)`、URL |
| Obsidian | `[[path]]`、`[[title]]`、alias、`#heading`、`^block`、embed |

默认规则是产品协议的一部分，无需在每个 space 配置。空间仅在自定义方言、大小写或冲突策略
不同于默认值时覆盖局部字段。

## 4. 决议规则

解析必须使用当前 source、当前文档目录、格式 profile 和目标类型。文件路径先按 source-relative
规范化；fragment 只在文件候选确定后解析。标题或 alias 可以返回多个候选，不能根据遍历顺序
静默选择。URL 默认标记 external。

`resolve` 响应应包含候选 `ResourceRef`、匹配方式、优先级和证据。写操作仅接受 resolved 的
地址；ambiguous 返回 `AMBIGUOUS_TARGET`，unresolved 返回 `ENTITY_NOT_FOUND`。

## 5. 确定性身份

无显式 ID 的资源使用版本化派生算法。路径规范化必须固定分隔符、`.`/`..`、Unicode 和
大小写策略；结构位置应使用可重复计算的语法定位。算法版本进入 provenance，以便未来迁移。

路径派生身份不能天然跨 rename 保持稳定。需要稳定 rename 语义的资源应写入显式 ID，或由
独立 move detection 根据内容和历史确认迁移。

## 6. 查询与诊断

`link list` 枚举 occurrences；`link resolve` 重算指定范围；`link diagnose` 展示原始目标、
profile、候选和证据；`link reindex` 重建全空间链接投影。重建报告必须分别统计 resolved、
unresolved、ambiguous、external 和 invalid。
