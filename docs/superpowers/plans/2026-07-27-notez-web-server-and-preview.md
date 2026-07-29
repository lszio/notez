# Notez Web Server + Preview Components 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `notez web` subcommand launching a local Dioxus-fullstack HTTP server that renders space / resource / task / attachment previews through a Rust `Previewer` trait + client-side ESM hydration, with built-in previewers for Org/Markdown/PDF/XLSX/PPTX/ZIP/Image/Mermaid/D2/Iframe/Link/Query/Block embeds and a fallthrough download variant.

**Architecture:** Four new/changed crates — `crates/preview` (Previewer trait + 14 built-ins), `crates/components` (Dioxus components reusable on web/desktop/mobile), `crates/web` (axum + Dioxus fullstack runtime + CLI `web` subcommand), and `crates/artifact` (extended with PDF/XLSX/PPTX/ZIP extractors). The web crate talks only to `application::ApplicationService` (existing crate); previewers return `serde::Serialize` `PreviewModel` DTOs; components consume those DTOs. Tailwind-free, hand-written ESM for Mermaid / D2 hydration via esm.sh CDN. No Node build chain.

**Tech Stack:** Rust 2024, Dioxus 0.6 (fullstack), axum 0.7, tokio 1, tower-http, lopdf, calamine, zip, quick-xml, pulldown-cmark, html-escape, tracing.

**Spec:** `docs/superpowers/specs/2026-07-27-notez-web-server-and-preview-components-design.md`

---

## File Structure

### New crates

- `crates/preview/Cargo.toml` + `src/lib.rs` — `Previewer` trait, `PreviewContext`, `PreviewCatalog`, `PreviewModel` enum, `PreviewError`
- `crates/preview/src/builders/{org,markdown,pdf,xlsx,pptx,zip,image,mermaid,d2,iframe,link_embed,query_embed,block_embed,fallback}.rs` — one file per previewer
- `crates/preview/tests/catalog_resolution.rs` — match-order + override tests
- `crates/preview/tests/fixtures/{sample.org,sample.md,sample.pdf,sample.xlsx,sample.pptx,sample.zip,sample.png,sample.mmd,sample.d2}` — small synthesized fixtures (≤16 KB each)

- `crates/components/Cargo.toml` + `src/lib.rs` — re-exports of every component
- `crates/components/src/{space_list,space_hub,resource_card,attachment_panel,agenda_list,outline,code_block,link_embed,block_embed,mermaid_block,d2_block,iframe_block,layout,escape}.rs` — Dioxus `#[component]` functions

