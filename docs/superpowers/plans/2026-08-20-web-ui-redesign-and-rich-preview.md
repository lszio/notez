# Notez Web UI 重构 + 富附件预览 设计规范 (2026-08-20)

承接 v0.3 spec (`2026-08-10-notez-v03-web-enhancements-design.md`) 与当前
v0.5.x-B1 (`2026-08-09-notez-v02-web-operable-design.org` + 8-10 增量)。
本文做两件事：(A) 现状审计 (B) 给出可落地为后续 PR 计划的 UI 重构 +
附件预览扩展设计。

---

## A. 现状审计

### A.1 路由与 SSR 拓扑

| 路由 | 用途 | 现状 |
|---|---|---|
| `/` | 全局 onboarding | 极简：标题 + 3 步说明 |
| `/space/:encoded` | Space 首页 (`SpaceHome`) | 默认渲染 `index.org`/`index.md`/`README.*`；无索引时列出所有文件 |
| `/space/:encoded/list` | 资源列表 (`ListPage`) | 内嵌搜索框 (input signal) + kind filter + 排序；表单 GET 化已在 v0.3 spec 计划 |
| `/space/:encoded/resource/:ref` | 详情 (`DetailPage`) | 走 `render_body` 派发 |
| `/space/:encoded/graph` | 全空间力导向图 (`GraphPage`) | 900×600 SVG |
| `/space/:encoded/preview/:loc` | 任意文件预览 (`PreviewPage`) | 走 `render_path` 派发 |

Shell (`packages/web/src/layout.rs`): 顶部 nav (space dropdown + 搜索框 +
action 区)，下方三栏 body (左 = TreePanel + FilesPanel；中 = page；右 =
PropertiesPanel + GraphPanel)。

### A.2 现状问题（用户列出 3 项 + 我审出的额外项）

1. **左侧目录树只有根节点**：TreePanel 用 `build_tree_with_disk` 收
   索引 + 磁盘合并。`tree.rs:build_tree_from_with_disk` 走
   `parent_folder_opt` 拆 locator，但 PDF / DOCX / PPTX / XLSX 等二
   进制 attachment 的 locator 通常是裸文件名（如 `proposal.pdf`），
   没有 `/`，因此 parent_folder 为空 —— 全部堆积到根。看 `inject_loose`
   的实现细节 (`tree.rs:286-346`)：当 `folder_path == ""` 时统一塞
   到根 `TreeNode`。这就解释了「只剩根节点」。
2. **index 主页不会默认渲染 index.org**：当前 `SpaceHome` 已经调用
   `resolve_index` 找 index.org/index.md/README.*，但用户报告「默
   认没看到」。两种可能：(a) `index.org` 在子目录，搜索只覆盖 root；
   (b) `index_entry` 与 `index_doc_resource` 两套 server future 串
   行触发渲染时序竞争。`space_home.rs:21-44` 用两个独立的
   `use_server_future`，分别看 entry 与 doc；`body_html` 为空就显示
   「no inline body」。搜索是 `palette.rs` 的 ⌘K 弹层，独立 trigger。
3. **搜索 + 附件预览整合**：palette 只能跳 resource reader
   (`route_for_space_resource`)。附件没有 thumbnail / preview 直接
   入口；用户必须从 FilesPanel 列表点开 `route_for_space_preview`。
4. **附件预览覆盖度**：`body.rs:render_kind_inner` 只有图片 / PDF /
   XLSX / PPTX（仅下载）/ ZIP（仅下载）/ MD / ORG；**没有 DOCX**；
   PPTX/XLSX/PDF 走的是 inline `PreviewerCatalog`，但 PPTX 落到
   fallback 「Download PowerPoint」而不是 `PptxPreviewer`（已实现于
   `crates/core/src/preview/builders/pptx.rs`，**未接进 web**）。
