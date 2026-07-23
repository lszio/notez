# Notez 文档

Notez 是本地优先、以 Org-mode 为主要行为语言的知识联邦引擎。原生 Org/Markdown 文件是
事实来源，SQLite、关系图、摘要和 Agent 产物均为可重建投影。

## 文档地图

- [架构](architecture.md)：系统边界、稳定模型、组件职责和数据流。
- [链接协议](link-protocol.md)：ID、文件、标题、WikiLink、锚点与 URL 的统一表示和解析。
- [配置](configuration.md)：全局空间注册表、空间配置、发现及覆盖顺序。
- [Roadmap](roadmap.md)：当前基线、演化阶段、验收门槛和后续方向。
- [已确认设计](superpowers/specs/2026-07-23-notez-interface-link-config-design.md)：本轮演化的设计依据。
- [实施计划](superpowers/plans/)：按 TDD 拆分的可执行工程计划。

## 权威性

原生空间中的知识内容以文件为准；公开行为以领域/应用协议和自动化测试为准；配置行为以
`configuration.md` 为准；具体演化决策以 `docs/superpowers/specs/` 中状态为已确认的设计为准。
README 示例不能覆盖这些协议。
