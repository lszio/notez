# Notez 本地 Web Server 与组件包/预览器设计

日期：2026-07-27
状态：已确认
范围：本地 HTTP 服务、SSR + 增量 fetch 渲染、组件包抽象、附件预览
子项目编号：第 5 / 5（"本地功能完善" 5 个子项目中的 Web Server + 组件包/预览器）

## 1. 本质问题

Notez 当前所有面向用户的呈现都通过 CLI 或 MCP stdio 完成。缺少一个本地 Web 界面来：

1. 浏览一个或多个 space 中的资源、任务、PARA 结构与附件。
2. 在浏览器中预览常用附件（PDF / Excel / PowerPoint / ZIP / 图片）。
3. 通过可扩展的组件包支持自定义嵌入块（mermaid / d2 / 链接 / 动态查询 / 块引用）。

由此引出几个核心冲突：

- **运行时冲突**：现有 crate 是纯同步的（无 tokio），无法承载长连接 HTTP。
- **职责冲突**：HTML 渲染、可视化资源处理、组件复用必须分离；不能让 web crate 既
  写 HTTP 路由又处理 PDF 文本提取。
- **可扩展性冲突**：预览器不能硬编码进 web crate；必须以 trait 形式让第三方在不影响
  核心的情况下注入新格式。
- **多平台愿景**：用户希望未来能把同一套组件发布为 desktop / mobile 客户端，因此组
  件层必须使用 Dioxus 这类支持多目标渲染的框架，而不是直接写 axum + 原生 HTML。

## 2. 设计目标与非目标

目标：

1. 增加 `notez web` CLI 子命令，启动一个本地 Dioxus-fullstack HTTP 服务。
2. 在浏览器中支持资源、任务、PARA 索引、附件元数据与内容的完整预览。
3. 建立 `crates/preview` 预览器抽象，提供 PDF / XLSX / PPTX / ZIP / Image / Org /
   Markdown / Mermaid / D2 / Iframe / LinkEmbed / QueryEmbed / BlockEmbed 等内置预览器。
4. 建立 `crates/components` 跨平台 Dioxus 组件库，可在 web 与未来 desktop/mobile 共用。
5. 保持架构不变量：web 不直读 SQLite，只走 `application::ApplicationService`；
   previewer 不写 HTML，只产出结构化 `PreviewModel`；components 是 Dioxus 组件库。
6. 一次性支持 `calamine`（xlsx）、`lopdf`（pdf）、`zip`（zip & pptx 内层 zip）、
   `quick-xml`（pptx 解析）四个新 extractor。

非目标：

- 不实现跨 space 链接路由（属于后续子项目）。
- 不实现动态查询 DTO（`#+QUERY` / `#+BEGIN_SRC query` 块），仅留路由占位。
- 不实现 space 元数据（summary / tags / default_tag_rules），属于后续子项目。
- 不实现多平台 client bundle 发布；本里程碑只保证组件抽象可移植，不交付
  desktop / mobile 二进制。
- 不实现写回/编辑操作的 web 化；本里程碑只读。
- 不引入 Node 构建链；客户端 JS 以 ES module 静态资源发布。

## 3. 架构

```
HTTP client
  │
  ▼
axum 路由 (crates/web)
  │
  ├─ Dioxus fullstack handler  → Dioxus 组件 (crates/components)
  │                                  │
  │                                  ▼
  │                            PreviewerCatalog (crates/web)
  │                                  │
  │                                  ▼
  │                            Previewer trait (crates/preview)
  │                                  │
  │                                  ├─ OrgPreviewer
  │                                  ├─ MarkdownPreviewer
  │                                  ├─ PdfPreviewer
  │                                  ├─ XlsxPreviewer
  │                                  ├─ PptxPreviewer
  │                                  ├─ ZipPreviewer
  │                                  ├─ ImagePreviewer
  │                                  ├─ MermaidPreviewer
  │                                  ├─ D2Previewer
  │                                  ├─ IframePreviewer
  │                                  ├─ LinkEmbedPreviewer
  │                                  ├─ QueryEmbedPreviewer
  │                                  └─ BlockEmbedPreviewer
  │
  └─ JSON API (preview.json / 资源 metadata)
            │
            ▼
        ApplicationService (crates/application)
            │
            ├─ SqliteProjection (crates/storage)
            └─ BlobStore (crates/storage)
```

### 3.1 关键边界

- `crates/web` 是 HTTP 传输层 + Dioxus fullstack 入口。不写 SQL、不写 HTML
  模板（模板由 Dioxus 组件生成）。
