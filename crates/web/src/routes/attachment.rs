//! `/s/:space/a/:att` — attachment endpoints.
//!
//!   - `raw`:        streams the attachment blob with the right MIME type.
//!   - `preview_json`: builds a `PreviewContext` and returns the matched
//!                    `PreviewModel` as JSON.
//!
//! Attachments are addressed by their `ResourceRef` (`attachment:<ulid>`);
//! the blob bytes live on disk under `<space>/.notez/blobs/<hash>`.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use domain::ResourceRef;
use preview::PreviewContext;
use storage::{BlobMeta, BlobStore};

use crate::state::WebState;

#[derive(serde::Deserialize)]
pub struct AttachmentParams {
    pub space: String,
    pub att: String,
}

pub async fn raw(
    State(state): State<WebState>,
    Path(p): Path<AttachmentParams>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let parsed = ResourceRef::parse(&p.att)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad att: {e}")))?;
    let resource = state
        .service
        .read(&parsed)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "attachment not found".into()))?;

    let hash = resource
        .properties
        .get("blob_hash")
        .or_else(|| resource.properties.get("HASH"))
        .ok_or_else(|| (StatusCode::NOT_FOUND, "no blob hash".into()))?
        .clone();
    let store = BlobStore::new(&state.space_root);
    let bytes = store
        .get(&hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "blob missing".into()))?;
    let meta = BlobMeta {
        hash: hash.clone(),
        size_bytes: bytes.len() as u64,
        mime_type: resource
            .properties
            .get("mime_type")
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".into()),
    };
    Ok(attachment_response(meta, bytes))
}

pub async fn preview_json(
    State(state): State<WebState>,
    Path(p): Path<AttachmentParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let parsed = ResourceRef::parse(&p.att)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("bad att: {e}")))?;
    let resource = state
        .service
        .read(&parsed)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "attachment not found".into()))?;

    let (bytes, mime) = load_blob(&state, &resource)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "blob missing".into()))?;
    let ctx = PreviewContext {
        resource: resource.clone(),
        bytes: Some(bytes::Bytes::from(bytes)),
        mime: Some(mime.clone()),
        locator: std::path::PathBuf::from(&resource.locator),
        segments: Vec::new(),
        siblings: Vec::new(),
        catalog: &state.catalog,
        service: None,
    };
    let previewer = state
        .catalog
        .resolve(None, &ctx)
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "no previewer".into()))?;
    let model = previewer
        .render(&ctx)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("render: {e}")))?;
    let json = serde_json::to_string(&model)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((
        StatusCode::OK,
        [("content-type", "application/json")],
        json,
    ))
}

fn load_blob(state: &WebState, resource: &domain::Resource) -> Option<(Vec<u8>, String)> {
    let hash = resource
        .properties
        .get("blob_hash")
        .or_else(|| resource.properties.get("HASH"))?
        .clone();
    let store = BlobStore::new(&state.space_root);
    let bytes = store.get(&hash).ok().flatten()?;
    let mime = resource
        .properties
        .get("mime_type")
        .cloned()
        .unwrap_or_else(|| "application/octet-stream".into());
    Some((bytes, mime))
}

fn attachment_response(meta: BlobMeta, bytes: Vec<u8>) -> Response<Body> {
    Response::builder()
        .header(header::CONTENT_TYPE, meta.mime_type)
        .header(header::CONTENT_LENGTH, meta.size_bytes)
        .body(Body::from(bytes))
        .unwrap()
}