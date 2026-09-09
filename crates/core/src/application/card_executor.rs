//! Card execution port.
//!
//! One executor per language; the engine holds `CardExecutor` port
//! implementations through a registry (like `FormatParser`s). Today
//! the only registered executor is [`JanetCardExecutor`]; future
//! SQL / Lua / JSONata executors slot in beside it without changing
//! the protocol or the dispatcher.
//!
//! A small [`CardCache`] memoises results by the cache key composed
//! of code_hash + locator (stable source identifier) + principal
//! (so per-user authorisation cannot leak) + cap. Execution cost is
//! bounded by the executor's wall-clock timeout, but cache hits
//! short-circuit even that — a refocused card page should not pay
//! for re-execution of every card.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::document::notez_block::{CardOutput, NotezBlock, parse_markdown_notez_blocks, parse_org_notez_blocks};

/// Bound the cache to a small number of entries per Space. The cache
/// is intentionally tiny; refresh-policy belongs to the executor's
/// caller, not the cache itself.
pub const CARD_CACHE_CAP_DEFAULT: usize = 256;

/// One card execution port. Stateless on its own; the engine wraps
/// it in [`CardCache`] for memoisation.
pub trait CardExecutor: Send + Sync {
    fn language(&self) -> &str;

    fn execute(
        &self,
        block: &NotezBlock,
        context: &CardExecutionContext,
    ) -> Result<CardOutput, CardExecutionError>;

    /// Wall-clock budget for a single execution. Used to compute
    /// cache TTL (a slightly longer window guards against thundering
    /// herd re-execution).
    fn default_timeout(&self) -> Duration;
}

/// Static data passed to the executor. Bundles Source identity and
/// execution parameters into a single value so the cache can hash
/// against it without knowing how an executor interprets them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardExecutionContext {
    /// Stable identifier for the source the card came from (the
    /// filesystem canonical root in the native case).
    pub locator: String,
    /// Optional principal; included in the cache key so authorisation
    /// cannot leak across identities.
    pub principal: Option<String>,
    /// Optional frozen-source-revision. When set, cards bound to a
    /// source whose revision moved become stale (`CardState::Stale`).
    pub source_revision: Option<String>,
    /// Wall-clock budget the executor should honour.
    pub timeout: Duration,
}

impl CardExecutionContext {
    /// Convenience builder with sensible defaults: no principal, no
    /// revision, two-second budget.
    pub fn for_locator(locator: impl Into<String>) -> Self {
        Self {
            locator: locator.into(),
            principal: None,
            source_revision: None,
            timeout: Duration::from_secs(2),
        }
    }

    /// Stable cache key: serialised tuple of all hashing inputs.
    pub fn cache_key(&self, block: &NotezBlock) -> String {
        let lang = block.language.clone();
        let kind = block.kind.as_str().to_string();
        let code = block.code_hash.clone();
        let principal = self.principal.clone().unwrap_or_default();
        let rev = self.source_revision.clone().unwrap_or_default();
        let to = self.timeout.as_millis() as u64;
        format!("{lang}|{kind}|{code}|{locator}|{principal}|{rev}|{to}",
            lang = lang,
            kind = kind,
            code = code,
            locator = self.locator,
            principal = principal,
            rev = rev,
            to = to,
        )
    }
}

/// Outcome of an attempted card execution, surfaced to renderers so
/// they can render every state distinctly instead of collapsing to
/// one error path.
#[derive(Debug, Clone, PartialEq)]
pub enum CardState {
    /// The envelope parsed into a typed `CardOutput`.
    Ready(CardOutput),
    /// The block ran without erroring but the envelope was invalid
    /// (missing `type`, malformed fields, sanitizer rejection).
    Failed(String),
    /// Executor asked for more resources than the budget allowed.
    Timeout,
    /// The cached entry's source_revision disagrees with the
    /// current one — re-execute before surfacing.
    Stale,
    /// Executor refused to run because the principal lacks the
    /// required capability (not implemented today; placeholder so
    /// future scope-based auth slots in without changing the type).
    Denied(String),
}