- `crates/components` 是纯 Dioxus 组件库，输入为已解析的 `Resource` /
  `PreviewModel`，输出为 Element。它可以同时被 web SSR 端与未来 desktop 端复用。
- `crates/preview` 是 `Previewer` trait 与内置实现，输出 `PreviewModel`。
  第三方以实现 trait 并通过 `PreviewerCatalog::register` 注入。
- `crates/artifact`（扩展）新增 PDF/XLSX/PPTX/Zip/Xml extractor；只做字节到
  `ExtractedContent` 的转换，不做 HTML 渲染。
- 所有跨进程边界（HTTP）传递的 DTO 都是 `serde::Serialize` / `Deserialize`。

## 4. Crate 切分与依赖

新增与修改的 crate：

| Crate | 角色 | 新增 workspace dep |
|---|---|---|
| `crates/web`（新建） | CLI `web` 子命令 + axum 路由 + Dioxus fullstack 启动 | axum, tower-http, tokio, dioxus, dioxus-fullstack, tracing |
| `crates/components`（新建） | Dioxus 组件库，跨 web/desktop 复用 | dioxus, dioxus-fullstack (feature gated) |
| `crates/preview`（新建） | `Previewer` trait + 内置预览器实现 | calamine, lopdf, zip, quick-xml, walkdir, regex, once_cell |
| `crates/artifact`（修改） | 增加 Pdf/Xlsx/Pptx/Zip/Xml extractors | lopdf, calamine, zip, quick-xml |
| `crates/cli`（修改） | 注册 `web` 子命令 | 无新增 |

不引入：Node / npm / vite / webpack。所有客户端 JS 以 plain ES module 形式手写并
随 `crates/web` 一同发布到 `crates/web/static/js/preview/`。

## 5. Previewer 模型

### 5.1 Trait

```rust
// crates/preview/src/lib.rs
pub trait Previewer: Send + Sync {
    fn id(&self) -> &'static str;
    fn matches(&self, ctx: &PreviewContext) -> bool;
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError>;
}

pub struct PreviewContext<'a> {
    pub resource: Resource,
    pub bytes: Option<bytes::Bytes>,
    pub mime: Option<String>,
    pub locator: PathBuf,
    pub segments: Vec<SegmentRecord>,
    pub siblings: Vec<Resource>,
    pub catalog: &'a PreviewerCatalog,
    pub service: &'a ApplicationService,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreviewModel {
    Org       { html: String, outline: Vec<Heading> },
    Markdown  { html: String },
    Pdf       { pages: Vec<PdfPage>, text: String },
    Xlsx      { sheets: Vec<Sheet> },
    Pptx      { slides: Vec<Slide> },
    Zip       { entries: Vec<ZipEntry> },
    Image     { src: String, width: u32, height: u32, mime: String },
    Mermaid   { source: String },
    D2        { source: String },
    Iframe    { src: String, sandbox: String },
    LinkEmbed { target: Resource, child: Box<PreviewModel> },
    QueryEmbed{ query: QueryRequest, snapshot: Vec<Resource> },
    BlockEmbed{ source: ResourceRef, html: String },
    Fallback  { message: String },
}
```

### 5.2 匹配顺序

1. URL 显式 override：`?preview=mermaid` 强制使用 MermaidPreviewer（即便默认不匹配）。
2. ResourceKind == Attachment：按 mime → 文件扩展名 → 自定义 scheme 选择预览器。
3. Document / Heading / Block：默认 OrgPreviewer；若正文中含 `#+BEGIN_SRC mermaid`
   块，则在 Org 渲染中将该块替换为 MermaidPreviewer 的输出。
4. 任何未匹配情况：FallbackPreviewer（提供"下载原始文件"链接 + 文件元信息）。

### 5.3 内置预览器

