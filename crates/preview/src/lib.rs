use application::ApplicationService;
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

#[derive(Clone)]
pub struct PreviewContext<'a> {
    pub resource: Resource,
    pub bytes: Option<bytes::Bytes>,
    pub mime: Option<String>,
    pub locator: PathBuf,
    pub segments: Vec<SegmentRecord>,
    pub siblings: Vec<Resource>,
    pub catalog: &'a PreviewerCatalog,
    pub service: Option<&'a ApplicationService<storage::SqliteProjection>>,
}

pub trait Previewer: Send + Sync {
    fn id(&self) -> &'static str;
    fn matches(&self, ctx: &PreviewContext) -> bool;
    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError>;
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryRequest {
    pub source: String,
    pub kind_hint: Option<String>,
    pub title_contains: Option<String>,
    pub limit: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    QueryEmbed { query: QueryRequest, snapshot: Vec<Resource> },
    BlockEmbed{ source: ResourceRef, html: String },
    Fallback  { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heading {
    pub level: u8,
    pub title: String,
    pub anchor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPage {
    pub index: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slide {
    pub index: u32,
    pub title: Option<String>,
    pub body: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Build the canonical `PreviewerCatalog` containing all 14 built-in
/// previewers.
///
/// The registration order is deliberate and matters: each entry is
/// consulted in turn by `PreviewerCatalog::resolve` and the first match
/// wins. Registration groups:
///
/// 1. Markdown                                                      (matches: `.md` / `text/markdown`)
/// 2. Mermaid                                                       (matches: `body` contains "mermaid")
/// 3. D2                                                            (matches: `body` contains "d2")
/// 4. Iframe                                                        (matches: `body` starts with `[[iframe:`)
/// 5. BlockEmbed                                                    (matches: `body` starts with `[[block:`)
/// 6. QueryEmbed                                                    (matches: `body` contains `#+BEGIN_SRC query`)
/// 7. Pdf                                                           (matches: `.pdf` / `application/pdf`)
/// 8. Xlsx                                                          (matches: `.xlsx` / xlsx MIME)
/// 9. Pptx                                                          (matches: `.pptx` / pptx MIME)
/// 10. Zip                                                          (matches: `.zip` / zip MIME)
/// 11. Image                                                        (matches: `image/*`)
/// 12. Org                                                          (matches: Document / Heading / Block)
/// 13. LinkEmbed                                                    (override-only — `matches` always false)
/// 14. Fallback                                                     (matches: always)
///
/// Notes:
/// - The abstract previewers (Mermaid/D2/Iframe/BlockEmbed/QueryEmbed) are
///   registered BEFORE Org because Org matches every document-shaped
///   resource and would otherwise always win.
/// - `LinkEmbedPreviewer` is override-only by design, so its position does
///   not affect match-order resolution.
/// - `FallbackPreviewer` is registered LAST so every other previewer gets
///   a chance to claim the context first.
pub fn default_catalog() -> PreviewerCatalog {
    use builders::{
        block_embed::BlockEmbedPreviewer,
        d2::D2Previewer,
        fallback::FallbackPreviewer,
        iframe::IframePreviewer,
        image::ImagePreviewer,
        link_embed::LinkEmbedPreviewer,
        markdown::MarkdownPreviewer,
        mermaid::MermaidPreviewer,
        org::OrgPreviewer,
        pdf::PdfPreviewer,
        pptx::PptxPreviewer,
        query_embed::QueryEmbedPreviewer,
        xlsx::XlsxPreviewer,
        zip::ZipPreviewer,
    };

    let mut c = PreviewerCatalog::new();
    c.register(MarkdownPreviewer);
    c.register(MermaidPreviewer);
    c.register(D2Previewer);
    c.register(IframePreviewer);
    c.register(BlockEmbedPreviewer);
    c.register(QueryEmbedPreviewer);
    c.register(PdfPreviewer);
    c.register(XlsxPreviewer);
    c.register(PptxPreviewer);
    c.register(ZipPreviewer);
    c.register(ImagePreviewer);
    c.register(OrgPreviewer);
    c.register(LinkEmbedPreviewer);
    c.register(FallbackPreviewer);
    c
}

pub mod builders;