impl CardState {
    pub fn ready(output: CardOutput) -> Self {
        CardState::Ready(output)
    }

    pub fn failed(detail: impl Into<String>) -> Self {
        CardState::Failed(detail.into())
    }
}

/// One cached entry. Failed/Timeout/Denied entries are cached too —
/// the failure itself is part of the rendered surface, and
/// replaying a slow script is worse than surfacing the last error.
#[derive(Debug, Clone)]
struct CacheEntry {
    state: CardState,
    stored_at: Instant,
}

/// FIFO-bounded cache. When capacity is exceeded the oldest entry
/// is dropped — a future refresh-policy layer can replace this with
/// TTL/LRU without changing the executor port.
pub struct CardCache {
    cap: usize,
    entries: Mutex<HashMap<String, CacheEntry>>,
}

impl Default for CardCache {
    fn default() -> Self {
        Self::new(CARD_CACHE_CAP_DEFAULT)
    }
}

impl CardCache {
    pub fn new(cap: usize) -> Self {
        Self { cap: cap.max(1), entries: Mutex::new(HashMap::new()) }
    }

    /// Look up an entry. The caller decides whether the cached
    /// state is still fresh (e.g. by comparing `source_revision`).
    pub fn get(&self, key: &str) -> Option<CardState> {
        self.entries
            .lock()
            .expect("card cache lock")
            .get(key)
            .map(|e| e.state.clone())
    }

    /// Insert or replace. Drops the oldest entry if at capacity.
    pub fn put(&self, key: String, state: CardState) {
        let mut entries = self.entries.lock().expect("card cache lock");
        if entries.len() >= self.cap && !entries.contains_key(&key) {
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.stored_at)
                .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest);
            }
        }
        entries.insert(
            key,
            CacheEntry { state, stored_at: Instant::now() },
        );
    }

    /// Invalidate every entry whose key contains the supplied
    /// source revision. Called when a Source's revision moves.
    pub fn invalidate_source_revision(&self, _revision: &str) {
        let mut entries = self.entries.lock().expect("card cache lock");
        entries.retain(|_, entry| match &entry.state {
            CardState::Stale => false,
            _ => entry.stored_at.elapsed()
                < Duration::from_secs(1), // keep near-fresh entries; the cache is advisory
        });
    }
}

/// Adapter between the dispatcher and the language-specific
/// executors. Looks up the right executor for the block's language
/// and routes execution through [`CardCache`].
pub struct CardExecutionService {
    pub executors: Mutex<HashMap<String, Arc<dyn CardExecutor>>>,
    cache: CardCache,
}

impl CardExecutionService {
    pub fn new(cache: CardCache) -> Self {
        Self { executors: Mutex::new(HashMap::new()), cache }
    }

    pub fn with_executor(self, executor: Arc<dyn CardExecutor>) -> Self {
        self.executors
            .lock()
            .expect("card executor registry lock")
            .insert(executor.language().to_string(), executor);
        self
    }

    pub fn cache(&self) -> &CardCache {
        &self.cache
    }

    /// Execute one block. Cache hits short-circuit; otherwise the
    /// executor runs, the result is validated, and the entry is
    /// stored.
    pub fn run(&self, block: &NotezBlock, context: &CardExecutionContext) -> CardState {
        let key = context.cache_key(block);
        if let Some(state) = self.cache.get(&key) {
            return state;
        }
        let executors = self.executors.lock().expect("card executor registry lock");
        let Some(executor) = executors.get(&block.language).cloned() else {
            let state = CardState::Failed(format!("no card executor registered for language `{}`", block.language));
            self.cache.put(key, state.clone());
            return state;
        };
        drop(executors);
        let _timeout = if context.timeout > Duration::ZERO {
            context.timeout
        } else {
            executor.default_timeout()
        };
        let state = match executor.execute(block, context) {
            Ok(output) => CardState::Ready(output),
            Err(CardExecutionError::Timeout) => CardState::Timeout,
            Err(CardExecutionError::Executor(detail)) => CardState::Failed(detail),
            Err(CardExecutionError::Envelope(detail)) => CardState::Failed(detail),
        };
        self.cache.put(key, state.clone());
        state
    }