5. **视觉/布局问题**：
   - 顶 nav 的 space dropdown 已搬到 nav 左上角 ✓（v0.2 已做）；
   - 但 search trigger 是 `<details>` 隐藏展开，**没有 ⌘K 弹出层**
     的全屏覆盖效果 —— 用户要求的「类似 popup 弹窗」形式未实现；
   - 主页 (`HomePage`) 只有 onboarding，没有真「space overview +
     index 预览 + 最近文件 + 全局快捷入口」的混合 dashboard；
   - 三栏布局在窄屏折叠为单列 (`index.html:909-917`)，但中栏 preview
     区域在桌面端没有最大宽度控制，超长 PDF iframe 会拉伸；
   - 全局没有「附件预览」集合视图：用户无法看到哪些 attachment 是
     DOCX/PPTX 哪些是其他。

### A.3 资源 / 附件 / 预览架构现状（足够支撑重构）

| 层 | 已实现 | 缺失 |
|---|---|---|
| `core::preview::PreviewerCatalog` | 14 个 previewer：markdown, mermaid, d2, iframe, block_embed, query_embed, pdf, xlsx, pptx, zip, image, org, link_embed, fallback | **没有 docx**；没有文本/代码高亮 (`txt`, `csv`, `tsv`, `json`, code) |
| `core::preview::builders::pptx` | 用 `quick-xml + zip` 提每页 `<a:t>` 文本 | 已实现但 web 没接 |
| `core::preview::builders::xlsx` | `calamine` 提所有 sheet 行 | ✓ web 已接 |
| `core::preview::builders::pdf` | `lopdf` 提每页文本 | ✓ web 已接 |
| `web::body::render_kind_inner` | 按 ext 分发：图片/PDF/XLSX 内联；PPTX/ZIP 只下载；MD/ORG 文本 | 没有 DOCX 分支；CSV/TSV 没有表格化；没有代码高亮 |
| `web::routes::build_router` | `GET /api/spaces/attachment/raw` 透传字节 | ✓ |
| `web::tree::build_tree_with_disk` | 索引 + 磁盘合并 | attachment 全堆根目录 |

### A.4 CLI / MCP 既有相关能力

- `notez attachment add / extract / segments`（`use_cases_impl/attachment.rs`）：
  把文件灌进 BlobStore，记录 hash/mime/size_bytes。
- `notez attachment segments <ref>`：返回 `Vec<SegmentRecord>`（`document` scanner
  的细分粒度，附 search 友好）。
- `notez artifact stale / list`：跟踪提取产物新鲜度。
- **MCP 暴露**：`attachment_add`、`attachment_extract`、`attachment_segments`。
- 这些与预览正交；UI 这边只需在 tree / palette 里把 attachment 视作
  一等公民。

---

## B. 重构 + 富预览 设计

### B.1 设计目标

1. 左侧目录树**还原项目结构**：每个 attachment 的 `locator` 都是
   `dir/sub/file.ext`（由 `attachment add` 写入时记录相对路径），
   tree 应该按此聚合，而不是全塞根。
2. Space 主页 `/space/:encoded` 默认渲染 `index.org`/`index.md`/
   `README.*`；找不到时显示「索引未建立 + 给出建索引的 scan 按钮 +
   最近修改文件 8 条 + 全局搜索入口」。
3. 全局搜索以**模态浮层**形式呈现（⌘K / Ctrl-K 唤起），结果分
   「文档 / 标题 / 附件」三组，附件项带类型 badge 与大小，**点击
   直接进 preview 路由**。
4. **附件预览层**统一通过 `core::preview::PreviewerCatalog` 派发
   —— `body.rs` 改成消费 `PreviewModel` JSON，而不再是手写 per-ext
   `format!`。新增 `docx` previewer；`txt/csv/tsv/json` 用现有文本
   fallback 加上 syntax class hint；`code` 用 `syntect` 或简单的
   `<pre><code class="lang-x">` 走客户端 highlight.js 主题。
5. 桌面端三栏布局：左 320px 目录树 + 文件列表（上下两栏）；中
   `minmax(0, 1fr)` + `max-width: 960px` 居中 preview；右 300px
   属性 + 1-hop 图谱（上下两栏）。

