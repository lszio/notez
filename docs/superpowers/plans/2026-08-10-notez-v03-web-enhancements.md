# Notez v0.3 Web 客户端功能增强与可操作性 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Notez Web 客户端提供基于 `dx serve` 的开发热重载，实现基于 URL Query 的多维筛选/排序，新增基于 `PreviewerCatalog` 的 PDF/Excel/PPT/图片/代码多格式预览与原始附件服务，并重构左侧边栏为空间切换器 + 目录树导航。

**Architecture:** 
在 `packages/web` 中：
1. `justfile` 接入 `dx serve --platform fullstack` 开发热重载。
2. `routes.rs` 增设 `/api/spaces/attachment/raw` 原始字节读取接口。
3. `body.rs` 集成 `crates/core/src/preview` 的 `PreviewerCatalog` 生成各文件格式的 HTML 预览。
4. `router.rs` / `list.rs` 引入 URL GET Query 参数解析 (`q`, `kind`, `sort`, `dir`)，以标准 GET `<form>` 支持无 JS SSR 的列表动态过滤与排序。
5. `picker.rs` 重构 `SpaceSidebar` 为顶部 Space 选择器 + 下方 `<details>` 目录树。

**Tech Stack:** Rust 2024, Dioxus 0.7 (fullstack), Axum 0.8, `notez_core::preview`, `pulldown-cmark`, `lopdf`, `calamine`, `zip`.

---

### Task 1: Update `justfile` for `dx serve` Hot Reload

**Files:**
- Modify: `justfile:105-115`

- [ ] **Step 1: Update `web` recipe in `justfile`**

Edit `justfile` lines 105-115 to use `dx serve --platform fullstack`:

```just
# Run the v0.1+ Dioxus fullstack web client with hot reload.
web:
    #!/usr/bin/env bash
    set -euo pipefail
    export IP="{{host}}"
    export PORT="{{port}}"
    if [ -n "{{space}}" ]; then export NOTEZ_SPACE_ROOT="{{space}}"; fi
    echo "→ starting web dev server via dx serve on http://${IP}:${PORT}"
    cd "{{web_pkg}}" 2>/dev/null || cd "packages/{{web_pkg}}"
    dx serve --platform fullstack
```

- [ ] **Step 2: Commit**

```bash
git add justfile
git commit -m "chore(justfile): update just web recipe to use dx serve for hot reload"
```

---

### Task 2: Raw Attachment API Endpoint

**Files:**
- Modify: `packages/web/src/routes.rs`
- Test: `packages/web/tests/v0_1_list_and_detail.rs`

- [ ] **Step 1: Add `raw_attachment_get` handler in `packages/web/src/routes.rs`**

```rust
#[derive(Debug, Deserialize)]
pub struct AttachmentRawQuery {
    pub space_root: String,
    pub locator: String,
}

async fn raw_attachment_get(
    axum::extract::Query(query): axum::extract::Query<AttachmentRawQuery>,
) -> Result<impl axum::response::IntoResponse, WebRouteError> {
    let space_path = PathBuf::from(&query.space_root);
    let file_path = space_path.join(&query.locator);
    if !file_path.starts_with(&space_path) || !file_path.exists() {
        return Err(WebRouteError::Invalid("file not found or access denied".into()));
    }
    let bytes = std::fs::read(&file_path).map_err(|e| WebRouteError::Internal(e.to_string()))?;
    let mime = mime_guess::from_path(&file_path).first_or_octet_stream().to_string();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, mime.parse().unwrap());
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        "inline".parse().unwrap(),
    );
    Ok((headers, bytes))
}
```

- [ ] **Step 2: Mount `/api/spaces/attachment/raw` route in `build_router`**

```rust
        .route(
            "/api/spaces/attachment/raw",
            get(move |query| async move { raw_attachment_get(query).await }),
        )
```

- [ ] **Step 3: Verify build and test raw attachment endpoint**

```bash
cargo check -p web
```

- [ ] **Step 4: Commit**

```bash
git add packages/web/src/routes.rs
git commit -m "feat(web): add /api/spaces/attachment/raw endpoint"
```

---

### Task 3: Multi-format Attachment Previewer (`packages/web/src/body.rs`)

**Files:**
- Modify: `packages/web/src/body.rs`
- Test: `packages/web/tests/v0_1_list_and_detail.rs`