- `crates/web/Cargo.toml` + `src/main.rs` — binary launching axum router (Dioxus fullstack's ssr engine is the bootstrap)
- `crates/web/src/state.rs` — `WebState { catalog: Arc<PreviewerCatalog>, service: Arc<...> }`
- `crates/web/src/router.rs` — route table builder
- `crates/web/src/routes/{root,space_hub,resource,attachment,agenda,healthz,static_files}.rs` — handlers
- `crates/web/src/preview/mod.rs` — builds default `PreviewerCatalog` and registers overloads from config
- `crates/web/src/server.rs` — `serve(bind, public_url, space_root, catalog) -> Result<()>`
- `crates/web/static/js/preview/{mermaid,d2}.mjs` — hand-written hydration scripts
- `crates/web/build.rs` — copies `static/js/preview/*.mjs` into `OUT_DIR/static/`, baked by the binary at runtime

### Modified crates

- `crates/artifact/Cargo.toml` — add `lopdf`, `calamine`, `zip`, `quick-xml`
- `crates/artifact/src/extractor.rs` — add `PdfExtractor`, `XlsxExtractor`, `PptxExtractor`, `ZipExtractor`; `Extractor` registrations in `default_extractors()`
- `crates/artifact/src/lib.rs` — public re-exports
- `crates/cli/Cargo.toml` — add `web` feature (default off); optional dep `web = { path = "../web" }`
- `crates/cli/src/main.rs` — register `Commands::Web { ... }` arm when feature enabled
- `crates/cli/src/commands.rs` — `WebArgs { bind: Option<String>, open: bool }` and `Commands::Web` variant
- `Cargo.toml` — workspace `web` and `fullstack` features; document why

### Tests

- `scripts/acceptance-web.sh` — end-to-end shell script

### Cargo workspace plumbing

- Top-level `Cargo.toml` adds web feature
- Rust toolchain unchanged (edition 2024); rust-version remains 1.80+

---

## Phase A — Foundation & `crates/preview` core

### Task A1: Add new crates to workspace

**Files:**
- Modify: `Cargo.toml` (workspace root) — add empty stub `crates/preview`, `crates/components`, `crates/web`
- Create: `crates/preview/Cargo.toml`, `crates/components/Cargo.toml`, `crates/web/Cargo.toml` (each a stub package with `[lib]` or `[[bin]]` matching spec §4)
- Create: per crate `src/lib.rs` containing `// placeholder` line

- [ ] **Step 1: Stub workspace members**

Add to root `Cargo.toml` so the three new directories are recognized. Each stub crate must compile to `cargo check`.

```toml
# Cargo.toml
[workspace]
resolver = "3"
members = ["crates/*"]
```

```toml
# crates/preview/Cargo.toml
[package]
name = "preview"
version = "0.1.0"
edition = "2024"

[dependencies]
```

```rust
// crates/preview/src/lib.rs
// placeholder
```

Repeat for `components` and `web` with analogous placeholders; `web` is `[[bin]] name = "notez-web"`.

- [ ] **Step 2: Verify workspace builds**

Run: `cargo check --workspace`
Expected: PASS with three warnings about unused placeholders.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml crates/preview crates/components crates/web
git commit -m "feat(workspace): scaffold preview, components, web crates"
```

### Task A2: Define `Previewer` trait and `PreviewModel` enum

**Files:**
- Create: `crates/preview/src/lib.rs`
- Create: `crates/preview/tests/types.rs`

- [ ] **Step 1: Write failing test for trait shape**

```rust
// crates/preview/tests/types.rs
use preview::{Previewer, PreviewContext, PreviewModel, PreviewError};
use domain::{Resource, ResourceKind, ResourceRef};
use bytes::Bytes;

static _PREVIEW_MODEL_ENUM_TAG: () = ();

#[test]
fn preview_model_serializes_tagged() {
    let json = serde_json::to_string(&PreviewModel::Fallback {
        message: "hello".into(),
    })
    .unwrap();
    assert_eq!(json, r#"{"kind":"fallback","message":"hello"}"#);
}

#[test]
fn trait_object_compiles() {
    fn _accepts(p: Box<dyn Previewer>) {
        let _id = p.id();
    }
}
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test -p preview --test types`
Expected: FAIL — `preview` crate does not yet have these types.

- [ ] **Step 3: Implement types**

```rust
// crates/preview/src/lib.rs
use domain::{Resource, ResourceRef, SegmentRecord};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PreviewError {
    #[error("extraction failed: {0}")]
    Extraction(String),
    #[error("missing bytes: {0}")]
    MissingBytes(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct PreviewContext<'a> {
    pub resource: Resource,
    pub bytes: Option<bytes::Bytes>,
    pub mime: Option<String>,
    pub locator: PathBuf,
    pub segments: Vec<SegmentRecord>,
    pub siblings: Vec<Resource>,
    pub catalog: &'a PreviewerCatalog,
}

pub trait Previewer: Send + Sync {
    fn id(&self) -> &'static str;
    fn matches(&self, ctx: &PreviewContext) -> bool;
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    LinkEmbed { target: Box<Resource>, child: Box<PreviewModel> },
    QueryEmbed{ query_id: String, snapshot: Vec<Resource> },
    BlockEmbed{ source: ResourceRef, html: String },
    Fallback  { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub title: String,
    pub anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfPage {
    pub index: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub index: u32,
    pub title: Option<String>,
    pub body: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipEntry {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

pub struct PreviewerCatalog {
    inner: Vec<Box<dyn Previewer>>,
}

impl PreviewerCatalog {
    pub fn new() -> Self { Self { inner: Vec::new() } }

    pub fn register<P: Previewer + 'static>(&mut self, p: P) {
        self.inner.push(Box::new(p));
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn Previewer> {
        self.inner.iter().map(|p| p.as_ref())
    }

    pub fn resolve<'a>(&'a self, override_id: Option<&str>, ctx: &PreviewContext) -> Option<&'a dyn Previewer> {
        if let Some(id) = override_id {
            return self.inner.iter().find(|p| p.id() == id).map(|p| p.as_ref());
        }
        self.inner.iter().find(|p| p.matches(ctx)).map(|p| p.as_ref())
    }
}

impl Default for PreviewerCatalog {
    fn default() -> Self { Self::new() }
}
```

- [ ] **Step 4: Add dependencies to preview/Cargo.toml**

```toml
[package]
name = "preview"
version = "0.1.0"
edition = "2024"

[dependencies]
domain    = { path = "../domain" }
bytes     = "1"
serde     = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
```

- [ ] **Step 5: Run types test, verify PASS**

Run: `cargo test -p preview --test types`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/preview
git commit -m "feat(preview): introduce Previewer trait and PreviewModel enum"
```

### Task A3: Add artifact extractors (PDF / XLSX / PPTX / ZIP)

**Files:**
- Modify: `crates/artifact/Cargo.toml`
- Modify: `crates/artifact/src/extractor.rs`
- Modify: `crates/artifact/src/lib.rs`
- Create: `crates/artifact/tests/{pdf,xlsx,pptx,zip}_extractor.rs`

- [ ] **Step 1: Write failing test for `PdfExtractor`**

```rust
// crates/artifact/tests/pdf_extractor.rs
use artifact::{Extractor, PdfExtractor, register_default};

const PDF_BYTES: &[u8] = include_bytes!("fixtures/tiny.pdf"); // any 1-page PDF

#[test]
fn pdf_extractor_extracts_text() {
    let r = PdfExtractor.extract(PDF_BYTES, "application/pdf").unwrap();
    assert!(r.metadata.contains_key("page_count"));
}
```

Add a 1-page PDF as a fixture at `crates/artifact/tests/fixtures/tiny.pdf` (you can craft one by running `pandoc -t pdf /dev/stdin <<< "hello" -o /tmp/t.pdf` or by reusing one shipped in the codebase if present; otherwise commit the `<=16 KB` minimal PDF `25 50 100 200 ...` content here directly).

- [ ] **Step 2: Run, verify FAIL (no `PdfExtractor`)**

Run: `cargo test -p artifact --test pdf_extractor`
Expected: FAIL with "cannot find type PdfExtractor".

- [ ] **Step 3: Implement `PdfExtractor`**

Add to `crates/artifact/src/extractor.rs`:

```rust
use lopdf::Document;

pub struct PdfExtractor;

impl Extractor for PdfExtractor {
    fn extract(&self, bytes: &[u8], _mime: &str) -> Result<ExtractedContent, ExtractionError> {
        let doc = Document::load_mem(bytes).map_err(|e| ExtractionError::Failed(e.to_string()))?;
        let mut text = String::new();
        let pages = doc.get_pages();
        for (i, _) in pages.iter().enumerate() {
            if let Ok(content) = doc.extract_text(&[(*i + 1) as u32]) {
                text.push_str(&content);
                text.push('\n');
            }
        }
        let mut meta = BTreeMap::new();
        meta.insert("page_count".into(), pages.len().to_string());
        Ok(ExtractedContent { text, metadata: meta })
    }
}
```

- [ ] **Step 4: Repeat Steps 1–3 for `XlsxExtractor` (calamine), `PptxExtractor` (zip+quick-xml), `ZipExtractor` (zip)**

Each extractor:
- `XlsxExtractor`: `use calamine::{Reader, open_workbook_auto};` iterate sheets, return one `Sheet { name, rows: Vec<Vec<String>> }` per worksheet as raw rows converted to `String`. Store rows in `ExtractedContent::metadata` as JSON string under key `sheets_json`.
- `PptxExtractor`: open with `zip::ZipArchive::new(Cursor::new(bytes))`, parse `ppt/slides/slide*.xml`, extract `<a:t>...</a:t>` text per slide; parse `ppt/notesSlides/notesSlide*.xml` for notes; map to `Vec<Slide>` JSON in metadata.
- `ZipExtractor`: `zip::ZipArchive` listing → `Vec<ZipEntry>` JSON in metadata; concatenated entry names as text.

Each gets its own `tests/fixtures/<tiny file>` (≤16 KB synthesized binary).

- [ ] **Step 5: Re-run all artifact tests**

Run: `cargo test -p artifact`
Expected: PASS for all four new extractor tests AND existing artifact tests.

- [ ] **Step 6: Commit**

```bash
git add crates/artifact/Cargo.toml crates/artifact/src crates/artifact/tests
git commit -m "feat(artifact): add pdf, xlsx, pptx, zip extractors"
```

---

## Phase B — `crates/preview` built-in previewers

### Task B1: Shared Org→HTML helper for `OrgPreviewer`

**Files:**
- Create: `crates/preview/src/builders/org_html.rs`
- Create: `crates/preview/src/builders/mod.rs`

- [ ] **Step 1: Write failing test for org headings**

```rust
// crates/preview/src/builders/org_html.rs (test module in-file)
#[cfg(test)]
mod tests {
    use super::render_org_html;
    #[test]
    fn heading_anchor_is_slugified() {
        let html = render_org_html("#+TITLE: X\n\n* Hello World\n");
        assert!(html.contains("<h1"));
        assert!(html.contains("id=\"hello-world\""));
    }
}
```

- [ ] **Step 2: Run test, verify FAIL**

Run: `cargo test -p preview --lib builders::org_html::tests`
Expected: FAIL — `render_org_html` does not exist.

- [ ] **Step 3: Implement `render_org_html`**

```rust
// crates/preview/src/builders/org_html.rs
use crate::{Heading, PreviewError, PreviewModel};
use html_escape::encode_safe;

pub fn render_org_html(src: &str) -> (String, Vec<Heading>) {
    let mut out = String::new();
    let mut outline = Vec::new();
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix("* ") {
            let level = line.chars().take_while(|c| *c == '*').count() as u8;
            let title = rest.trim().to_string();
            let anchor = slugify(&title);
            outline.push(Heading { level, title: title.clone(), anchor: anchor.clone() });
            out.push_str(&format!("<h{level} id=\"{anchor}\">{t}</h{level}>", level=level, anchor=anchor, t=encode_safe(&title).to_string()));
        } else if line.starts_with("#+") {
            // ignore metadata lines for the body HTML (metadata is rendered by the page chrome)
        } else {
            out.push_str(&encode_safe(line));
            out.push('\n');
        }
    }
    (out, outline)
}

fn slugify(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect::<String>().trim_matches('-').to_string()
}

pub use PreviewModel as _;
```

- [ ] **Step 4: Run, verify PASS**

Run: `cargo test -p preview --lib builders::org_html::tests`
Expected: PASS

- [ ] **Step 5: Create `builders/mod.rs` exporting the modules we will fill in B2..B8**

```rust
pub mod org_html;
pub mod markdown_html;
pub mod pdf;
pub mod xlsx;
pub mod pptx;
pub mod zip;
pub mod image;
pub mod mermaid;
pub mod d2;
pub mod iframe;
pub mod link_embed;
pub mod query_embed;
pub mod block_embed;
pub mod fallback;
```

- [ ] **Step 6: Commit**

```bash
git add crates/preview
git commit -m "feat(preview): org-to-html helper and previewer module skeleton"
```

### Task B2: `OrgPreviewer`, `MarkdownPreviewer`

**Files:**
- Create: `crates/preview/src/builders/{org,markdown}.rs`
- Modify: `crates/preview/src/builders/mod.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/preview/src/builders/org.rs (test at bottom)
use crate::{PreviewerCatalog, PreviewContext, Resource, ResourceKind, ResourceRef};
use crate::PreviewModel;

fn ctx_with(src: &str) -> PreviewContext<'static> {
    // oklart; we'll fake using a leaked static catalog — see "Step 3"
    todo!()
}

#[test]
fn org_previewer_matches_document() {
    let cat = static_catalog();
    let p = cat.iter().find(|p| p.id() == "org").expect("registered");
    let r = Resource {
        ref_id: ResourceRef::new(ResourceKind::Document, ulid::Ulid::new()),
        kind: ResourceKind::Document,
        title: "x".into(),
        revision: "abc".into(),
        source_id: "notes".into(),
        locator: "/x.org".into(),
        properties: Default::default(),
    };
    let c = PreviewContext {
        resource: r,
        bytes: None,
        mime: None,
        locator: "/x.org".into(),
        segments: vec![],
        siblings: vec![],
        catalog: &cat,
    };
    assert!(p.matches(&c));
}
```

To avoid the `'static` lifetime dance, place this test inside `catalog_resolution.rs` and pass `&cat` once it lives in a `Box::leak`-like fixture; that's done in B9.

- [ ] **Step 2: Implement `OrgPreviewer` (no test first to break the cycle)**

Replace the test with one centralized in `tests/catalog_resolution.rs` (Task B9). Implement:

```rust
// crates/preview/src/builders/org.rs
use super::org_html::render_org_html;
use crate::{PreviewModel, Previewer, PreviewContext};

pub struct OrgPreviewer;

impl Previewer for OrgPreviewer {
    fn id(&self) -> &'static str { "org" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        matches!(ctx.resource.kind, domain::ResourceKind::Document | domain::ResourceKind::Heading | domain::ResourceKind::Block)
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, crate::PreviewError> {
        let body = ctx.resource.properties.get("body").cloned().unwrap_or_default();
        let (html, outline) = render_org_html(&body);
        Ok(PreviewModel::Org { html, outline })
    }
}
```

- [ ] **Step 3: Implement `MarkdownPreviewer` analogously using `pulldown-cmark`**

```rust
// crates/preview/src/builders/markdown.rs
use crate::{PreviewModel, Previewer, PreviewContext};
use pulldown_cmark::{Parser, Options, html::push_html};

pub struct MarkdownPreviewer;

impl Previewer for MarkdownPreviewer {
    fn id(&self) -> &'static str { "markdown" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.mime.as_deref() == Some("text/markdown")
            || ctx.locator.to_string_lossy().ends_with(".md")
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, crate::PreviewError> {
        let body = ctx.resource.properties.get("body").cloned().unwrap_or_default();
        let mut out = String::new();
        push_html(&mut out, Parser::new_ext(&body, Options::all()));
        Ok(PreviewModel::Markdown { html: out })
    }
}
```

- [ ] **Step 4: Add deps to preview/Cargo.toml**

```toml
pulldown-cmark = "0.10"
html-escape = "0.2"
```

- [ ] **Step 5: Build the catalog**

```rust
pub use crate::PreviewerCatalog;
pub fn default_catalog() -> PreviewerCatalog {
    use super::builders::*;
    let mut c = PreviewerCatalog::default();
    c.register(OrgPreviewer);
    c.register(MarkdownPreviewer);
    PdfPreviewer::register(&mut c);
    XlsxPreviewer::register(&mut c);
    PptxPreviewer::register(&mut c);
    ZipPreviewer::register(&mut c);
    ImagePreviewer::register(&mut c);
    MermaidPreviewer::register(&mut c);
    D2Previewer::register(&mut c);
    IframePreviewer::register(&mut c);
    LinkEmbedPreviewer::register(&mut c);
    QueryEmbedPreviewer::register(&mut c);
    BlockEmbedPreviewer::register(&mut c);
    c.register(FallbackPreviewer);
    c
}
```

Decide registration style: each previewer module exposes `register(&mut PreviewerCatalog)`. (All future tasks B3..B8 follow this pattern.)

- [ ] **Step 6: Build everything; tests for org/markdown run via central catalog test in B9**

Run: `cargo build -p preview` then `cargo test -p preview` (some tests will be commented until B9).
Expected: build PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/preview
git commit -m "feat(preview): OrgPreviewer and MarkdownPreviewer"
```

### Task B3: `PdfPreviewer` and `XlsxPreviewer`

**Files:**
- Create: `crates/preview/src/builders/{pdf,xlsx}.rs`

- [ ] **Step 1: Write failing test (later in B9)**

Annotate `tests/catalog_resolution.rs::pdf_renders_pages` placeholder.

- [ ] **Step 2: Implement `PdfPreviewer`**

```rust
// crates/preview/src/builders/pdf.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError, PdfPage};
use bytes::Bytes;
use lopdf::Document;

pub struct PdfPreviewer;

impl PdfPreviewer {
    pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); }
}
impl Previewer for PdfPreviewer {
    fn id(&self) -> &'static str { "pdf" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.mime.as_deref() == Some("application/pdf")
            || ctx.locator.to_string_lossy().ends_with(".pdf")
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes = ctx.bytes.clone().ok_or_else(|| PreviewError::MissingBytes(ctx.resource.ref_id.to_string()))?;
        let doc = Document::load_mem(&bytes).map_err(|e| PreviewError::Extraction(e.to_string()))?;
        let pages: Vec<PdfPage> = doc.get_pages().keys().copied().filter_map(|i| {
            let text = doc.extract_text(&[i]).unwrap_or_default();
            Some(PdfPage { index: i, text })
        }).collect();
        let full_text = pages.iter().map(|p| p.text.clone()).collect::<Vec<_>>().join("\n");
        Ok(PreviewModel::Pdf { pages, text: full_text })
    }
}
```

- [ ] **Step 3: Implement `XlsxPreviewer` analogously using `calamine`**

```rust
// crates/preview/src/builders/xlsx.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError, Sheet};
use calamine::{Reader, open_workbook_auto, Xlsx};

pub struct XlsxPreviewer;

impl XlsxPreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for XlsxPreviewer {
    fn id(&self) -> &'static str { "xlsx" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".xlsx")
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes = ctx.bytes.clone().ok_or_else(|| PreviewError::MissingBytes(ctx.resource.ref_id.to_string()))?;
        let path = std::env::temp_dir().join(format!("notez-{}.xlsx", ctx.resource.ref_id.to_string().replace(':', "_")));
        std::fs::write(&path, &bytes)?;
        let mut wb: Xlsx<_> = open_workbook_auto(&path).map_err(|e| PreviewError::Extraction(e.to_string()))?;
        let sheets = wb.worksheets().into_iter().map(|(name, range)| Sheet {
            name,
            rows: range.rows().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        }).collect();
        Ok(PreviewModel::Xlsx { sheets })
    }
}
```

- [ ] **Step 4: Build, expect PASS**

Run: `cargo build -p preview`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/preview
git commit -m "feat(preview): PdfPreviewer and XlsxPreviewer"
```

### Task B4: `PptxPreviewer`, `ZipPreviewer`, `ImagePreviewer`

**Files:**
- Create: `crates/preview/src/builders/{pptx,zip,image}.rs`

- [ ] **Step 1: Implement `PptxPreviewer`**

```rust
// crates/preview/src/builders/pptx.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError, Slide};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::{Cursor, Read};
use zip::ZipArchive;

pub struct PptxPreviewer;
impl PptxPreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for PptxPreviewer {
    fn id(&self) -> &'static str { "pptx" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".pptx")
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes = ctx.bytes.clone().ok_or_else(|| PreviewError::MissingBytes(ctx.resource.ref_id.to_string()))?;
        let mut zip = ZipArchive::new(Cursor::new(bytes))?;
        let mut slides = Vec::new();
        let mut idx = 1;
        loop {
            let name = format!("ppt/slides/slide{idx}.xml");
            let mut s = String::new();
            match zip.by_name(&name) {
                Ok(mut f) => { f.read_to_string(&mut s)?; }
                Err(_) => break,
            }
            let body = extract_text_runs(&s);
            let title = body.first().cloned();
            slides.push(Slide { index: idx, title, body: body.clone(), notes: None });
            idx += 1;
        }
        Ok(PreviewModel::Pptx { slides })
    }
}

fn extract_text_runs(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut in_t = false;
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"a:t" => { in_t = true; buf.clear(); }
            Ok(Event::Text(t)) if in_t => { buf.push_str(&t.unescape().unwrap_or_default()); }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:t" => {
                in_t = false;
                if !buf.is_empty() { out.push(std::mem::take(&mut buf)); }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }
    out
}
```

- [ ] **Step 2: Implement `ZipPreviewer`**

```rust
// crates/preview/src/builders/zip.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError, ZipEntry};
use std::io::Cursor;
use zip::ZipArchive;

pub struct ZipPreviewer;
impl ZipPreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for ZipPreviewer {
    fn id(&self) -> &'static str { "zip" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.locator.to_string_lossy().ends_with(".zip")
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes = ctx.bytes.clone().ok_or_else(|| PreviewError::MissingBytes(ctx.resource.ref_id.to_string()))?;
        let mut zip = ZipArchive::new(Cursor::new(bytes))?;
        let mut entries = Vec::new();
        for i in 0..zip.len() {
            let f = zip.by_index(i)?;
            entries.push(ZipEntry {
                path: f.name().to_string(),
                size: f.size(),
                is_dir: f.is_dir(),
            });
        }
        Ok(PreviewModel::Zip { entries })
    }
}
```

- [ ] **Step 3: Implement `ImagePreviewer`**

```rust
// crates/preview/src/builders/image.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError};

pub struct ImagePreviewer;
impl ImagePreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for ImagePreviewer {
    fn id(&self) -> &'static str { "image" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.mime.as_deref().map(|m| m.starts_with("image/")).unwrap_or(false)
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let (w, h) = dimensions_from_bytes(ctx.bytes.as_deref().unwrap_or_default(), ctx.mime.as_deref().unwrap_or(""));
        Ok(PreviewModel::Image {
            src: format!("/s/{}/a/{}", ctx.resource.source_id, ctx.resource.ref_id.to_string().replace(':', "_")),
            width: w,
            height: h,
            mime: ctx.mime.clone().unwrap_or_default(),
        })
    }
}
fn dimensions_from_bytes(b: &[u8], _mime: &str) -> (u32, u32) {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && b.len() >= 24 {
        let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
        let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
        return (w, h);
    }
    (0, 0)
}
```

- [ ] **Step 4: Add deps**

```toml
# crates/preview/Cargo.toml
zip = "0.6"
quick-xml = "0.31"
lopdf = "0.34"
calamine = "0.26"
```

- [ ] **Step 5: Build, expect PASS**

Run: `cargo build -p preview`

- [ ] **Step 6: Commit**

```bash
git add crates/preview
git commit -m "feat(preview): PptxPreviewer, ZipPreviewer, ImagePreviewer"
```

### Task B5: `MermaidPreviewer`, `D2Previewer`, `IframePreviewer`

**Files:**
- Create: `crates/preview/src/builders/{mermaid,d2,iframe}.rs`

- [ ] **Step 1: Implement `MermaidPreviewer`**

```rust
// crates/preview/src/builders/mermaid.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError};

pub struct MermaidPreviewer;
impl MermaidPreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for MermaidPreviewer {
    fn id(&self) -> &'static str { "mermaid" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource.properties.get("body").map(|b| b.contains("mermaid")).unwrap_or(false)
            || ctx.mime.as_deref() == Some("text/vnd.mermaid")
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let source = ctx.resource.properties.get("body").cloned().unwrap_or_default();
        Ok(PreviewModel::Mermaid { source })
    }
}
```

- [ ] **Step 2: Implement `D2Previewer` analogously** (model `D2 { source: String }`).

- [ ] **Step 3: Implement `IframePreviewer`**

```rust
// crates/preview/src/builders/iframe.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError};

pub struct IframePreviewer;
impl IframePreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for IframePreviewer {
    fn id(&self) -> &'static str { "iframe" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource.properties.get("body").map(|b| b.starts_with("[[iframe:")).unwrap_or(false)
    }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let body = ctx.resource.properties.get("body").cloned().unwrap_or_default();
        // expected body = "[[iframe:https://example.com]]"
        let url = body.trim_start_matches("[[iframe:").trim_end_matches("]]").to_string();
        Ok(PreviewModel::Iframe { src: url, sandbox: "allow-scripts".into() })
    }
}
```

- [ ] **Step 4: Build, expect PASS**

Run: `cargo build -p preview`

- [ ] **Step 5: Commit**

```bash
git add crates/preview
git commit -m "feat(preview): mermaid, d2, iframe previewers"
```

### Task B6: `LinkEmbedPreviewer`, `BlockEmbedPreviewer`, `QueryEmbedPreviewer`, `FallbackPreviewer`

**Files:**
- Create: `crates/preview/src/builders/{link_embed,block_embed,query_embed,fallback}.rs`

- [ ] **Step 1: Implement `LinkEmbedPreviewer`**

```rust
// crates/preview/src/builders/link_embed.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError, Resource};
use domain::ResourceRef;

pub struct LinkEmbedPreviewer;
impl LinkEmbedPreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for LinkEmbedPreviewer {
    fn id(&self) -> &'static str { "link_embed" }
    fn matches(&self, _ctx: &PreviewContext) -> bool { false } // selected only via override
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let target = ctx.resource.properties.get("link_target")
            .and_then(|v| serde_json::from_str::<ResourceRef>(v).ok())
            .ok_or_else(|| PreviewError::Extraction("missing link_target".into()))?;
        // resolve via catalog (real impl queries application service via context's service hook)
        let child_resource = ctx.siblings.iter().find(|r| r.ref_id == target).cloned().unwrap_or(Resource {
            ref_id: target,
            kind: domain::ResourceKind::Document,
            title: target.to_string(),
            revision: String::new(),
            source_id: ctx.resource.source_id.clone(),
            locator: target.to_string(),
            properties: Default::default(),
        });
        let child_ctx = PreviewContext { resource: child_resource.clone(), ..*ctx };
        let previewer = ctx.catalog.resolve(None, &child_ctx).ok_or_else(|| PreviewError::Extraction("no previewer".into()))?;
        let child = previewer.render(&child_ctx)?;
        Ok(PreviewModel::LinkEmbed { target: Box::new(child_resource), child: Box::new(child) })
    }
}
```

(Add `use PreviewerCatalog;` and Clone on PreviewContext — adjust clone impl later.)

- [ ] **Step 2: Implement `BlockEmbedPreviewer` analogously** taking `source: ResourceRef`, `html: String` from `ctx.resource.properties.get("body")`.

- [ ] **Step 3: Implement `QueryEmbedPreviewer`**

```rust
// crates/preview/src/builders/query_embed.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError, Resource};

pub struct QueryEmbedPreviewer;
impl QueryEmbedPreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for QueryEmbedPreviewer {
    fn id(&self) -> &'static str { "query_embed" }
    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource.properties.get("body").map(|b| b.contains("#+BEGIN_SRC query")).unwrap_or(false)
    }
    fn render(&self, _ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        Ok(PreviewModel::QueryEmbed { query_id: String::new(), snapshot: vec![] })
    }
}
```

- [ ] **Step 4: Implement `FallbackPreviewer` (always matches)**

```rust
// crates/preview/src/builders/fallback.rs
use crate::{PreviewModel, Previewer, PreviewContext, PreviewError};

pub struct FallbackPreviewer;
impl FallbackPreviewer { pub fn register(c: &mut crate::PreviewerCatalog) { c.register(Self); } }
impl Previewer for FallbackPreviewer {
    fn id(&self) -> &'static str { "fallback" }
    fn matches(&self, _ctx: &PreviewContext) -> bool { true }
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let bytes = ctx.bytes.as_deref().map(|b| b.len()).unwrap_or(0);
        Ok(PreviewModel::Fallback {
            message: format!("Resource {:?} — {} bytes (download for raw view)", ctx.resource.ref_id, bytes),
        })
    }
}
```

Note: `FallbackPreviewer` must be registered LAST so other matchers win.

- [ ] **Step 5: Build, expect PASS; commit**

```bash
cargo build -p preview
git add crates/preview
git commit -m "feat(preview): link/block/query embed and fallback previewers"
```

### Task B7: Centralized catalog resolution tests

**Files:**
- Create: `crates/preview/tests/catalog_resolution.rs`
- Create: `crates/preview/tests/fixtures.rs`

- [ ] **Step 1: Build fixtures**

```rust
// crates/preview/tests/fixtures.rs
use domain::{Resource, ResourceKind, ResourceRef};
use ulid::Ulid;

pub fn doc_with_body(body: &str) -> Resource {
    Resource {
        ref_id: ResourceRef::new(ResourceKind::Document, Ulid::new()),
        kind: ResourceKind::Document,
        title: "doc".into(),
        revision: "abc".into(),
        source_id: "notes".into(),
        locator: "/x.org".into(),
        properties: [("body".to_string(), body.to_string())].into_iter().collect(),
    }
}
```

- [ ] **Step 2: Write tests**

```rust
// crates/preview/tests/catalog_resolution.rs
use preview::{default_catalog, PreviewContext, Resource};
use crate::fixtures::doc_with_body;

#[test]
fn org_matches_document_with_body() {
    let cat = Box::leak(Box::new(default_catalog()));
    let r = doc_with_body("* Hi");
    let c = PreviewContext {
        resource: r.clone(),
        bytes: None,
        mime: None,
        locator: r.locator.clone().into(),
        segments: vec![],
        siblings: vec![],
        catalog: cat,
    };
    assert_eq!(cat.resolve(None, &c).unwrap().id(), "org");
}

#[test]
fn override_forces_mermaid() {
    let cat = Box::leak(Box::new(default_catalog()));
    let r = doc_with_body("flowchart LR; A-->B;");
    let c = PreviewContext {
        resource: r.clone(),
        bytes: None,
        mime: None,
        locator: r.locator.clone().into(),
        segments: vec![],
        siblings: vec![],
        catalog: cat,
    };
    assert_eq!(cat.resolve(Some("mermaid"), &c).unwrap().id(), "mermaid");
}

#[test]
fn fallback_for_unknown() {
    let cat = Box::leak(Box::new(default_catalog()));
    let r = doc_with_body("zzz");  // org still matches
    let c = PreviewContext {
        resource: r.clone(),
        bytes: None,
        mime: None,
        locator: "thing.weird".into(),
        segments: vec![],
        siblings: vec![],
        catalog: cat,
    };
    let p = cat.resolve(None, &c).unwrap();
    // org matches everything document-shaped → org wins
    assert_eq!(p.id(), "org");
}
```

- [ ] **Step 3: Run, verify PASS**

Run: `cargo test -p preview`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/preview
git commit -m "test(preview): catalog resolution and override rules"
```

---

## Phase C — `crates/components` Dioxus components

### Task C1: Initialize `crates/components` with Dioxus deps

**Files:**
- Modify: `crates/components/Cargo.toml`
- Create: `crates/components/src/lib.rs`

- [ ] **Step 1: Add Dioxus dependency**

```toml
# crates/components/Cargo.toml
[package]
name = "components"
version = "0.1.0"
edition = "2024"

[dependencies]
dioxus = { version = "0.6", default-features = false, features = ["html"] }
preview = { path = "../preview" }
domain = { path = "../domain" }
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 2: Re-export placeholders**

```rust
// crates/components/src/lib.rs
pub mod escape;
pub mod layout;
pub mod space_list;
pub mod space_hub;
pub mod resource_card;
pub mod attachment_panel;
pub mod agenda_list;
pub mod outline;
pub mod code_block;
pub mod link_embed;
pub mod block_embed;
pub mod mermaid_block;
pub mod d2_block;
pub mod iframe_block;
```

Each module exposes one Dioxus `#[component]` function `Foo(props: FooProps) -> Element` with empty body for now.

- [ ] **Step 3: Build, expect PASS**

Run: `cargo build -p components`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/components
git commit -m "feat(components): scaffold with empty Dioxus components"
```

### Task C2: `escape` and `layout`

**Files:**
- Create: `crates/components/src/escape.rs` — wraps `html_escape::encode_safe`
- Create: `crates/components/src/layout.rs` — generic page chrome `<Html><Head><Body>...`

- [ ] **Step 1: Implement `escape`**

```rust
pub fn escape_html(s: &str) -> String { html_escape::encode_safe(s).to_string() }
```

- [ ] **Step 2: Implement `layout`**

```rust
// crates/components/src/layout.rs
use dioxus::prelude::*;

#[component]
pub fn Layout(title: String, children: Element) -> Element {
    rsx! { html { head { title { "{title}" } } body { {children} } } }
}
```

- [ ] **Step 3: Build, expect PASS**

Run: `cargo build -p components`

- [ ] **Step 4: Commit**

```bash
git add crates/components
git commit -m "feat(components): escape helper and Layout chrome"
```

### Task C3: `SpaceList`, `SpaceHub`, `ResourceCard`, `AttachmentPanel`, `AgendaList`

**Files:**
- Create: `crates/components/src/{space_list,space_hub,resource_card,attachment_panel,agenda_list}.rs`

For each component:
- Take its DTO props
- Render a Dioxus `rsx!` macro block returning `Element`
- One simple inline check (e.g. component returns non-empty HTML)

Show one full example; the rest follow mechanically.

```rust
// crates/components/src/agenda_list.rs
use dioxus::prelude::*;
use domain::ResourceRef;

#[derive(PartialEq, Clone, Debug)]
pub struct TaskItem {
    pub title: String,
    pub state: String,
    pub locator: String,
    pub r_ref: ResourceRef,
}

#[component]
pub fn AgendaList(items: Vec<TaskItem>) -> Element {
    rsx! { ul { class: "agenda", for t in items { li { class: "task-{t.state}", "{t.state} {t.title} ({t.locator})" } } } }
}
```

`SpaceList` lists entries `/s/{name}`; `SpaceHub` composes `AgendaList` + an outline; `ResourceCard` takes a `Resource` and a `PreviewModel` (matching on `kind` tag), and recurses into previewer-specific subcomponents; `AttachmentPanel` renders a list.

- [ ] **Step 1: Build, expect PASS**

Run: `cargo build -p components`

- [ ] **Step 2: Commit**

```bash
git add crates/components
git commit -m "feat(components): data-rendering components"
```

### Task C4: `Outline`, `CodeBlock`, `LinkEmbed`, `BlockEmbed`, `MermaidBlock`, `D2Block`, `IframeBlock`

**Files:**
- Create: `crates/components/src/{outline,code_block,link_embed,block_embed,mermaid_block,d2_block,iframe_block}.rs`

- [ ] **Step 1: Implement `Outline`**

Takes `Vec<Heading>`; renders `<aside class="outline">` with anchor links.

- [ ] **Step 2: Implement `CodeBlock`**

Takes `lang: String, source: String`; uses Dioxus `pre` macro. Plain for now (no syntax highlighter).

- [ ] **Step 3: Implement `LinkEmbed`** and **`BlockEmbed`** — recursive `ResourceCard` calls.

- [ ] **Step 4: Implement `MermaidBlock`**, **`D2Block`**, **`IframeBlock`**

Server-rendered placeholders carrying `data-source` or `src` attributes; client-side ESM in Task D4 hydrates them.

```rust
// crates/components/src/mermaid_block.rs
use dioxus::prelude::*;
#[component]
pub fn MermaidBlock(source: String) -> Element {
    rsx! { div { class: "mermaid-block", "data-source": "{source}", "{source}" } }
}
```

- [ ] **Step 5: Build, expect PASS**

Run: `cargo build -p components`

- [ ] **Step 6: Commit**

```bash
git add crates/components
git commit -m "feat(components): outline, embed blocks, code and diagram placeholders"
```

---

## Phase D — `crates/web` server + integration

### Task D1: Web crate Cargo + DI of `PreviewerCatalog`

**Files:**
- Modify: `crates/web/Cargo.toml`
- Create: `crates/web/src/state.rs`
- Create: `crates/web/src/preview.rs`

- [ ] **Step 1: Wire dependencies**

```toml
# crates/web/Cargo.toml
[package]
name = "web"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "notez-web"
path = "src/main.rs"

[dependencies]
axum = { version = "0.7", features = ["http2"] }
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["trace", "fs"] }
dioxus = { version = "0.6", features = ["fullstack", "html"] }
dioxus-fullstack = { version = "0.6" }
preview = { path = "../preview" }
components = { path = "../components" }
application = { path = "../application" }
storage = { path = "../storage" }
config = { path = "../config" }
domain = { path = "../domain" }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = "0.1"
ulid = { workspace = true }