### B.2 信息架构（IA）重画

```
┌─────────────────────────────────────────────────────────────────────┐
│ TopNav:  [Space ▼] [/]   [🔎 Search… ⌘K]   [Scan] [Watch] [Graph]  │
├──────────────┬─────────────────────────────────────┬─────────────────┤
│ LEFT (320px) │ MAIN (max-w 960px, scroll)          │ RIGHT (300px)   │
│ ┌──────────┐ │                                     │ ┌─────────────┐ │
│ │DIRECTORY │ │  Breadcrumb:  space › docs › api.md │ │ PROPERTIES  │ │
│ │ TREE     │ │                                     │ │ kind: doc   │ │
│ │ ▾ docs/ 5│ │  ┌───────────────────────────────┐  │ │ rev: 01AR…  │ │
│ │ ▸ cr8s/ 3│ │  │ PREVIEW                      │  │ │ source:     │ │
│ │ ▾ tests 4│ │  │  ┌── PDF ────────┐            │  │ │  native     │ │
│ │   ▸ sub… │ │  │  │ pages 1..12   │            │  │ │ tags: …     │ │
│ └──────────┘ │  │  └───────────────┘            │  │ └─────────────┘ │
│ ┌──────────┐ │  │  ┌── XLSX ───────┐            │  │ ┌─────────────┐ │
│ │FILES (n) │ │  │  │ Sheet1 Sheet2 │            │  │ │1-HOP GRAPH  │ │
│ │ ├─ .org 4 │ │  │  └───────────────┘            │  │ │ ○ ○ ◎ ○ ○  │ │
│ │ ├─ .md 12 │ │  │  ┌── DOCX ──────┐            │  │ │             │ │
│ │ ├─ .pdf 3 │ │  │  │ ¶ ¶ ¶ ¶ ¶ ¶ ¶  │            │  │ └─────────────┘ │
│ │ └─ .docx2│ │  │  └───────────────┘            │  │                 │
│ └──────────┘ │  └───────────────────────────────┘  │                 │
├──────────────┴─────────────────────────────────────┴─────────────────┤
│ StatusBar (tiny): space path · file count · last scan · watch badge  │
└─────────────────────────────────────────────────────────────────────┘
```

### B.3 模块级改动清单

#### B.3.1 `core::preview` 新增 / 修正

- 新增 `builders/docx.rs`：
  - `match`：`.docx` locator 或
    `application/vnd.openxmlformats-officedocument.wordprocessingml.document`
  - 用 `zip::ZipArchive` 读 `word/document.xml`；
  - `quick_xml::Reader` 流式解析，遇到 `w:p` 计段、遇到 `w:t` 收文
    本，遇到 `w:pStyle val="HeadingN"` 升级到 heading；
  - 输出 `PreviewModel::Docx { paragraphs: Vec<DocxParagraph> }`
    （`DocxParagraph { level: u8, runs: Vec<String> }`）。
  - 加 `register()` 到 `default_catalog()`，**位置**：`org` 之后，
    `link_embed` 之前，因为 docx 已经是定形流（org 在前因为它对
    org 文件也可能匹配，要让 org 拿不到 docx 字节时早 fail）。
- 在 `preview/mod.rs::PreviewModel` 加变体：
  ```rust
  pub enum PreviewModel {
      // ...
      Docx { paragraphs: Vec<DocxParagraph> },
  }
  pub struct DocxParagraph {
      pub level: u8, // 0 普通段；1..=6 heading
      pub text: String,
  }
  ```
- 新增 `builders/csv_tsv.rs`：用简单 split（无 csv crate 依赖），
  输出 `PreviewModel::Table { headers, rows }`，与 xlsx 一致的
  web 端表渲染路径复用。
- 新增 `builders/code.rs`：从 `core::artifact` 已有 `extractor.rs`
  借鉴：直接读字节，按 `mime_guess` 给 lang class，返回
  `PreviewModel::Code { lang, body }`（web 端走 highlight.js）。
- 文本/二进制 fallback 已经存在；保持不动。

