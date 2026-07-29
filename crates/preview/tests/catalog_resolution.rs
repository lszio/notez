use preview::{PreviewContext, Previewer, PreviewerCatalog};
use domain::{Resource, ResourceKind};
use storage::SqliteProjection;
use std::path::PathBuf;

struct Org;
struct Fallback;
impl Previewer for Org { fn id(&self)->&'static str { "org" } fn matches(&self,c:&PreviewContext)->bool { c.resource.kind == ResourceKind::Document } fn render(&self,_:&PreviewContext)->Result<preview::PreviewModel,preview::PreviewError>{unimplemented!()} }
impl Previewer for Fallback { fn id(&self)->&'static str { "fallback" } fn matches(&self,_:&PreviewContext)->bool { true } fn render(&self,_:&PreviewContext)->Result<preview::PreviewModel,preview::PreviewError>{unimplemented!()} }
fn ctx<'a>(catalog:&'a PreviewerCatalog)->PreviewContext<'a>{ PreviewContext { resource: Resource { r#ref: domain::ResourceRef::new(ResourceKind::Document, ulid::Ulid::new()), kind: ResourceKind::Document, title: "x".into(), revision: "1".into(), source_id: "s".into(), locator: "x".into(), properties: Default::default() }, bytes: None, mime: None, locator: PathBuf::new(), segments: vec![], siblings: vec![], catalog, service: None } }
#[test] fn org_matches_document_body(){ let mut c=PreviewerCatalog::new(); c.register(Org); c.register(Fallback); assert_eq!(c.resolve(None,&ctx(&c)).unwrap().id(),"org"); }
#[test] fn override_id_forces_choice(){ let mut c=PreviewerCatalog::new(); c.register(Org); c.register(Fallback); assert_eq!(c.resolve(Some("fallback"),&ctx(&c)).unwrap().id(),"fallback"); }
#[test] fn attachment_unknown_locator_falls_back(){ let mut c=PreviewerCatalog::new(); c.register(Fallback); assert_eq!(c.resolve(None,&ctx(&c)).unwrap().id(),"fallback"); }
