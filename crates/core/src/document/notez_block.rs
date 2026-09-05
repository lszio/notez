//! `notez` dynamic blocks — the document-level protocol for cards,
//! queries and views.
//!
//! A `notez` block declares *what* a dynamic fragment is (kind,
//! input, output, policy); the `language` attribute only picks the
//! executor. Today that is Janet; the envelope below is language
//! agnostic so future executors plug in without touching surfaces.
//!
//! # Wire forms
//!
//! Markdown (fence info string; bare token `card` sets the kind):
//!
//! ````markdown
//! ```notez kind=card id=inbox title=Inbox language=janet output=list
//! (notez/query ctx {:kind "document" :status "TODO"})
//! ```
//! ````
//!
//! Org (standard `src` block + header arguments; `:card yes` opts the
//! block into the notez model, other `:key value` pairs are
//! attributes, `#+name` is the default id):
//!
//! ```org
//! #+name: inbox
//! #+begin_src janet :card yes :title "Inbox" :output list
//! (notez/query ctx {:kind "document" :status "TODO"})
//! #+end_src
//! ```
//!
//! Plain `janet` fences / `src janet` blocks without the card marker
//! are NOT notez blocks — they stay ordinary code blocks (the web
//! body keeps its legacy inline evaluation for them).
//!
//! # Output envelope
//!
//! A program's final value must be one of ([`CardOutput`]):
//!
//! ```janet
//! {:type "json"   :value ...}
//! {:type "list"   :items [...]}
//! {:type "object" :ref "notez://object/..."}
//! ```
//!
//! `html` is intentionally rejected in this iteration until the
//! sanitizer boundary exists. The engine validates the envelope —
//! the renderer never guesses.

use crate::document::content_hash_of_bytes;
use std::collections::BTreeMap;

/// What a notez block is for. Extensible; `card` is the only kind
/// rendered by surfaces today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotezBlockKind {
    Card,
}

impl NotezBlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotezBlockKind::Card => "card",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "card" => Some(NotezBlockKind::Card),
            _ => None,
        }
    }
}

/// Source format a block was parsed from. The attribute syntax
/// differs; the model does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotezBlockFormat {
    Markdown,
    Org,
}

/// One parsed `notez` block. Pure data: no execution, no engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotezBlock {
    /// Stable within its document: explicit attribute, `#+name`, or
    /// `card-<ordinal>` (1-based ordinal among notez blocks of the
    /// document). Never a fresh random id — cache keys and dashboard
    /// layout depend on stability.
    pub id: String,
    pub kind: NotezBlockKind,
    /// Executor language. Attribute on Markdown, `src` language on
    /// Org. Defaults to `janet`.
    pub language: String,
    /// All attributes (`key=value` / `:key value`), lowercased keys.
    pub attrs: BTreeMap<String, String>,
    /// Program source between the fences / src lines.
    pub program: String,
    /// Content hash of `program` (cache key component).
    pub code_hash: String,
    pub format: NotezBlockFormat,
    /// 1-based ordinal among notez blocks in the document.
    pub ordinal: usize,
    /// Byte range of the whole block (opening fence line through the
    /// closing fence) in the source text. Lets renderers replace
    /// blocks in place.
    pub span: (usize, usize),
}

impl NotezBlock {
    /// Convenience accessor for the declared output contract
    /// (`output` attribute); `None` lets the executor infer it from
    /// the envelope `:type`.
    pub fn declared_output(&self) -> Option<&str> {
        self.attrs.get("output").map(String::as_str)
    }

    /// Declared wall-clock budget in milliseconds (`timeout`
    /// attribute); surfaces cap this with their own default.
    pub fn declared_timeout_ms(&self) -> Option<u64> {
        self.attrs.get("timeout").and_then(|v| v.trim().parse().ok())
    }
}

