use crate::preview::{PreviewContext, PreviewError, PreviewModel, Previewer};
use pulldown_cmark::{html::push_html, Options, Parser};

/// Previewer for Markdown documents.
///
/// Matches on either the declared MIME (`text/markdown`) or a `.md` locator
/// suffix. Body is read from `ctx.resource.properties["body"]`.
pub struct MarkdownPreviewer;

impl Previewer for MarkdownPreviewer {
    fn id(&self) -> &'static str {
        "markdown"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.mime.as_deref() == Some("text/markdown")
            || ctx.locator.to_string_lossy().ends_with(".md")
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let body = ctx
            .resource
            .properties
            .get("body")
            .cloned()
            .unwrap_or_default();
        let mut out = String::with_capacity(body.len());
        push_html(&mut out, Parser::new_ext(&body, Options::all()));
        Ok(PreviewModel::Markdown { html: out })
    }
}