| Previewer | 输入 | 输出 | 关键依赖 |
|---|---|---|---|
| `OrgPreviewer` | document/heading | Org HTML + outline | crates/document 已有扫描器 |
| `MarkdownPreviewer` | document | Markdown HTML | pulldown-cmark |
| `PdfPreviewer` | attachment `.pdf` | 每页文本 + 嵌入图片信息 | lopdf |
| `XlsxPreviewer` | attachment `.xlsx` | 每 sheet 行列 | calamine |
| `PptxPreviewer` | attachment `.pptx` | 每 slide 文本 + 备注 | zip + quick-xml |
| `ZipPreviewer` | attachment `.zip` | 目录树 + 列表 | zip |
| `ImagePreviewer` | attachment 图片 | `<img>` + 尺寸 | 复用 ImageMetadataExtractor |
| `MermaidPreviewer` | `#+BEGIN_SRC mermaid` 块 | 客户端 hydration | 手写 ESM：`/static/js/preview/mermaid.mjs` |
| `D2Previewer` | `#+BEGIN_SRC d2` 块 | 客户端 hydration | 手写 ESM：`/static/js/preview/d2.mjs` |
| `IframePreviewer` | `[[iframe:url]]` 链接 | sandboxed iframe | 无 |
| `LinkEmbedPreviewer` | `[[link:ref]]` 链接 | 递归渲染目标资源 | catalog |
| `QueryEmbedPreviewer` | `#+BEGIN_SRC query` 块 | 当前实现：返回未实现提示 | 留作下个里程碑 |
| `BlockEmbedPreviewer` | `[[block:ref]]` 链接 | 目标 block 文本/HTML | document |
| `FallbackPreviewer` | 兜底 | 下载链接 + 元信息 | 无 |

### 5.4 第三方扩展

第三方 previewer 在自己的 crate 中实现 `Previewer` trait，并通过 feature flag 或
`web` 子命令的 `--previewer <crate>` 参数挂载。`PreviewerCatalog` 在启动时收集
内置与第三方 previewer，匹配顺序按注册顺序，但 `?preview=<id>` 始终最高优先级。

## 6. 路由表

`crates/web` 注册的路由：

| Method | Path | 用途 | 输出 |
|---|---|---|---|
| GET | `/` | space 列表/选择页 | SSR HTML |
| GET | `/s/:space` | 选定 space 的 Hub 视图 | SSR HTML |
| GET | `/s/:space/r/:ref` | 单资源 SSR 渲染 | SSR HTML |
| GET | `/s/:space/r/:ref/preview.json` | preview model | JSON |
| GET | `/s/:space/a/:attachment_id` | 附件原始字节 | bytes / 302 |
| GET | `/s/:space/a/:attachment_id/preview.json` | 附件 preview model | JSON |
| GET | `/s/:space/agenda` | 任务 agenda | SSR HTML |
| GET | `/s/:space/q` | 动态查询（占位） | 501 |
| GET | `/static/*` | 静态资源 | bytes |
| GET | `/healthz` | 健康检查 | text/plain |

`:space` 是 space name 段；解析失败时返回 404。`:ref` 是 `kind:ulid` 形式；解析
失败时返回 400。`:attachment_id` 是 attachment ULID。

## 7. 配置

`notez.toml` 增量字段（仅在出现 `[web]` 段时启用）：

```toml
[web]
bind = "127.0.0.1:3030"
public_url = "http://127.0.0.1:3030"
preview_root = ".notez/web"
```

CLI：

```bash
notez --space <space> web [--bind 127.0.0.1:3030] [--open]
```

`--open` 启动后调用 `xdg-open` / `open` 打开浏览器。

## 8. 数据流

### 8.1 首屏 SSR（资源页）

```
HTTP GET /s/work-notes/r/document:01J...
  → axum route
  → Dioxus fullstack handler
  → ApplicationService::read(ref) → Resource
  → PreviewerCatalog::resolve(resource) → matches first Previewer
  → Previewer::render(ctx) → PreviewModel
  → Dioxus 组件 (ResourceCard + AttachmentPanel) → HTML
  → 200 Content-Type: text/html
```

### 8.2 增量 fetch（preview.json）

```
HTTP GET /s/work-notes/r/.../preview.json
  → PreviewerCatalog::resolve + render
  → JSON 200
  → client fetch 完成后 hydrate
```

### 8.3 附件字节

```
HTTP GET /s/work-notes/a/<attachment_id>
  → BlobStore::get(hash) → bytes
  → Content-Type: attachment.mime
  → 200 (或 304 with If-None-Match)
```

## 9. 组件分层

`crates/components` 提供以下组件（每组件都是 Dioxus `#[component]` 函数）：

| 组件 | 输入 | 输出 | 备注 |
|---|---|---|---|
| `SpaceList` | `Vec<SpaceSummary>` | 列表 | `/` 页面 |
| `SpaceHub` | `SpaceSummary, Vec<Resource>, Vec<TaskItem>` | Hub 视图 | `/s/:space` 页面 |
| `ResourceCard` | `Resource, PreviewModel` | 资源详情 | 资源页 |
| `AttachmentPanel` | `Vec<(Resource, PreviewModel)>` | 附件面板 | 资源页下方 |
| `AgendaList` | `Vec<TaskItem>` | agenda | 复用 |
| `Outline` | `Vec<Heading>` | 侧边目录 | Org 文档 |
| `CodeBlock` | `lang, source` | 带语法高亮的代码块 | Org 渲染内部 |
| `LinkEmbed` | `target, child` | 内嵌链接资源 | LinkEmbedPreviewer |
| `BlockEmbed` | `block, html` | 内嵌块 | BlockEmbedPreviewer |
| `MermaidBlock` | `source` | 占位 + ESM 注入 | MermaidPreviewer |
| `D2Block` | `source` | 占位 + ESM 注入 | D2Previewer |
| `IframeBlock` | `src, sandbox` | sandboxed iframe | IframePreviewer |