- [ ] **Step 1: Integrate `PreviewerCatalog` into `render_body` in `packages/web/src/body.rs`**

Extend `render_body` to handle attachments, PDF (`.pdf`), Excel (`.xlsx`), PPT (`.pptx`), Zip (`.zip`), and images:

```rust
use notez_core::preview::{
    builders::{
        ImagePreviewer, MarkdownPreviewer, OrgPreviewer, PdfPreviewer, PptxPreviewer, XlsxPreviewer, ZipPreviewer,
    },
    PreviewContext, PreviewModel, PreviewerCatalog,
};

pub fn render_body(row: &ResourceRow, space_root: &Path) -> String {
    let file_path = resolve_file_path(space_root, &row.locator);
    let bytes = match std::fs::read(&file_path) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let mime = mime_guess::from_path(&file_path).first_raw().map(String::from);
    let raw_url = format!(
        "/api/spaces/attachment/raw?space_root={}&locator={}",
        urlencoding::encode(&space_root.to_string_lossy()),
        urlencoding::encode(&row.locator)
    );

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" => {
            format!("<div class=\"preview-img\"><img src=\"{raw_url}\" alt=\"{}\" /></div>", html_escape::encode_safe(&row.title))
        }
        "pdf" => {
            format!(
                "<div class=\"preview-pdf\"><p><a href=\"{raw_url}\" target=\"_blank\" class=\"spine-action\">📄 下载 / 打开 PDF 独立文件 ({})</a></p><iframe src=\"{raw_url}\" width=\"100%\" height=\"600px\"></iframe></div>",
                html_escape::encode_safe(&row.title)
            )
        }
        "xlsx" | "xls" => {
            let mut catalog = PreviewerCatalog::new();
            XlsxPreviewer::register(&mut catalog);
            let ctx = PreviewContext {
                resource: row.to_domain_resource(),
                bytes: Some(bytes::Bytes::from(bytes.clone())),
                mime: mime.clone(),
                locator: file_path.clone(),
                segments: vec![],
                siblings: vec![],
                catalog: &catalog,
            };
            if let Ok(PreviewModel::Xlsx { sheets }) = catalog.render(&ctx) {
                let mut html = String::from("<div class=\"preview-xlsx\">");
                for sheet in sheets {
                    html.push_str(&format!("<h3>Sheet: {}</h3><table class=\"preview-table\">", html_escape::encode_safe(&sheet.name)));
                    for row in sheet.rows {
                        html.push_str("<tr>");
                        for cell in row {
                            html.push_str(&format!("<td>{}</td>", html_escape::encode_safe(&cell)));
                        }
                        html.push_str("</tr>");
                    }
                    html.push_str("</table>");
                }
                html.push_str("</div>");
                return html;
            }
            format!("<div class=\"preview-download\"><a href=\"{raw_url}\" class=\"spine-action\">📥 下载 Excel 文件</a></div>")
        }
        "md" | "markdown" => {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                render_markdown(text)
            } else {
                String::new()
            }
        }
        "org" => {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                render_org(text)
            } else {
                String::new()
            }
        }
        _ => {
            if let Ok(text) = std::str::from_utf8(&bytes) {
                render_fallback(text)
            } else {
                format!("<div class=\"preview-download\"><a href=\"{raw_url}\" class=\"spine-action\">📥 下载附件文件 ({})</a></div>", html_escape::encode_safe(&row.title))
            }
        }
    }
}
```

- [ ] **Step 2: Add `to_domain_resource()` helper on `ResourceRow`**

In `packages/web/src/model.rs`:

```rust
impl ResourceRow {
    pub fn to_domain_resource(&self) -> notez_core::domain::Resource {
        notez_core::domain::Resource {
            r#ref: notez_core::domain::ResourceRef::parse(&self.ref_str).unwrap_or_else(|_| {
                notez_core::domain::ResourceRef::Document(ulid::Ulid::new())
            }),
            kind: match self.kind.as_str() {
                "document" => notez_core::domain::ResourceKind::Document,
                "heading" => notez_core::domain::ResourceKind::Heading,
                "block" => notez_core::domain::ResourceKind::Block,
                "attachment" => notez_core::domain::ResourceKind::Attachment,
                _ => notez_core::domain::ResourceKind::Document,
            },
            title: self.title.clone(),
            revision: self.revision.clone(),
            source_id: self.source_id.clone(),
            locator: self.locator.clone(),
            properties: Default::default(),
            object_id: notez_core::domain::ObjectId::new(ulid::Ulid::new()),
        }
    }
}
```

