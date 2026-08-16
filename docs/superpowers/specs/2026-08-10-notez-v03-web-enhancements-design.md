# Notez v0.3 Web 客户端功能增强与可操作性设计规范 (2026-08-10)

## 1. 概述与目标

本设计规范承接 Notez v0.2 Web 客户端，解决开发体验与交互体验方面的 4 个核心需求：
1. **热重载支持 (Hot Reload)**：开发命令 `just web` 使用 `dx serve` 替代原始 `cargo run`，支持代码与静态资源增量重编与实时热重载。
2. **多格式附件与丰富预览 (Rich Preview & Attachments)**：在详情页全面集成 `crates/core/src/preview` 模块，支持 PDF (`.pdf`)、Excel (`.xlsx`)、PPT (`.pptx`)、Zip (`.zip`)、图片 (`.png`, `.jpg`, `.svg`, `.webp`) 及代码/文本文件的即时 HTML/表格/图片预览与原始文件直链下载。
3. **列表筛选与排序 (Search, Filter & Sort)**：建立基于 URL Query 参数（`q`, `kind`, `sort`, `dir`）驱动的 GET 表单体系，确保无 WASM 客户端水合的 SSR 模式下所有搜索、类型筛选与字段排序 100% 可用。
4. **左侧边栏目录树 (Directory Tree & Space Switcher)**：重构左侧边栏布局，顶部为 Space 快速切换器下拉框，下半部分展示从资源 `locator` 导出的分层目录树，点击目录节点直接按文件夹筛选资源。

---

## 2. 架构设计

### 2.1 组件与路由关系
- **路由扩展 (`packages/web/src/router.rs`)**：
  - `Route::List { encoded: String }` 支持解析 URL 查询参数：`q` (search query), `kind` (resource kind), `sort` (sort field), `dir` (directory prefix filter).
- **附件 Raw 接口 (`packages/web/src/routes.rs`)**：
  - 新增 `GET /api/spaces/attachment/raw?space_root=...&locator=...` 接口，透传附件文件的 byte 内容与 `Content-Type` Header。
- **Preview 预览层 (`packages/web/src/body.rs`)**：
  - 调用 `crates/core/src/preview` 的 `PreviewerCatalog`（包含 `PdfPreviewer`, `XlsxPreviewer`, `PptxPreviewer`, `ZipPreviewer`, `ImagePreviewer`），将提取出的 `PreviewModel` 转义渲染为优雅的 HTML 片段。

---

## 3. 详细设计

### 3.1 热重载 (`justfile`)
- 更新 `justfile` 中的 `web` recipe：
  ```just
  web:
      export IP="{{host}}"
      export PORT="{{port}}"
      if [ -n "{{space}}" ]; then export NOTEZ_SPACE_ROOT="{{space}}"; fi
      echo "→ starting web dev server via dx serve on http://${IP}:${PORT}"
      cd "{{web_pkg}}" 2>/dev/null || cd "packages/{{web_pkg}}"
      dx serve --platform fullstack
  ```

### 3.2 左侧边栏重构 (Space Switcher + Directory Tree)
- **Space 切换器 (`packages/web/src/pages/picker.rs`)**：
  - 放置在侧边栏最顶端：渲染当前 Space 的 Badge 与下拉框。展开可点击其他已注册/发现的 Space，底部包含展开式 `+ Register a space` 表单。
- **目录树组件 (`packages/web/src/pages/picker.rs` / `DirectoryTree`)**：
  - 由全量资源 `locator`（如 `docs/api.md`, `crates/core/src/lib.rs`）在内存中解析出嵌套的 `DirectoryNode` 树。
  - 使用标准的 `<details>` HTML 元素实现可折叠层级结构。
  - 每个文件夹节点均带由超链接：`/space/:encoded/list?dir=path/to/dir`。

### 3.3 列表搜索、筛选与排序
- **Form 驱动表单 (`packages/web/src/pages/list.rs`)**：
  - 搜索框 `<input name="q">`
  - 类型下拉框 `<select name="kind">`
  - 排序下拉框 `<select name="sort">`
  - 隐藏目录输入 `<input type="hidden" name="dir">`
- 将所有 control 包裹在 `<form method="get" action="/space/:encoded/list">` 中，并加上 `<noscript>` / `onchange="this.form.submit()"` 增强，无 WASM 条件下表单提交均可准确触发带 Query 参数的 URL 加载。

### 3.4 附件与多格式预览 (PDF / XLSX / PPTX / Image / Zip)
- **Raw File 接口 (`/api/spaces/attachment/raw`)**：
  - 安全校验 `space_root` 与 `locator`，防止路径穿越。
  - 根据扩展名或 MIME 类型设置 `Content-Type` 与 `Content-Disposition: inline`。
- **体模渲染逻辑 (`packages/web/src/body.rs`)**：
  - **Image**：渲染 `<div class="preview-img"><img src="/api/spaces/attachment/raw?..." /></div>`
  - **PDF**：通过 `PdfPreviewer` 解析文本与页码，渲染页数卡片 + 文本段落 preview + PDF 原生 `<iframe src="/api/spaces/attachment/raw?..." type="application/pdf">`
  - **XLSX**：通过 `XlsxPreviewer` 提取工作表 (Sheets)，渲染 HTML 表格 `<table class="preview-table">`
  - **PPTX / ZIP**：渲染幻灯片提纲/压缩包文件树列表 HTML
  - **代码与纯文本**：渲染高亮代码块 `<pre><code>...</code></pre>`

---

## 4. 验证计划

1. **编译与功能测试**：
   - 运行 `cargo check --workspace` 与 `cargo test -p web`。
2. **端到端流程验证**：
   - 使用 `dx serve` 或启动服务后，测试 GET `/space/:encoded/list?q=test&kind=document&sort=title&dir=docs` 筛选效果。
   - 访问带有 PDF/Image/XLSX 附件的详情页，验证 Preview HTML 正确输出及 Raw File 接口响应。
   - 验证左侧边栏目录树的层级折叠与链接路由。