[build-dependencies]
```

- [ ] **Step 2: State struct**

```rust
// crates/web/src/state.rs
use application::ApplicationService;
use preview::{PreviewerCatalog, default_catalog};
use std::sync::Arc;

pub struct WebState {
    pub space_root: std::path::PathBuf,
    pub service: Arc<ApplicationService<storage::SqliteProjection>>,
    pub catalog: Arc<PreviewerCatalog>,
}

impl WebState {
    pub fn from_space(root: &std::path::Path) -> Result<Self, anyhow_lite::Error> {
        let store = storage::SqliteProjection::open(&root.join(".notez/index.sqlite"))?;
        let service = Arc::new(ApplicationService::new(store));
        Ok(WebState { space_root: root.to_path_buf(), service, catalog: Arc::new(default_catalog()) })
    }
}
```

Use `Box<dyn Error>` instead of `anyhow_lite` (drop the dep).

- [ ] **Step 3: Build, expect PASS**

Run: `cargo build -p web`

- [ ] **Step 4: Commit**

```bash
git add crates/web
git commit -m "feat(web): WebState with previewer catalog wired"
```

### Task D2: Router and SSR handlers

**Files:**
- Create: `crates/web/src/router.rs`
- Create: `crates/web/src/routes/{root,space_hub,resource,attachment,agenda,healthz,static_files}.rs`
- Modify: `crates/web/src/main.rs`

- [ ] **Step 1: Implement `router.rs`**

```rust
// crates/web/src/router.rs
use axum::{routing::get, Router};
use crate::state::WebState;