/// Parse every `notez` block in a Markdown document.
pub fn parse_markdown_notez_blocks(text: &str) -> Vec<NotezBlock> {
    let mut blocks = Vec::new();
    let mut rest = text;
    let mut consumed = 0usize;
    let mut ordinal = 0usize;
    while let Some(offset) = rest.find("```") {
        let block_start = consumed + offset;
        let after_fence = &rest[offset..];
        let line_end = after_fence.find('\n').unwrap_or(after_fence.len());
        let fence_line = &after_fence[..line_end];
        let info = fence_line.trim_start_matches('`').trim();
        if !info.split_whitespace().next().unwrap_or("").eq_ignore_ascii_case("notez") {
            // Not a notez block: skip past its closing fence if any.
            let body = after_fence.get(line_end + 1..).unwrap_or("");
            rest = match body.find("```") {
                Some(close) => {
                    consumed += offset + line_end + 1 + close + 3;
                    &body[close + 3..]
                }
                None => "",
            };
            continue;
        }
        let body = after_fence.get(line_end + 1..).unwrap_or("");
        let Some(close) = body.find("```") else {
            break; // unterminated block: stop scanning
        };
        let program = body[..close].trim().to_string();
        let block_end = block_start + line_end + 1 + close + 3;
        ordinal += 1;
        blocks.push(build_block(
            info_string_attrs(info),
            program,
            NotezBlockFormat::Markdown,
            ordinal,
            (block_start, block_end),
        ));
        consumed = block_end;
        rest = &after_fence[line_end + 1 + close + 3..];
    }
    blocks
}

/// Parse every notez card block in an Org document. Only `src`
/// blocks whose header arguments contain `:card yes` (or a bare
/// `:card`) participate.
pub fn parse_org_notez_blocks(text: &str) -> Vec<NotezBlock> {
    let mut blocks = Vec::new();
    let mut ordinal = 0usize;
    let mut pending_name: Option<String> = None;
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        // Affiliated keyword: `#+name:` directly before the block.
        if let Some(rest) = trimmed
            .strip_prefix("#+name:")
            .or_else(|| trimmed.strip_prefix("#+NAME:"))
        {
            let value = rest.trim().to_string();
            pending_name = if value.is_empty() { None } else { Some(value) };
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if !lower.starts_with("#+begin_src") {
            if !trimmed.is_empty() {
                // Any other line breaks name affiliation.
                pending_name = None;
            }
            continue;
        }
        let header = trimmed["#+begin_src".len()..].trim();
        let mut tokens = header.split_whitespace();
        let language = tokens.next().unwrap_or("janet").to_string();
        let attrs = parse_org_header_args(tokens);
        let marker = attrs
            .get("card")
            .map(|v| matches!(v.as_str(), "" | "yes" | "true" | "t"))
            .unwrap_or(false);
        // Find the matching `#+end_src` (org has no nested src).
        let mut program_lines: Vec<&str> = Vec::new();
        let mut closed = false;
        for inner in lines.by_ref() {
            let inner_trimmed = inner.trim();
            if inner_trimmed.to_ascii_lowercase().starts_with("#+end_src") {
                closed = true;
                break;
            }
            program_lines.push(inner);
        }
        if marker && closed {
            ordinal += 1;
            let mut block_attrs = attrs;
            if let Some(name) = pending_name.take() {
                block_attrs.entry("id".to_string()).or_insert(name);
            }
            blocks.push(build_block(
                block_attrs,
                program_lines.join("\n").trim().to_string(),
                NotezBlockFormat::Org,
                ordinal,
                (0, 0),
            ));
        } else {
            pending_name = None;
        }
    }
    blocks
}

/// Parse `key=value` pairs from a fence info string
/// (`notez kind=card id=inbox`). A bare token sets `kind` if it
/// names one, otherwise it lands in `attrs` under `"<token>"`.
fn info_string_attrs(info: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    for token in info.split_whitespace().skip(1) {
        if let Some((key, value)) = token.split_once('=') {
            let value = value.trim_matches('"').trim_matches('\'').to_string();
            attrs.insert(key.to_ascii_lowercase(), value);
        } else {
            let key = token.trim_matches('"').trim_matches('\'');
            attrs.insert(key.to_ascii_lowercase(), String::new());
            if let Some(kind) = NotezBlockKind::parse(key) {
                attrs.insert("kind".to_string(), kind.as_str().to_string());
            }
        }
    }
    attrs
}

/// Parse Org header arguments (`:key value` or bare `:key`). Values
/// may be double-quoted. Standard babel args are kept in `attrs`
/// untouched; notez only reads the keys it knows.
fn parse_org_header_args<'a, I: Iterator<Item = &'a str>>(tokens: I) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let mut current: Option<String> = None;
    for token in tokens {
        if let Some(key) = token.strip_prefix(':') {
            if let Some(prev) = current.take() {
                // Previous flag had no value.
                attrs.insert(prev.to_ascii_lowercase(), String::new());
            }
            current = Some(key.to_string());
        } else if let Some(key) = current.take() {
            let value = token.trim_matches('"').trim_matches('\'').to_string();
            attrs.insert(key.to_ascii_lowercase(), value);
        }
    }
    if let Some(key) = current.take() {
        attrs.insert(key.to_ascii_lowercase(), String::new());
    }
    attrs
}

