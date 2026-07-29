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