# Notez Roadmap

## 当前基线：0.2

当前实现已具备 Rust 模块化单内核、Org/Markdown 扫描、SQLite 投影、CLI/MCP、来源挂载、
附件、社区、派生产物及 folder/relay 同步的纵向切片。主要技术债是链接目标被限制为 ULID、
无 ID 资源身份不稳定、配置分散，以及 CLI/MCP 能力漂移。

## 0.3：无损链接与稳定身份

交付 `LinkTarget`、`LinkOccurrence`、`ResolvedRelation` 和 `ResourceAddress`；支持 Org、标准
Markdown 与 Obsidian 默认 profile；加入确定性派生身份和链接诊断。

发布门槛：重复扫描和数据库重建保持身份稳定；所有支持链接均被保留；歧义不静默决议；旧
relations 查询在迁移窗口可用。

## 0.4：两级配置与空间注册表

交付 XDG 全局配置、空间 `notez.toml`、空间名称选择、配置来源解释、严格校验和旧 JSON 显式
迁移。所有应用服务从已解析的 `RuntimeConfig` 启动。

发布门槛：名称、向上查找和默认空间三种发现方式通过验收；相对路径不依赖 cwd；无效配置在
打开数据库前失败；常见链接零配置工作。

## 0.5：统一协议与接口完整覆盖

建立共享 Request/Response DTO、稳定错误、能力目录和统一 dispatcher；迁移 CLI/MCP 的全部
公开操作，并生成或复用 MCP schema。

发布门槛：能力矩阵测试无缺口；CLI/MCP 对相同请求返回等价领域结果；MCP `inspect` 使用真正
的 inspect 用例；写操作统一执行地址唯一性、capability 和 revision 检查。

## 0.6：保真写回与可观察性强化

把 mutate/relate/task transition 落到最小文本补丁，引入 journal、expected revision、审计和
端到端 trace；完善 source stale 状态和链接解析指标。

发布门槛：无关格式逐字节不变；冲突不会覆盖用户文件；失败可诊断并可恢复。

## 1.0：稳定本地优先知识联邦

冻结公开地址、配置版本和错误码协议；完成跨平台路径、Unicode、长时间重建、故障恢复与安全
审计；提供从 0.x 数据和配置升级的正式迁移工具。

## 后续方向

- 更多 source adapter 和 URI scheme，但通过现有 capability/link profile 扩展。
- 基于历史和内容证据的 rename/move detection。
- 更强的搜索、community 和 Agent context pack，但保持派生产物可删除重建。
- 多设备传输优化，不改变 manifest/object/merge 的同步语义。