fn build_block(
    mut attrs: BTreeMap<String, String>,
    program: String,
    format: NotezBlockFormat,
    ordinal: usize,
    span: (usize, usize),
) -> NotezBlock {
    let kind = attrs
        .get("kind")
        .and_then(|v| NotezBlockKind::parse(v))
        .or_else(|| {
            // `:card yes` style / bare `card` token: kind card.
            if attrs.contains_key("card") {
                Some(NotezBlockKind::Card)
            } else {
                None
            }
        })
        .unwrap_or(NotezBlockKind::Card);
    let language = attrs
        .get("language")
        .cloned()
        .unwrap_or_else(|| "janet".to_string());
    let id = attrs
        .get("id")
        .filter(|v| !v.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| format!("card-{ordinal}"));
    if format == NotezBlockFormat::Org {
        // `:card yes` is the marker, not a display attribute.
        attrs.remove("card");
    }
    let code_hash = content_hash_of_bytes(program.as_bytes());
    NotezBlock { id, kind, language, attrs, program, code_hash, format, ordinal, span }
}

/// Validated program output envelope.
#[derive(Debug, Clone, PartialEq)]
pub enum CardOutput {
    /// `{:type "json" :value …}` — stats, counters, arbitrary JSON.
    Json(serde_json::Value),
    /// `{:type "list" :items [...]}` — list cards.
    List(Vec<serde_json::Value>),
    /// `{:type "object" :ref "notez://…"}` — a reference the client
    /// resolves through the protocol; never a fabricated identity.
    Object { reference: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardOutputError {
    /// Result was not an object with a `type` field.
    MissingEnvelope,
    /// Unknown `type` value.
    UnknownType(String),
    /// Known type but malformed payload.
    Malformed { detail: String },
    /// `html` output requires the sanitizer boundary; rejected for now.
    HtmlDisabled,
}

impl std::fmt::Display for CardOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardOutputError::MissingEnvelope => {
                write!(f, "card result must be an object like {{:type \"json\" :value …}}")
            }
            CardOutputError::UnknownType(t) => write!(f, "unknown card output type `{t}`"),
            CardOutputError::Malformed { detail } => write!(f, "malformed card output: {detail}"),
            CardOutputError::HtmlDisabled => write!(
                f,
                "html card output is disabled (sanitizer boundary not available)"
            ),
        }
    }
}

impl std::error::Error for CardOutputError {}

