# AGENTS.md — Notez 仓库 Agent 指令

> 给在此仓库工作的 AI agent 阅读的项目级上下文。不是 GitNexus 的全局 hook（见 `/home/lszio/Projects/AGENTS.md`），只覆盖 Notez 的领域约定、构建契约与必避陷阱。

## 1. 项目本质

本地优先的 Org/Markdown 知识平台：源文件是事实，SQLite projection 与同步对象是可重建的派生。rust + Dioxus。v1 进度见 `docs/roadmap.org` 与 `docs/current-architecture-and-redesign.md`。

## 2. 必须遵守的契约

1. **协议是脊柱**。所有面向表面的写路径必须经由 `notez_protocol::Request` → `notez_core::application::dispatcher::ApplicationDispatcher`。CLI / web / MCP / HTTP API 都是协议翻译层（见 `crates/cli/src/commands.rs`、`packages/web/src/host.rs`、`crates/api/src/transport.rs`、`crates/mcp/src/server.rs`）。新增操作：先在 `crates/protocol/src/request.rs` 与 `response.rs` 加 typed 变体，再在 dispatcher 接通，最后才在表层暴露。
2. **共享一个 Runtime**。`notez_composition::native::Runtime` 是每个进程唯一的引擎缓存 + watcher。一个进程里 web / API / MCP 必须共用同一个 Runtime（参见 `packages/web/src/host.rs::build_services`），禁止新建第二份引擎缓存。
3. **typed Response 优先**。所有 boundary 都使用 `notez_protocol::Response` 与结构化 `Error`；禁止把 `serde_json::Value` 当响应主类型。
4. **format 解析在 composition 层**。`Engine::with_source` 之后由 `composition::native::open_selected_with_paths` 注册 org/markdown parser。任何写路径不得绕过 parser registry 直接读文件。
5. **Dioxus 不进入服务层**。`crates/composition` / `crates/api` / `crates/mcp` 都不依赖 `dioxus`。`packages/*` 才能用 dioxus。

## 3. 构建 / 测试命令

```bash
cargo test --workspace          # 必须全绿
cargo check -p desktop --features desktop   # desktop native 渲染路径
cargo check -p mobile                        # mobile native 渲染路径
```

`packages/web` 默认 build 把 `public/` 同步到 `target/<profile>/public/`（见 `packages/web/build.rs`），不要手动复制。

## 4. 关键文件速查

| 主题 | 文件 |
|---|---|
| 协议 Request/Response | `crates/protocol/src/{request,response}.rs` |
| 共享 Runtime | `crates/composition/src/lib.rs` (`native::Runtime`) |
| Engine dispatcher | `crates/core/src/application/dispatcher.rs` |
| HTTP API transport | `crates/api/src/transport.rs` (`build_router(auth, ApiState::with_runtime(...))`) |
| MCP stdio / HTTP | `crates/mcp/src/{server.rs,http.rs}` |
| Web 宿主（双模式） | `packages/web/src/{host.rs,main.rs}` |
| Dioxus 共享 UI | `packages/ui/src/{notez.rs,backend.rs}` |
| Desktop EmbeddedBackend | `packages/desktop/src/backend.rs` |
| 文档入口 | `README.org` → `docs/roadmap.org` / `docs/current-architecture-and-redesign.md` |

## 5. 必避陷阱

- **不要**直接 `use notez_api::dispatcher::...` —— 没有这个路径。dispatcher 在 `notez_core::application::dispatcher`。
- **不要**新增第二份 engine HashMap（旧的 `WebState.engines` / `ApiState.engines` 字段已被 `composition::Runtime` 取代，commit cab6f47）。
- **不要**在 `crates/mcp` 默认 features 引入 axum：cli 仍走 stdio。axum 仅在 `http` feature 下编译（commit 09aa357）。
- **不要**绕过 `composition::open_selected` 直接构造 `Engine<SqliteProjection>`：那条路径不会注入 journal/audit/janet executor/parser registry。
- **不要**在 web `main.rs` 用 `dioxus::serve` 跑 headless 模式：headless 必须直接用 `axum::serve`（commit bfa20f1）。
- **不要**把 MCP 工具注册时漏掉 `#[tool]` 宏：`source_list` 历史上就这么丢过一次（commit 9be47b3 修复），rmcp 不会报错但工具不可达。
- **不要**新增测试而不放到 `tests/<name>.rs` 顶层：`tests/<subdir>/*.rs` 不会被 cargo 发现为独立测试 target（commit 9be47b3 教训）。
- **不要**提交 `Cargo.lock` 中 `notez-mcp` 的 `default-features = false` 但漏掉 `http` feature 还要让 web 编译的情况；web 通过 `notez-mcp = { path, optional, features = ["http"] }` 拉起，缺 feature 会无声编译失败。

## 6. 触发器

- 用户提到 `/graphify` → 调用 graphify skill（用户级 `~/.claude/CLAUDE.md` 已说明）。
- 用户要求 review / 架构 → 优先看 `docs/current-architecture-and-redesign.md` §10 "最终判断" + `docs/roadmap.org` 验收场景映射，二者不一致时以代码为准并修文档。
- 用户要求新增 surface（MCP 工具 / HTTP 路由 / CLI 子命令）→ 严格按 §2 协议脊柱顺序落地。

## 7. 风格

- 函数签名 / 模块表头 `//!` 注释优先中文（与 `docs/*.org` 一致），但 Rust 标识符保持英文。
- 不要在 commit 中混合 refactor + feature；同主题 1 个 commit（参考已有历史）。
- 公开 crate 改动必须在 commit body 说明"动机 + 影响 + 验证"。