#### B.3.2 `web::body` 重写派发

把 `render_kind_inner` 替换为：

```rust
fn render_preview(file_path: &Path, bytes: &[u8], locator: &str,
                  ext: &str, title: &str, raw_url: &str) -> String {
    // 1. 构造完整 catalog
    let catalog = notez_core::preview::default_catalog();
    // 2. 构造 placeholder resource + PreviewContext
    let ctx = PreviewContext { resource: placeholder(...),
        bytes: Some(Bytes::copy_from_slice(bytes)),
        mime: mime_guess::from_path(file_path).first_raw(),
        catalog: &catalog, ... };
    // 3. resolve → render → to_html(model)
    match catalog.resolve(&ctx).and_then(|p| p.render(&ctx)) {
        Ok(model) => model_to_html(&model, raw_url),
        Err(_) => fallback_link(raw_url, title),
    }
}
```

`model_to_html` 是新函数，分支：
- `Pdf { pages, text }` → 现有 `render_pdf` HTML；
- `Xlsx { sheets }` → 现有 `render_xlsx` HTML；
- `Pptx { slides }` → 新增 `render_pptx`：每个 slide 一个折叠
  `<details>`，标题取 first run，body 拼接 `<p>`；
- `Docx { paragraphs }` → 新增 `render_docx`：按 level 输出
  `<h1..6>` 或 `<p>`；外加下载 raw link；
- `Image { src, w, h }` → 改用 raw_url 而非 `/s/{}/a/{}`（保持
  attachment raw endpoint 单一来源）；
- `Markdown { html }` → passthrough（已有 `render_markdown` 可换掉）；
- `Org { html, outline }` → passthrough（已有 `render_org_html` 已
  实现，但 `body.rs:render_org` 把 org 当 fenced code —— 这是一个
  bug，应替换为真正的 org_html::render_org_html）；
- `Table { headers, rows }` → 现有表渲染（XLSX 复用）；
- `Code { lang, body }` → `<pre><code class="lang-{lang}">{body}</code></pre>`；
- 其他变体（iframe/block_embed/query_embed/mermaid/d2/link_embed）→
  原样 passthrough。

`render_body` 与 `render_path` 都收敛到这一个函数，消掉现版本里
PPTX/ZIP 误落 fallback 的 bug。

#### B.3.3 `web::tree` 修根目录堆积 bug

- 给 `add_attachment` 在 `application/use_cases_impl/attachment.rs`
  写 attachment 时，确保 `locator` 字段 = 文件相对于 space_root 的
  POSIX 路径（当前实现可能直接用了文件名，导致 tree 折叠）。
  - 现状 `attachment.rs:47-51`：title = `file_name()`；
    但 `Resource.locator` 需要单独写：当前实现看 `attachment.rs`
    没有显式设（继承自 `Resource::new`），导致 locator 是空或脏。
  - **改**：`attachment.rs` 在构造 Resource 时显式
    ```rust
    locator: file_path.strip_prefix(space_root)
        .unwrap_or(file_path).to_string_lossy().replace('\\', "/"),
    ```
- `tree.rs::inject_loose`：当所有 loose 都进根时，把 folder 名加
  「files」前缀，并把根 `count` 加上 loose 数量（已经实现
  `propagate_count`），不要改变 folder 层级。
- 增加「按扩展名聚合」的次级视图（可选）：FilesPanel 顶部加一个
  kind pill（`doc 12 · pdf 3 · docx 2 · xlsx 1 · md 8 · other 5`）
  点击过滤；过滤值通过 `?kind=` URL 参数传给 list 页（这正是
  v0.3 spec 已规划的）。

#### B.3.4 Space 主页 `space_home.rs` 改造

- `index_entry` 与 `index_doc_resource` 合并成单个 server future
  `load_index_document(space_root)`，减少 SSR 时序竞态。
