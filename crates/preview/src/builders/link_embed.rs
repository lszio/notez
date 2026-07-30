use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};
use domain::{Resource, ResourceKind, ResourceRef};
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
/// The result is rendered recursively via the catalog and emitted as a
/// `PreviewModel::LinkEmbed { target, child }` payload.
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

        let child_resource = ctx
            .siblings
            .iter()
            .find(|r| r.r#ref == target_ref)
            .cloned()
            .unwrap_or_else(|| Resource {
                r#ref: target_ref.clone(),
                kind: ResourceKind::Document,
                title: target_ref.to_string(),
                revision: String::new(),
                source_id: ctx.resource.source_id.clone(),
                locator: target_ref.to_string(),
                properties: BTreeMap::new(),
            });

        let child_ctx = PreviewContext {
            resource: child_resource.clone(),
            locator: std::path::PathBuf::from(child_resource.locator.clone()),
            ..ctx.clone()
        };

        let previewer = ctx
            .catalog
            .resolve(None, &child_ctx)
            .ok_or_else(|| PreviewError::Extraction("no child previewer".into()))?;
        let child = previewer.render(&child_ctx)?;

        Ok(PreviewModel::LinkEmbed {
            target: child_resource,
            child: Box::new(child),
        })
    }
}
