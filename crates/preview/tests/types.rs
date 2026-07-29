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
    // Reference types so unused imports remain harmless.
    let _r: Option<Resource> = None;
    let _k: Option<ResourceKind> = None;
    let _rr: Option<ResourceRef> = None;
    let _b: Option<Bytes> = None;
    let _ctx: Option<PreviewContext> = None;
    let _err: Option<PreviewError> = None;
}