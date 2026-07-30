use crate::{PreviewContext, PreviewError, PreviewModel, Previewer, QueryRequest};

/// Previewer for `#+BEGIN_SRC query` source blocks.
///
/// Matches when the resource body contains the canonical Org source-block
/// marker `#+BEGIN_SRC query`. The body is parsed by scanning single
/// `KEY: VALUE` lines that follow the opening marker and using them to
/// populate a `QueryRequest`. The `snapshot` field is always empty for now
/// because dynamic query execution is a later milestone — the previewer only
/// captures the request the user has written.
pub struct QueryEmbedPreviewer;

impl Previewer for QueryEmbedPreviewer {
    fn id(&self) -> &'static str {
        "query_embed"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        ctx.resource
            .properties
            .get("body")
            .map(|b| b.contains("#+BEGIN_SRC query"))
            .unwrap_or(false)
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let body = ctx
            .resource
            .properties
            .get("body")
            .cloned()
            .unwrap_or_default();

        let req = parse_query_block(&body);
        Ok(PreviewModel::QueryEmbed {
            query: req,
            snapshot: Vec::new(),
        })
    }
}

/// Parse the `KEY: VALUE` lines that follow `#+BEGIN_SRC query` into a
/// `QueryRequest`. Unknown lines are ignored. The block is delimited by the
/// matching `#+END_SRC` marker.
fn parse_query_block(body: &str) -> QueryRequest {
    let mut source = String::new();
    let mut kind_hint = None;
    let mut title_contains = None;
    let mut limit: usize = 50;

    let mut in_block = false;
    for line in body.lines() {
        if !in_block {
            if line.contains("#+BEGIN_SRC query") {
                in_block = true;
            }
            continue;
        }
        if line.contains("#+END_SRC") {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_uppercase();
            let value = v.trim();
            match key.as_str() {
                "SOURCE" => source = value.to_string(),
                "KIND" => kind_hint = Some(value.to_string()),
                "TITLE" => title_contains = Some(value.to_string()),
                "LIMIT" => {
                    if let Ok(n) = value.parse::<usize>() {
                        limit = n;
                    }
                }
                _ => {}
            }
        }
    }

    QueryRequest {
        source,
        kind_hint,
        title_contains,
        limit,
    }
}