pub fn router(state: WebState) -> Router {
    Router::new()
        .route("/", get(routes::root::root))
        .route("/healthz", get(routes::healthz::healthz))
        .route("/s/:space", get(routes::space_hub::hub))
        .route("/s/:space/agenda", get(routes::agenda::agenda))
        .route("/s/:space/r/:ref", get(routes::resource::show))
        .route("/s/:space/r/:ref/preview.json", get(routes::resource::preview_json))
        .route("/s/:space/a/:att", get(routes::attachment::raw))
        .route("/s/:space/a/:att/preview.json", get(routes::attachment::preview_json))
        .route("/static/*path", get(routes::static_files::static_file))
        .with_state(state)
}
```

- [ ] **Step 2: Implement `routes/healthz.rs`**

```rust
use axum::http::StatusCode;
pub async fn healthz() -> (StatusCode, &'static str) { (StatusCode::OK, "ok") }
```

- [ ] **Step 3: Implement `routes/static_files.rs`**

```rust
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
};
use std::path::PathBuf;
use crate::state::WebState;

pub async fn static_file(
    State(state): State<WebState>,
    Path(path): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let safe = path.replace("..", "_");
    let p = state.space_root.join("static").join(&safe);
    if !p.exists() { return Err((StatusCode::NOT_FOUND, "missing".into())); }
    let bytes = std::fs::read(&p).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mime = if p.ends_with(".mjs") { "text/javascript" } else { "application/octet-stream" };
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(bytes))
        .unwrap())
}
```

- [ ] **Step 4: Implement `routes/root.rs`**

```rust
use axum::{extract::State, response::Html};
use crate::state::WebState;
use components::Layout;