所有组件保持平台无关：仅依赖 Dioxus 框架与 `PreviewModel` / `Resource` 等 DTO。

## 10. 测试与验收

### 10.1 单元测试

- `crates/preview`：每个内置 previewer 至少 1 个 happy-path 单测 + 1 个非匹配场景。
- `crates/components`：组件 prop smoke test（Dioxus 提供的 `use_state` + 渲染后检
  查 Element）。
- `crates/web`：路由 handler 单元测试（使用 axum-test 或 tower 提供的 ServiceExt）。

### 10.2 端到端测试

新增 `scripts/acceptance-web.sh`：

1. `cargo build -p web` 通过。
2. 启动 `notez --space /tmp/acceptance-web web --bind 127.0.0.1:3939 &`。
3. `curl -fsS http://127.0.0.1:3939/healthz` → 200 "ok"。
4. `curl -fsS http://127.0.0.1:3939/s/<space>` → 200，包含 README 标题、PARA 索引。
5. `curl -fsS http://127.0.0.1:3939/s/<space>/r/<ref>` → 200，HTML 含 org 渲染结果。
6. `curl -fsS http://127.0.0.1:3939/s/<space>/a/<attachment_id>` → 200，字节长度匹配。
7. `curl -fsS http://127.0.0.1:3939/s/<space>/r/<ref>/preview.json | jq .kind` → 期望
   字符串（如 `org` / `pptx` / `zip`）。
8. 关闭服务，断言无 panic。

### 10.3 性能基线

- 单资源 SSR（不含附件字节）：p95 < 200ms（10 个资源列表）。
- 附件 raw 响应：p95 < 100ms（7MB pptx）。
- preview.json：p95 < 150ms。

性能基线通过 `criterion` benchmark 在 `crates/web/benches/ssr.rs` 验证；性能目标
不作为发布门槛，只在文档中记录。

### 10.4 不动测试

现有 `scripts/acceptance-*.sh` 全部保持绿色。

## 11. 风险与约束

- **运行时分裂**：HTTP server 引入 tokio + axum + Dioxus fullstack runtime，与现有
  同步 CLI/MCP 互不冲突（不同进程）。但首次构建会显著增加 release 二进制体积，
  因此在 workspace 顶层用 `--features web` 启用 `crates/web`。
- **Dioxus 0.x API 不稳**：Dioxus fullstack API 仍在演化。固定 patch 版本
  （0.6.x），并在 Cargo.toml 中精确锁定。组件抽象保证即使 Dioxus 升级，previewer
  与组件接口不需变更。
- **PDF/XLSX/PPTX 解析的覆盖度**：本里程碑只提取文本与元信息；PDF 嵌入字体解析
  与 XLSX 公式计算不在范围。`fallback` 链接始终可点击下载原始文件。
- **可移植性**：`crates/components` 必须在 web 目标下编译。`crates/preview` 与平
  台无关；`crates/web` 平台相关（axum + tokio）。后续里程碑验证 desktop / mobile
  目标时，components 与 preview 应当无修改。
- **不引入 Node 构建链**：客户端 ESM 由手写 `crates/web/static/js/preview/*.mjs`
  提供；Mermaid / D2 客户端使用 esm.sh CDN 动态 import。
- **安全性**：附件 raw 路径使用 `Content-Disposition: attachment` + `sandbox`
  iframe 防止 XSS；所有 preview model 字段在 SSR 前经过 `html_escape`。

## 12. 后续里程碑
- 后续子项目（待拆解）：跨 space 链接路由（GlobalConfig、space 寻址 scheme `notez://space/<n>/...`）。
- 后续子项目（待拆解）：Space 元数据（summary / tags / default_tag_rules / 目录预定义规则）。
- 后续子项目（待拆解）：动态查询 DTO（`#+QUERY` / `#+BEGIN_SRC query` 块在 scan 时静态求值）。
- 后续子项目（待拆解）：Dioxus desktop / mobile 客户端 bundle 发布。
