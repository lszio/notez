use notez_core::domain::{Resource, ResourceKind, ResourceRef};
use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};
use std::collections::BTreeMap;

/// Previewer that renders a linked-target resource as a child preview.
///
/// This previewer is only reachable via the explicit `?preview=link_embed`
/// override — `matches` always returns `false` so the catalog never picks it
/// through the default match-order. The target resource is read from the
/// `link_target` property of the source resource (a serialised `ResourceRef`)
/// and resolved through the catalog:
/// - If the target appears in `ctx.siblings`, the sibling is used as the
///   child resource.
/// - Otherwise a synthetic `Resource` is constructed from the ref so the
///   catalog can still attempt to render it.
///
/// The rendered `PreviewModel::LinkEmbed { target, child }` payload is
/// recursively rendered via the catalog and emitted as the preview HTML.
///
/// ## Phase D limitation
///
/// `PreviewContext::service` is `None` for handlers built on the current
/// `WebState` (see `crates/web/src/send.rs`), so this previewer cannot
/// resolve cross-resource targets by hitting the projection. It only
/// Full target resolution is owned by the Engine and supplied as serialized
/// preview context; preview builders never depend on storage.
pub struct LinkEmbedPreviewer;

impl Previewer for LinkEmbedPreviewer {
    fn id(&self) -> &'static str {
        "link_embed"
    }

    fn matches(&self, _ctx: &PreviewContext) -> bool {
        false
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let target_raw = ctx
            .resource
            .properties
            .get("link_target")
            .ok_or_else(|| PreviewError::Extraction("missing link_target".into()))?;

        let target_ref: ResourceRef = serde_json::from_str(target_raw)
            .map_err(|e| PreviewError::Extraction(format!("link_target json: {e}")))?;

        // Resolve the target to a concrete Resource we own: either a sibling
        // already present in `ctx.siblings`, or a synthetic Document derived
        // from the ref. We then move it directly into the child context and
        // re-borrow it for the returned `target` field — avoiding a second
        // clone of the (potentially large) `Resource` value.
        let target_resource =
            if let Some(sibling) = ctx.siblings.iter().find(|r| r.r#ref == target_ref).cloned() {
                sibling
            } else {
                Resource {
                    r#ref: target_ref.clone(),
                    kind: ResourceKind::Document,
                    title: target_ref.to_string(),
                    revision: String::new(),
                    source_id: ctx.resource.source_id.clone(),
                    locator: target_ref.to_string(),
                    properties: BTreeMap::new(),
                    object_id: notez_core::domain::derived_object_id("", "", ""),
                    primary_source_id: String::new(),
                }
            };

        let child_locator = std::path::PathBuf::from(target_resource.locator.clone());
        let child_ctx = PreviewContext {
            resource: target_resource,
            locator: child_locator,
            ..ctx.clone()
        };

        let previewer = ctx
            .catalog
            .resolve(None, &child_ctx)
            .ok_or_else(|| PreviewError::Extraction("no child previewer".into()))?;
        let child = previewer.render(&child_ctx)?;

        Ok(PreviewModel::LinkEmbed {
            target: child_ctx.resource.clone(),
            child: Box::new(child),
        })
    }
}