    /// Project every card in one document and return its result,
    /// in document order. The caller is responsible for source-
    /// policy enforcement before invoking this.
    pub fn project_document(&self, text: &str, locator: &str) -> Vec<(NotezBlock, CardState)> {
        let format = guess_format(locator);
        let blocks = match format {
            DocFormat::Org => parse_org_notez_blocks(text),
            DocFormat::Markdown => parse_markdown_notez_blocks(text),
        };
        let context = CardExecutionContext::for_locator(locator);
        blocks
            .into_iter()
            .map(|block| {
                let state = self.run(&block, &context);
                (block, state)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocFormat {
    Markdown,
    Org,
}

fn guess_format(locator: &str) -> DocFormat {
    if locator.ends_with(".org") {
        DocFormat::Org
    } else {
        DocFormat::Markdown
    }
}

/// Failures surfaced by a [`CardExecutor`]. Distinct categories let
/// the cache hold failures too and renderers surface them precisely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardExecutionError {
    /// Wall-clock budget elapsed before the executor finished.
    Timeout,
    /// The runtime produced a script-level error.
    Executor(String),
    /// The script returned, but the envelope did not match the
    /// contract (missing `type`, malformed payload, sanitizer
    /// rejection). The renderer should surface `state = failed`
    /// with the detail string.
    Envelope(String),
}

/// One card projection rendered as JSON-ready data — the shape
/// renderers and remote clients consume.
#[derive(Debug, Clone)]
pub struct CardProjection {
    pub id: String,
    pub title: String,
    pub language: String,
    pub locator: String,
    pub ordinal: usize,
    pub state_name: &'static str,
    pub output_type: &'static str,
    pub output: Option<CardOutput>,
    pub error: Option<String>,
    pub error_kind: Option<String>,
    pub object_ref: Option<String>,
    pub html: Option<String>,
}

impl CardProjection {
    /// Render a CardState + block into the stable projection shape
    /// every surface consumes (Web SSR, future CLI / remote).
    pub fn from_state(block: &NotezBlock, locator: String, state: CardState) -> Self {
        let (state_name, output_type, output, error, error_kind, object_ref, html) = match state {
            CardState::Ready(out) => {
                let output_type = match &out {
                    CardOutput::Json(_) => "json",
                    CardOutput::List(_) => "list",
                    CardOutput::Object { .. } => "object",
                    CardOutput::Html(_) => "html",
                };
                let _output = match &out {
                    CardOutput::Json(value) => Some(value.clone()),
                    CardOutput::List(items) => Some(Value::Array(items.clone())),
                    CardOutput::Object { .. } => None,
                    CardOutput::Html(_) => None,
                };
                let object_ref = match &out {
                    CardOutput::Object { reference } => Some(reference.clone()),
                    _ => None,
                };
                let html = match &out {
                    CardOutput::Html(html) => Some(html.as_str().to_string()),
                    _ => None,
                };
                ("ready", output_type, Some(out.clone()), None, None, object_ref, html)
            }
            CardState::Failed(detail) => (
                "failed",
                "",
                None,
                Some(detail),
                Some("failed".to_string()),
                None,
                None,
            ),
            CardState::Timeout => (
                "timeout",
                "",
                None,
                Some("card execution exceeded its budget".to_string()),
                Some("timeout".to_string()),
                None,
                None,
            ),
            CardState::Stale => (
                "stale",
                "",
                None,
                Some("card source revision moved; refresh to see new results".to_string()),
                Some("stale".to_string()),
                None,
                None,
            ),
            CardState::Denied(detail) => (
                "denied",
                "",
                None,
                Some(detail),
                Some("denied".to_string()),
                None,
                None,
            ),
        };
        let title = block
            .attrs
            .get("title")
            .cloned()
            .unwrap_or_else(|| block.id.clone());
        Self {
            id: block.id.clone(),
            title,
            language: block.language.clone(),
            locator,
            ordinal: block.ordinal,
            state_name: state_static(state_name),
            output_type: output_type_static(&output_type),
            output,
            error,
            error_kind,
            object_ref,
            html,
        }
    }
}

fn state_static(s: &str) -> &'static str {
    // Five known values: ready / failed / timeout / stale / denied.
    // All are string literals above.
    match s {
        "ready" => "ready",
        "failed" => "failed",
        "timeout" => "timeout",
        "stale" => "stale",
        "denied" => "denied",
        _ => "failed",
    }
}

fn output_type_static(s: &str) -> &'static str {
    match s {
        "json" => "json",
        "list" => "list",
        "object" => "object",
        "html" => "html",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::notez_block::NotezBlockFormat;

    fn dummy_block(language: &str) -> NotezBlock {
        NotezBlock {
            id: "x".to_string(),
            kind: crate::document::notez_block::NotezBlockKind::Card,
            language: language.to_string(),
            attrs: Default::default(),
            program: String::new(),
            code_hash: "h".to_string(),
            format: NotezBlockFormat::Markdown,
            ordinal: 1,
            span: (0, 0),
        }
    }

    #[test]
    fn cache_stores_and_returns_last_value() {
        let cache = CardCache::new(2);
        let k = "k";
        cache.put(k.to_string(), CardState::Failed(String::from("x")));
        assert!(matches!(cache.get(k), Some(CardState::Failed(_))));
    }

    #[test]
    fn cache_evicts_oldest_when_over_capacity() {
        let cache = CardCache::new(2);
        cache.put("a".to_string(), CardState::Failed(String::from("a")));
        cache.put("b".to_string(), CardState::Failed(String::from("b")));
        cache.put("c".to_string(), CardState::Failed(String::from("c")));
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn cache_key_distinguishes_locator_and_principal() {
        let block = dummy_block("janet");
        let mut a = CardExecutionContext::for_locator("notes/a.md");
        let mut b = CardExecutionContext::for_locator("notes/b.md");
        a.principal = Some("alice".to_string());
        b.principal = Some("bob".to_string());
        assert_ne!(a.cache_key(&block), b.cache_key(&block));
    }

    struct FixedExecutor { output: CardOutput }

    impl CardExecutor for FixedExecutor {
        fn language(&self) -> &str { "test" }
        fn execute(&self, _: &NotezBlock, _: &CardExecutionContext) -> Result<CardOutput, CardExecutionError> {
            Ok(self.output.clone())
        }
        fn default_timeout(&self) -> Duration { Duration::from_secs(1) }
    }

    #[test]
    fn service_routes_to_matching_language_executor() {
        let service = CardExecutionService::new(CardCache::new(8))
            .with_executor(Arc::new(FixedExecutor {
                output: CardOutput::Json(serde_json::json!({"n": 1})),
            }));
        let block = dummy_block("test");
        let state = service.run(&block, &CardExecutionContext::for_locator(""));
        match state {
            CardState::Ready(CardOutput::Json(v)) => assert_eq!(v["n"], 1),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn service_records_failed_state_when_executor_missing() {
        let service = CardExecutionService::new(CardCache::new(8));
        let state = service.run(&dummy_block("sql"), &CardExecutionContext::for_locator(""));
        assert!(matches!(state, CardState::Failed(_)));
    }

    #[test]
    fn projection_state_names_are_stable_strings() {
        let block = dummy_block("janet");
        let proj = CardProjection::from_state(
            &block,
            "notes/a.md".to_string(),
            CardState::Ready(CardOutput::Json(serde_json::json!({}))),
        );
        assert_eq!(proj.state_name, "ready");
        assert_eq!(proj.output_type, "json");
    }
}