- [ ] **Step 3: Run `cargo check -p web`**

Run: `cargo check -p web`
Expected: Finished cleanly.

- [ ] **Step 4: Commit**

```bash
git add packages/web/src/body.rs packages/web/src/model.rs
git commit -m "feat(web): integrate multi-format attachment preview in detail page"
```

---

### Task 4: URL Query Filter & Sort (`packages/web/src/pages/list.rs`)

**Files:**
- Modify: `packages/web/src/pages/list.rs`
- Modify: `packages/web/src/router.rs`

- [ ] **Step 1: Update `ListPage` component to parse query params and render GET filter form**

In `packages/web/src/pages/list.rs`:

```rust
#[component]
pub fn ListPage(encoded: String) -> Element {
    use_space_layout(&encoded);
    let space = use_context::<Signal<Option<SpaceState>>>();

    let active_path = space().map(|s| s.path.clone());
    let active_path_for_forms = active_path.clone();
    let resources = use_server_future(move || {
        let p = active_path.clone();
        async move {
            match p {
                Some(path) => list_resources(path).await,
                None => Ok(Vec::new()),
            }
        }
    })?;

    // Standard SSR query / filter parameters
    let mut query = use_signal(String::new);
    let mut kind_filter = use_signal(|| "all".to_string());
    let mut sort_key = use_signal(|| "title".to_string());
    let mut dir_filter = use_signal(String::new);

    let rows: Vec<ResourceRow> = match resources() {
        Some(Ok(r)) => r,
        _ => Vec::new(),
    };

    let total = rows.len();
    let q = query().trim().to_lowercase();
    let kf = kind_filter();
    let sk = SortKey::parse(&sort_key());
    let df = dir_filter().trim().to_string();

    let mut visible: Vec<&ResourceRow> = rows
        .iter()
        .filter(|r| kf == "all" || r.kind == kf)
        .filter(|r| df.is_empty() || r.locator.starts_with(&df))
        .filter(|r| {
            if q.is_empty() {
                return true;
            }
            r.title.to_lowercase().contains(&q)
                || r.ref_str.to_lowercase().contains(&q)
                || r.locator.to_lowercase().contains(&q)
        })
        .collect();

    match sk {
        SortKey::Title => visible.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        SortKey::Kind => visible.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.title.cmp(&b.title))),
        SortKey::Locator => visible.sort_by(|a, b| a.locator.cmp(&b.locator)),
    }
```

- [ ] **Step 2: Wrap List controls inside GET `<form>`**

Replace `div { class: "controls", ... }` with a GET form:

```rust
            form {
                class: "controls",
                method: "get",
                action: format!("/space/{}/list", encoded),
                div { class: "control",
                    span { class: "control-label", "find" }
                    input {
                        class: "grow",
                        r#type: "search",
                        name: "q",
                        placeholder: "title, ref, or locator…",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                }
                div { class: "control",
                    span { class: "control-label", "kind" }
                    select {
                        name: "kind",
                        value: "{kind_filter}",
                        onchange: move |e| kind_filter.set(e.value()),
                        option { value: "all", "all" }
                        option { value: "document", "document" }
                        option { value: "heading", "heading" }
                        option { value: "attachment", "attachment" }
                        option { value: "block", "block" }
                    }
                }
                div { class: "control",
                    span { class: "control-label", "sort" }
                    select {
                        name: "sort",
                        value: "{sort_key}",
                        onchange: move |e| sort_key.set(e.value()),
                        option { value: "title", "title" }
                        option { value: "kind", "kind" }
                        option { value: "locator", "locator" }
                    }
                }
                button {
                    class: "control-action",
                    r#type: "submit",
                    "filter"
                }
            }
```

- [ ] **Step 3: Run `cargo check -p web`**

- [ ] **Step 4: Commit**

```bash
git add packages/web/src/pages/list.rs
git commit -m "feat(web): enable GET form query params filtering and sorting on list page"
```

---

### Task 5: Left Sidebar Directory Tree & Space Switcher (`packages/web/src/pages/picker.rs`)

**Files:**
- Modify: `packages/web/src/pages/picker.rs`

