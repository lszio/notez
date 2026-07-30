use crate::{PreviewContext, PreviewError, PreviewModel, Previewer};

/// Previewer for `[[iframe:<url>]]` embed references.
///
/// Matches when the resource body starts with the literal `[[iframe:` prefix
/// (resilient to leading whitespace). The URL is parsed out of the body and
/// emitted as a `PreviewModel::Iframe { src, sandbox }` payload. The default
/// sandbox is `allow-scripts`; an explicit `sandbox=<value>` after the URL
/// (separated by a space) overrides it.
pub struct IframePreviewer;

impl Previewer for IframePreviewer {
    fn id(&self) -> &'static str {
        "iframe"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource
            .properties
            .get("body")
            .map(|b| b.trim_start().starts_with("[[iframe:"))
            .unwrap_or(false)
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let body = ctx
            .resource
            .properties
            .get("body")
            .cloned()
            .unwrap_or_default();
        // Strip the `[[iframe:` prefix and the trailing `]]` suffix.
        // Trim first so that trailing whitespace does not defeat the
        // `trim_end_matches("]]")` call.
        let inner = body
            .trim()
            .trim_start_matches("[[iframe:")
            .trim_end_matches("]]")
            .trim();

        // Optional `sandbox=<value>` suffix, separated by whitespace.
        let (src, sandbox) = match inner.split_once(char::is_whitespace) {
            Some((url, rest)) => {
                let sandbox = rest
                    .strip_prefix("sandbox=")
                    .unwrap_or("allow-scripts")
                    .to_string();
                (url.to_string(), sandbox)
            }
            None => (inner.to_string(), "allow-scripts".to_string()),
        };

        Ok(PreviewModel::Iframe { src, sandbox })
    }
}