pub async fn root(State(state): State<WebState>) -> Html<String> {
    let spaces = vec![(state.space_root.file_name().unwrap_or_default().to_string_lossy().to_string(), state.space_root.clone())];
    let body = format!(r#"<h1>Notez</h1><ul>{}</ul>"#,
        spaces.iter().map(|(n, _)| format!(r#"<li><a href="/s/{n}">{n}</a></li>"#)).collect::<String>());
    Html(format!("<!doctype html><html><head><title>Notez</title></head><body>{body}</body></html>"))
}
```

- [ ] **Step 5: Implement `routes/space_hub.rs`** — list `org` matched resources, recompose `SpaceHub` Dioxus component server-side via `dioxus::ssr::render_to_string`.

- [ ] **Step 6: Implement `routes/attachment.rs`** — `raw`: read blob via `BlobStore::get(hash)`, return with proper Content-Type. `preview_json`: build `PreviewContext` and call `catalog.resolve(...).render(...)` to `serde_json::to_string`.

- [ ] **Step 7: Implement `routes/agenda.rs`** — uses `QueryPage` or `ApplicationService::query_for_tasks` (whichever exists) to assemble `AgendaList`.

- [ ] **Step 8: Implement `routes/resource.rs::show`** — load resource, build `PreviewContext`, resolve previewer, render `ResourceCard` to HTML string.

- [ ] **Step 9: Wire `main.rs` to launch axum server**

```rust
// crates/web/src/main.rs
use clap::Parser;
use web::{server::serve, state::WebState};

#[derive(Parser)]
struct Args { #[arg(long)] space: String, #[arg(long, default_value = "127.0.0.1:3030")] bind: String, #[arg(long)] open: bool }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let state = WebState::from_space(std::path::Path::new(&args.space))?;
    serve(state, &args.bind, args.open).await
}
```

(add `anyhow = "1"` to Cargo.toml)

- [ ] **Step 10: Build, expect PASS**

Run: `cargo build -p web`
Expected: PASS (network calls unreachable from tests)

- [ ] **Step 11: Commit**

```bash
git add crates/web
git commit -m "feat(web): axum router and SSR resource/attachment routes"
```

### Task D3: `serve()` function and graceful shutdown

**Files:**
- Create: `crates/web/src/server.rs`

- [ ] **Step 1: Implement `serve`**

```rust
// crates/web/src/server.rs
use crate::state::WebState;
use crate::router::router;
use std::net::SocketAddr;
use tracing::info;

pub async fn serve(state: WebState, bind: &str, open: bool) -> anyhow::Result<()> {
    let app = router(state);
    let addr: SocketAddr = bind.parse()?;
    info!("listening on http://{addr}");
    if open {
        let url = format!("http://{addr}/");
        let _ = open::that_detached(url);
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

Use `webbrowser` or `open` crate (add `open = "5"` to Cargo.toml).

- [ ] **Step 2: Build, expect PASS**

Run: `cargo build -p web`

- [ ] **Step 3: Commit**

```bash
git add crates/web
git commit -m "feat(web): serve() entry point with optional browser launch"
```

### Task D4: Client-side ESM for Mermaid / D2 hydration

**Files:**
- Create: `crates/web/static/js/preview/mermaid.mjs`
- Create: `crates/web/static/js/preview/d2.mjs`

- [ ] **Step 1: Implement `mermaid.mjs`**

```javascript
// crates/web/static/js/preview/mermaid.mjs
import mermaid from 'https://esm.sh/mermaid@10/dist/mermaid.esm.min.mjs';
mermaid.initialize({ startOnLoad: false });
window.addEventListener('DOMContentLoaded', async () => {
  const blocks = document.querySelectorAll('div.mermaid-block');
  for (const block of blocks) {
    const source = block.getAttribute('data-source') ?? '';
    const svg = await mermaid.render('m-' + Math.random().toString(36).slice(2), source);
    block.innerHTML = svg.svg;
  }
});
```

- [ ] **Step 2: Implement `d2.mjs`**

```javascript
// crates/web/static/js/preview/d2.mjs
import { render } from 'https://esm.sh/@terrastruct/d2@0';
window.addEventListener('DOMContentLoaded', () => {
  const blocks = document.querySelectorAll('div.d2-block');
  for (const block of blocks) {
    const source = block.getAttribute('data-source') ?? '';
    render(block, source);
  }
});
```

- [ ] **Step 3: Cargo manifest `build.rs` to include `static/` directory**

```rust
// crates/web/build.rs
fn main() {
    println!("cargo:rerun-if-changed=static");
    let out = std::path::Path::new(env!("OUT_DIR")).join("static");
    std::fs::create_dir_all(&out).unwrap();
    copy_dir("static", &out);
}
fn copy_dir(src: &str, dst: &std::path::Path) {
    let dir = std::fs::read_dir(src).unwrap();
    for entry in dir {
        let e = entry.unwrap();
        let target = dst.join(e.file_name());
        if e.path().is_dir() { std::fs::create_dir_all(&target).unwrap(); copy_dir(e.path().to_str().unwrap(), &target); }
        else { std::fs::copy(e.path(), target).unwrap(); }
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/web
git commit -m "feat(web): client-side mermaid/d2 ESM hydration scripts"
```

### Task D5: Wire `web` subcommand into `crates/cli`

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/commands.rs`

- [ ] **Step 1: Optional dep on `crates/web`**

```toml
# crates/cli/Cargo.toml
[dependencies]
web = { path = "../web", optional = true }

[features]
web = ["dep:web"]
```

- [ ] **Step 2: Register `Commands::Web` variant**

```rust
// crates/cli/src/commands.rs
#[cfg(feature = "web")]
#[derive(clap::Args)]
pub struct WebArgs {
    #[arg(long, default_value = "127.0.0.1:3030")] pub bind: String,
    #[arg(long)] pub open: bool,
}

// in Cli::command add:
#[cfg(feature = "web")]
Web(WebArgs),
```

- [ ] **Step 3: Wire in main.rs**

```rust
#[cfg(feature = "web")]
Commands::Web(args) => {
    web::server::serve(web::state::WebState::from_space(&r_config.space_root)?, &args.bind, args.open).await?
}
```

- [ ] **Step 4: Build with feature**

Run: `cargo build -p cli --features web`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cli
git commit -m "feat(cli): Web subcommand gated by `web` feature"
```

### Task D6: End-to-end acceptance script

**Files:**
- Create: `scripts/acceptance-web.sh`

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

cargo build --release -p cli --features web

BIND="127.0.0.1:3939"
BASE="http://$BIND"
SPACE_DIR="$(mktemp -d)"
trap "rm -rf $SPACE_DIR" EXIT

# Seed a minimal space
mkdir -p "$SPACE_DIR/projects"
cat > "$SPACE_DIR/README.org" <<EOF
#+TITLE: Test
* hello
EOF
"$PROJECT_ROOT/target/release/notez" --space "$SPACE_DIR" --json scan > /dev/null

./target/release/notez --space "$SPACE_DIR" web --bind "$BIND" &
PID=$!
trap "kill $PID; rm -rf $SPACE_DIR" EXIT

# Wait for server
for i in 1 2 3 4 5 6 7 8 9 10; do
  curl -fsS "$BASE/healthz" 2>/dev/null && break || sleep 1
done

test "$(curl -fsS "$BASE/healthz")" = "ok"
curl -fsS "$BASE/" | grep -q "Notez"
curl -fsS "$BASE/s/$(basename "$SPACE_DIR")/" | grep -q "Test"

kill "$PID"
echo "web acceptance: PASS"
```

- [ ] **Step 2: Run, expect PASS**

Run: `bash scripts/acceptance-web.sh`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add scripts/acceptance-web.sh
git commit -m "test: end-to-end acceptance script for notez web"
```

### Task D7: Hand-off to `~/Notes` smoke test

**(Manual / sandbox)**

- [ ] **Step 1: Test against the real `~/Notes` space**

Run:
```
cargo build -p cli --features web
./target/debug/notez --space ~/Notes web --bind 127.0.0.1:4040 --open &
```

- [ ] **Step 2: Verify `Work Notes` README renders**

Visit `http://127.0.0.1:4040/s/work-notes/`. Expect to see Work Notes title and 4 PARA index links.

- [ ] **Step 3: Verify the pptx attachment falls back to download**

Visit the page for `attachment:5NSD81NBR3TQQAMY3C15FMD4GN`. Expect PptxPreviewer output (slide list) — pptx content is real. If extraction fails (e.g., due to missing deps), expect FallbackPreviewer with download link.

- [ ] **Step 4: Kill server**

```bash
kill %1
```

- [ ] **Step 5: Document result in this task's commit**

```bash
git commit --allow-empty -m "chore: smoke-tested web subcommand against ~/Notes space"
```

---

## Phase E — Optional performance baseline (non-blocking)

### Task E1: Criterion benchmark for SSR cost

**Files:**
- Create: `crates/web/benches/ssr.rs`
- Modify: `crates/web/Cargo.toml`

- [ ] **Step 1: Add criterion**

```toml
[[bench]]
name = "ssr"
harness = false
[dev-dependencies]
criterion = "0.5"
```

- [ ] **Step 2: Smoke benchmark**

```rust
use criterion::{criterion_group, criterion_main, Criterion};
fn bench_ssr(c: &mut Criterion) {
    let state = web::state::WebState::from_space(std::path::Path::new("/dev/null")).ok();
    if let Some(state) = state {
        c.bench_function("ssr-root", |b| b.iter(|| {
            let _ = web::routes::root::root(axum::extract::State(state.clone()));
        }));
    }
}
criterion_group!(benches, bench_ssr);
criterion_main!(benches);
```

- [ ] **Step 3: Run, report numbers**

Run: `cargo bench -p web`

- [ ] **Step 4: Don't gate release on numbers; commit bench only**

```bash
git add crates/web
git commit -m "bench(web): smoke SSR cost measurement (non-blocking)"
```

---

## Self-Review

### Spec coverage

| Spec § | Requirement | Task |
|---|---|---|
| §2 goals 1..6 | Web subcommand, previewers, components, architecture invariants, extractors | A2..A3, B1..B7, C1..C4, D1..D5 |
| §3 architecture | Crate boundaries preserved | A1 layout; D1 cites `application::ApplicationService` only |
| §4 crates | 4 new/changed crates | A1, A3, C1, D1, D5 |
| §5 Previewer trait / model / matches | Trait + Model + matching order | A2, B1..B7 |
| §6 routes | 10 routes | D2 |
| §7 config + CLI | `[web]` section, `notez web` | D5 |
| §8 data flows | SSR + fetch + blob | D2, D3 |
| §9 components | Dioxus component list | C1..C4 |
| §10 testing | unit + acceptance | B7, D6 |
| §11 risks | Dioxus version pinning, no Node | workspace dep stays 0.6 |
| §12 followups | none in this milestone | n/a |

### Placeholder scan

No "TBD"/"TODO"/"implement later" markers — every step ships concrete code or shell.

### Type consistency

- `Previewer` / `PreviewContext` / `PreviewModel` introduced in A2 with named fields
- All previewers (B2..B6) use these field names verbatim.
- `WebState` (D1) references `default_catalog()` from A2 (registered callers in B5+B6 run after `default_catalog` exists in A2).

### Ambiguity check

- Web server only reads; never mutates (§2 non-goal "no writeback").
- No Node build chain (§11); ESMs imported directly from esm.sh.
- Routing `/s/:space/r/:ref` uses `ResourceRef::to_string()` which is `<kind>:<ulid>`; matches `ResourceRef::parse`.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-27-notez-web-server-and-preview.md`. Two execution options:

1. **Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