impl CardOutput {
    /// Validate a program result against the output envelope.
    pub fn from_value(value: &serde_json::Value) -> Result<CardOutput, CardOutputError> {
        let Some(obj) = value.as_object() else {
            return Err(CardOutputError::MissingEnvelope);
        };
        let Some(kind) = obj.get("type").and_then(serde_json::Value::as_str) else {
            return Err(CardOutputError::MissingEnvelope);
        };
        match kind {
            "json" => {
                let value = obj
                    .get("value")
                    .ok_or_else(|| CardOutputError::Malformed {
                        detail: "`json` envelope requires a `value` field".into(),
                    })?;
                Ok(CardOutput::Json(value.clone()))
            }
            "list" => {
                let items = obj.get("items").and_then(serde_json::Value::as_array).ok_or_else(
                    || CardOutputError::Malformed {
                        detail: "`list` envelope requires an `items` array".into(),
                    },
                )?;
                Ok(CardOutput::List(items.clone()))
            }
            "object" => {
                let reference = obj
                    .get("ref")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| CardOutputError::Malformed {
                        detail: "`object` envelope requires a `ref` string".into(),
                    })?;
                Ok(CardOutput::Object { reference: reference.to_string() })
            }
            "html" => Err(CardOutputError::HtmlDisabled),
            other => Err(CardOutputError::UnknownType(other.to_string())),
        }
    }

    /// Stable machine name used by renderers and wire payloads.
    pub fn type_name(&self) -> &'static str {
        match self {
            CardOutput::Json(_) => "json",
            CardOutput::List(_) => "list",
            CardOutput::Object { .. } => "object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn markdown_info_string_form_parses() {
        let text = "before\n\n```notez kind=card id=inbox title=Inbox output=list\n(notez/query ctx)\n```\n\nafter\n";
        let blocks = parse_markdown_notez_blocks(text);
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!(b.id, "inbox");
        assert_eq!(b.kind, NotezBlockKind::Card);
        assert_eq!(b.language, "janet");
        assert_eq!(b.declared_output(), Some("list"));
        assert_eq!(b.program, "(notez/query ctx)");
        assert_eq!(b.ordinal, 1);
        assert!(!b.code_hash.is_empty());
        let (start, end) = b.span;
        assert_eq!(&text[start..end], "```notez kind=card id=inbox title=Inbox output=list\n(notez/query ctx)\n```");
    }

    #[test]
    fn multiple_markdown_blocks_get_stable_ordinals_and_spans() {
        let first = "```notez id=a\n(+ 1 2)\n```";
        let second = "```notez id=b\n(+ 3 4)\n```";
        let text = format!("x\n{first}\ny\n{second}\nz");
        let blocks = parse_markdown_notez_blocks(&text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id, "a");
        assert_eq!(blocks[1].id, "b");
        let (s0, e0) = blocks[0].span;
        assert_eq!(&text[s0..e0], first);
        let (s1, e1) = blocks[1].span;
        assert_eq!(&text[s1..e1], second);
    }

    #[test]
    fn bare_card_token_and_defaults() {
        let text = "```notez card\n(+ 1 2)\n```";
        let blocks = parse_markdown_notez_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, "card-1");
        assert_eq!(blocks[0].kind, NotezBlockKind::Card);
        assert_eq!(blocks[0].language, "janet");
    }

    #[test]
    fn non_notez_fences_are_ignored() {
        let text = "```rust\nlet x = 1;\n```\n\n```janet\n(os/exit)\n```";
        assert!(parse_markdown_notez_blocks(text).is_empty());
    }

    #[test]
    fn org_header_argument_form_parses() {
        let text = "#+name: inbox\n#+begin_src janet :card yes :title \"Inbox\" :output list :timeout 1500\n(notez/query ctx)\n#+end_src\n";
        let blocks = parse_org_notez_blocks(text);
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!(b.id, "inbox"); // :id absent → #+name fallback
        assert_eq!(b.format, NotezBlockFormat::Org);
        assert_eq!(b.declared_output(), Some("list"));
        assert_eq!(b.declared_timeout_ms(), Some(1500));
        assert_eq!(b.attrs.get("title").map(String::as_str), Some("Inbox"));
        assert!(!b.attrs.contains_key("card"), "marker must not leak as attribute");
    }

    #[test]
    fn org_explicit_id_beats_name_and_plain_src_is_ignored() {
        let text = "#+name: named\n#+begin_src janet :card yes :id explicit\n(+ 1 2)\n#+end_src\n\n#+begin_src python\nprint('hi')\n#+end_src\n";
        let blocks = parse_org_notez_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, "explicit");
    }

    #[test]
    fn unterminated_markdown_block_stops_cleanly() {
        let text = "```notez card\n(+ 1 2)";
        assert!(parse_markdown_notez_blocks(text).is_empty());
    }

    #[test]
    fn envelope_json_list_object_validate() {
        assert_eq!(
            CardOutput::from_value(&json!({"type":"json","value":{"n":1}})).unwrap(),
            CardOutput::Json(json!({"n":1}))
        );
        assert_eq!(
            CardOutput::from_value(&json!({"type":"list","items":[1,2]})).unwrap(),
            CardOutput::List(vec![json!(1), json!(2)])
        );
        assert_eq!(
            CardOutput::from_value(&json!({"type":"object","ref":"notez://object/local::x"}))
                .unwrap(),
            CardOutput::Object { reference: "notez://object/local::x".into() }
        );
    }

    #[test]
    fn envelope_rejects_missing_unknown_and_html() {
        assert!(matches!(
            CardOutput::from_value(&json!("plain string")),
            Err(CardOutputError::MissingEnvelope)
        ));
        assert!(matches!(
            CardOutput::from_value(&json!({"type":"wat"})),
            Err(CardOutputError::UnknownType(_))
        ));
        assert!(matches!(
            CardOutput::from_value(&json!({"type":"list"})),
            Err(CardOutputError::Malformed { .. })
        ));
        assert_eq!(
            CardOutput::from_value(&json!({"type":"html","value":"<p>x</p>"})),
            Err(CardOutputError::HtmlDisabled)
        );
    }
}