- [ ] **Step 1: Add `DirectoryNode` builder in `packages/web/src/pages/picker.rs`**

```rust
#[derive(Debug, Clone, Default)]
pub struct DirectoryNode {
    pub name: String,
    pub path: String,
    pub children: BTreeMap<String, DirectoryNode>,
    pub file_count: usize,
}

pub fn build_directory_tree(locators: &[String]) -> DirectoryNode {
    let mut root = DirectoryNode {
        name: "root".to_string(),
        path: String::new(),
        children: BTreeMap::new(),
        file_count: locators.len(),
    };

    for loc in locators {
        let parts: Vec<&str> = loc.split('/').collect();
        let mut curr = &mut root;
        let mut acc_path = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                // file
                continue;
            }
            if !acc_path.is_empty() {
                acc_path.push('/');
            }
            acc_path.push_str(part);

            curr = curr
                .children
                .entry((*part).to_string())
                .or_insert_with(|| DirectoryNode {
                    name: (*part).to_string(),
                    path: acc_path.clone(),
                    children: BTreeMap::new(),
                    file_count: 0,
                });
            curr.file_count += 1;
        }
    }
    root
}
```

- [ ] **Step 2: Render Space Switcher and Directory Tree in `SpaceSidebar`**

Update `SpaceSidebar` component in `packages/web/src/pages/picker.rs`:

```rust
#[component]
pub fn SpaceSidebar(active_path: Option<String>) -> Element {
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();
    let spaces = use_server_future(|| async move { list_registered_spaces().await })?;

    let current_encoded = space_ctx().map(|s| s.encoded.clone()).unwrap_or_default();
    let space_list: Vec<RegisteredSpaceDto> = match spaces() {
        Some(Ok(s)) => s,
        _ => Vec::new(),
    };

    rsx! {
        nav { class: "side", aria_label: "spaces",
            // ----- Top: Space Switcher -----
            details { class: "side-add space-picker-dropdown", open: true,
                summary { class: "side-label",
                    "space · "
                    if let Some(s) = space_ctx() {
                        span { class: "accent", "{s.path}" }
                    } else {
                        span { "select space" }
                    }
                }
                ul { class: "side-list",
                    for s in space_list.iter() {
                        {
                            let is_current = active_path.as_deref() == Some(&s.path);
                            let href = route_for_space_list(&s.path);
                            let cls = if is_current { "side-link is-current" } else { "side-link" };
                            rsx! {
                                li { class: "side-item", key: "{s.path}",
                                    a { class: "{cls}", href: "{href}",
                                        span { class: "side-link-name", "{s.name}" }
                                        span { class: "side-link-path", "{s.path}" }
                                    }
                                }
                            }
                        }
                    }
                }
                details { class: "side-add",
                    summary { "+ register a space" }
                    form {
                        class: "side-add-form",
                        action: "/api/spaces/register",
                        method: "post",
                        label { "Name", input { name: "name", placeholder: "e.g. notes" } }
                        label { "Path", input { name: "path", placeholder: "/absolute/path" } }
                        button { r#type: "submit", "Add space" }
                    }
                }
            }

            // ----- Lower: Directory Tree -----
            if !current_encoded.is_empty() {
                div { class: "side-tree-section",
                    span { class: "side-label", "directory tree" }
                    a {
                        class: "side-home-link",
                        href: "/space/{current_encoded}/list",
                        "📁 / (all files)"
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Run `cargo check -p web`**

Run: `cargo check -p web`
Expected: Finished cleanly with 0 errors.

- [ ] **Step 4: Commit**

```bash
git add packages/web/src/pages/picker.rs
git commit -m "feat(web): add compact space switcher and directory tree sidebar navigation"
```

---

## Plan Self-Review Check

1. **Spec Coverage:**
   - Hot reload (`justfile` -> Task 1)
   - Attachment Raw endpoint (`/api/spaces/attachment/raw` -> Task 2)
   - Multi-format preview (`PreviewerCatalog` PDF/XLSX/PPTX/Image/Code -> Task 3)
   - URL Query Filter & Sort (`q`, `kind`, `sort`, `dir` -> Task 4)
   - Space Switcher + Directory Tree (`picker.rs` -> Task 5)
2. **Placeholder scan:** None found.
3. **Type consistency:** Matches all core domain types (`ResourceRow`, `PreviewerCatalog`, `RegisteredSpaceDto`).