- 找不到 index document 时，主页不再空白；显示：
  - eyebrow: `space · {name}`
  - h1: 「无 index document」
  - lede: 「创建一个 `index.org` 或 `README.md` 后会自动作为主页。」
  - 行动按钮组：
    - 「Open full list」→ `/space/:enc/list`
    - 「Force rescan」→ POST `/api/spaces/scan`
    - 「Watch this space」→ POST `/api/spaces/watch/start`
  - 「Recently modified」列表（取 `list_filesystem` 的前 12 条按
    mtime 倒序）—— **新增**：当前 `list_filesystem` 没返回
    mtime；加一个 `mtime_ms: u64` 字段；按 mtime 倒序。

#### B.3.5 全局搜索改造

- 当前 `palette.rs` 用 `<details>` 展开，体验差。改为：
  - `SearchTrigger` 是 nav 中的一个按钮，点击 toggle `open`
    Signal；
  - `CommandPalette` 渲染时检查 `open`；打开时铺一个
    `position:fixed; inset:0` 的 modal backdrop + 居中的输入框
    + 结果列表；
  - 输入框用 `use_signal`，`oninput` 触发 `use_server_future`
    防抖（防抖逻辑：在 signal 上挂 timer，250ms 内不变化才
    dispatch）；
  - 结果分组：先 docs / headings，后 attachments；每行显示：
    `[kind badge] [title] [path mono-sm]`;
  - 附件结果 `route_for_space_preview`；文档结果
    `route_for_space_resource`。
- ⌘K / Ctrl-K 全局键：在 `Layout` 上挂 `onkeydown` 监听；已经
  写在 `use_server_future` 的 SSR 不需要 key handler，把键盘事件
  监听用 Dioxus 的 `onkeydown` attribute —— SSR 渲染一次后浏览器
  就有了，依赖 progressive enhancement 路径仍然有效。

#### B.3.6 布局与样式

- 新增 CSS 变量 `--col-left-w: 320px; --col-right-w: 300px;
  --col-main-max: 960px;`，三栏 grid：
  ```css
  .shell-body {
    grid-template-columns: var(--col-left-w) minmax(0, 1fr) var(--col-right-w);
  }
  .col-main { max-width: var(--col-main-max); margin: 0 auto; }
  ```
- 状态栏 `StatusBar`：在 shell-body 下方，绝对底栏，显示
  space path / file count / last scan timestamp / watch
  indicator。组件放 `pages/status_bar.rs`。

### B.4 验收（每条对应一个自动化或人眼检查）

1. **树结构还原**：把 `/tmp/notebook/{docs/{a,b}.md, proposals/x.pdf}`
   加进 space 后，左侧目录树显示 `docs (2) · proposals (1)`，不再
   全部堆根。→ 单元测试：`tree.rs::build_tree_from_with_disk` 输入
   上述 locator 集合，断言 `tree.children` 长度为 2。
2. **DOCX 预览**：`fixtures/space/attachments/sample.docx` 在 detail
   页渲染 `<h1>...</h1><p>...</p>`，外加下载链接。→ 单元测试：
   `docx.rs` 输入 fixture docx，断言 `paragraphs.len() > 0` 且
   第一个 level=1。
3. **PPTX 预览**：现版本 PPTX 只显示下载链接；新版本渲染每个
   slide 的折叠卡片。→ 截图回归。
4. **CSV / TSV 预览**：`sample.csv` 在 detail 页渲染 `<table>` 而非
   下载链接。
5. **代码高亮**：`main.rs` 渲染 `<pre><code class="lang-rust">…</code>`
   + 客户端 highlight.js 主题。
6. **Search modal**：⌘K 唤起模态浮层（不是 `<details>`）；输入
   "sync" 显示文档 + 附件分组；附件项点击直接进 preview。
7. **Space 主页 dashboard**：`index.org` 缺失时显示「Recently
   modified」+ 行动按钮；存在时直接渲染 index 正文（已经是
   这样，回归）。
8. **StatusBar**：space path / count / watch badge 显示正确。

### B.5 落地步骤（PR 拆分）

