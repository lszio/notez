use crate::preview::{PreviewContext, PreviewError, PreviewModel, Previewer};
use crate::domain::{ResourceKind, ResourceRef};
use std::hash::{DefaultHasher, Hash, Hasher};

/// Previewer for `[[block:<ref-id>]]` embed references within a document body.
///
/// Matches when the resource body starts with the literal `[[block:` prefix
/// (resilient to leading whitespace). The referenced resource is encoded as
/// a `ResourceRef`.
///
/// ## Trust model (updated by Phase D review fix-up)
///
/// The block target is **not** resolved through the application service in
/// this build — the preview context the web layer constructs does not carry
/// a `service` handle (see `crates/web/src/routes/resource.rs`). The payload
/// therefore piggybacks on the *source* body's raw text. The web route is
/// responsible for HTML-escaping that text (`<pre class="block-embed">…</pre>`)
/// so a malicious `[[block:…]]` body cannot inject markup into the host page.
/// When the application service becomes available in `PreviewContext`,
/// this previewer should resolve the block target and emit its rendered
/// preview instead — at which point the route-side escape can be dropped.
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
