use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};
use domain::{ResourceKind, ResourceRef};
use std::hash::{DefaultHasher, Hash, Hasher};

/// Previewer for `[[block:<ref-id>]]` embed references within a document body.
///
/// Matches when the resource body starts with the literal `[[block:` prefix
/// (resilient to leading whitespace). The referenced resource is encoded as
/// a `ResourceRef`; the rendered HTML is the raw body of the source resource
/// (the `BlockEmbed` payload is a placeholder for now — a future iteration
/// will resolve the block target via the application service and render the
/// referenced block directly).
pub struct BlockEmbedPreviewer;

impl Previewer for BlockEmbedPreviewer {
    fn id(&self) -> &'static str {
        "block_embed"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource
            .properties
            .get("body")
            .map(|b| b.trim_start().starts_with("[[block:"))
            .unwrap_or(false)
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let body = ctx
            .resource
            .properties
            .get("body")
            .cloned()
            .unwrap_or_default();

        let inner = body
            .trim()
            .trim_start_matches("[[block:")
            .trim_end_matches("]]")
            .trim();

        // The inner form is `<ref-id>` or `<ref-id> <custom-label>`. We only
        // need the ref-id for the payload; the rest is opaque to the client.
        let ref_id_str = inner.split_whitespace().next().unwrap_or(inner);

        let source = ResourceRef::parse(ref_id_str).unwrap_or_else(|_| {
            // Best-effort fallback when the ref isn't in canonical
            // `<kind>:<ulid>` form: synthesise a stable `ResourceRef` of
            // kind `Block` keyed by a hash of the original string.
            let mut h = DefaultHasher::new();
            ref_id_str.hash(&mut h);
            let v = h.finish();
            ResourceRef::new(
                ResourceKind::Block,
                ulid::Ulid::from(u128::from(v).to_be_bytes()),
            )
        });

        Ok(PreviewModel::BlockEmbed {
            source,
            html: body,
        })
    }
}