| PR | 范围 | 验收 |
|---|---|---|
| 0 | `core::preview::builders::docx` + 模型 + 注册 | 单元测试覆盖 |
| 1 | `core::preview::builders::csv_tsv` + `code` | 单元测试覆盖 |
| 2 | `web::body` 重写派发到 catalog；bug 修：org 用真正的 org_html；PPTX 接入；附件 locator 写入 | detail/preview 页截图回归 + body.rs 单元测试 |
| 3 | `web::tree` 修根目录堆积 + `attachment.rs` 写正确 locator | 单元测试 `build_tree_from_with_disk` |
| 4 | `web::pages::space_home` dashboard 化 + `list_filesystem` 加 mtime | SSR 截图 |
| 5 | `web::pages::palette` 真 modal + ⌘K 全局键 | 浏览器交互 |
| 6 | `web::pages::status_bar` + shell-body grid CSS 变量 | 视觉 |
| 7 | `web::pages::panel_files` 加 kind pill + `?kind=` URL 过滤 | list 页 GET 表单测试 |

每个 PR 走仓库 `skill://verification-before-completion`：截图 +
`cargo test -p core -p web`。

### B.6 不在本规范范围

- WASM hydration（拆 `core` 拆 rusqlite 工作量过大，仓库已声明
  v0.2 跳过；v0.3 仍跳过）
- 实时 watch SSE / WebSocket 推送
- 客户端编辑（v0.6 任务）
- notez://scheme / OAuth / Agent 写回（v0.6+）

### B.7 风险与权衡

- **Catalog 体积**：`default_catalog()` 已注册 14 个，再加 docx +
  csv_tsv + code = 17。每次 SSR 都构造一份 `PreviewerCatalog::new()`
  + 17 个 `Box<dyn Previewer>` 是冷启动开销，但 `body.rs` 当前就
  是每个 PDF/XLSX 各构造一份 catalog 单 previewer，比新方案还差。
  `default_catalog()` 是 `pub fn`，web 可以单例化进程级 catalog
  存到 `WebState`。
- **DOCX 表格 / 图片 / 嵌入对象**：v1 只输出段落文本 + heading
  层级；表格简单以 `\t` 拼接；图片忽略（用户点「下载 raw」自己
  看图）。这是 MVP 范围，规范不承诺 docx 完整保真。
- **CSV 解析**：不用 csv crate，自己 split 应对 RFC 4180 引号
  转义是不完整的；接受限制 —— 大多数 notez 用户 CSV 来自 Excel
  导出，**没有引号转义**。后续 PR 再换 `csv` crate。

---

## C. 用户视角的快速 demo 脚本（验收用）

```bash
# 准备 fixture
mkdir -p /tmp/notez-demo/{docs,proposals,crates/core}
echo '* Heading' > /tmp/notez-demo/index.org
echo '* heading' > /tmp/notez-demo/docs/a.org
echo '# heading' > /tmp/notez-demo/docs/b.md
curl -L https://.../sample.docx -o /tmp/notez-demo/proposals/spec.docx
echo 'a,b,c\n1,2,3' > /tmp/notez-demo/data.csv
cat > /tmp/notez-demo/main.rs <<EOF
fn main() { println!("hi"); }
EOF

notez --space /tmp/notez-demo scan
notez --space /tmp/notez-demo attachment add --path /tmp/notez-demo/proposals/spec.docx --mime application/vnd.openxmlformats-officedocument.wordprocessingml.document
notez --space /tmp/notez-demo attachment add --path /tmp/notez-demo/data.csv --mime text/csv

cargo run -p web
# 浏览器：http://localhost:8765/space/<encoded>
#   - TreePanel：docs (2), proposals (1), crates (0), <root csv,rs> 2
#   - SpaceHome：渲染 index.org
#   - ⌘K：输入 "spec"，结果包含 spec.docx（kind=attachment badge）
#   - 点 spec.docx：进入 preview，渲染 DOCX 段落
#   - 点 data.csv：进入 preview，渲染 table
#   - 点 main.rs：进入 preview，<pre><code class="lang-rust">
```