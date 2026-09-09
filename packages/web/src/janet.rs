//! Janet runtime foundation for dynamic document content.
//!
//! This module embeds the Janet language so scripts can be evaluated
//! server-side as part of the "Janet query / render" capability in
//! `docs/refactoring-v1.org` §7.
//!
//! # Security model (design doc §7.2)
//!
//! Untrusted scripts run against a **restricted environment**, built
//! from three cooperating layers (single source of truth for the
//! forbidden-name lists is [`FORBIDDEN_EXACT`] / [`FORBIDDEN_PREFIXES`]):
//!
//! 1. **Capability sandbox**: every evaluation first runs Janet's
//!    native `(sandbox :all)` (evil-janet ≥1.41), which disables fs,
//!    net, subprocess, threads, ffi, dynamic modules, signal handlers,
//!    env vars and — critically — the `compile`/`unmarshal` entry
//!    points *at the C level*, so even a constructed call such as
//!    `(eval (string "os" "/exit"))` panics with an operation-forbidden
//!    error instead of executing.
//! 2. **Env scrubbing**: the sandbox does *not* guard `os/exit`, so the
//!    preamble additionally walks the root environment and replaces
//!    every binding matching the deny lists with a function that raises
//!    `notez-forbidden-api`. After scrubbing there is **no reachable
//!    binding** for os/io/net/dyn/ffi/require/import/… left in the VM;
//!    reaching one would require naming another denied symbol.
//! 3. **Host-side AST scan** (defense in depth): before the VM starts,
//!    the script is tokenized and any forbidden symbol anywhere in the
//!    source (including quoted forms) short-circuits evaluation with
//!    [`JanetScriptError::ForbiddenApi`].
//!
//! Execution runs on a dedicated one-shot worker thread with a ; results above [`MAX_RESULT_BYTES`]
//! are rejected. Known limitation: janetrs exposes neither an
//! instruction budget nor an interrupt hook, so a timed-out worker
//! thread cannot be preempted — see [`LEAKED_WORKERS`] for the bound
//! placed on that leak. A memory bomb inside the VM (allocating until
//! OOM) is likewise not yet bounded.

use std::fmt;

/// Structured failure of a Janet script evaluation.
///
/// Rendered inline by [`render_janet_blocks`] as a local error widget;
/// never fails a whole document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JanetScriptError {
    /// Script failed to parse or compile.
    Syntax(String),
    /// Script raised a runtime error (including uncaught `(error …)`).
    Runtime(String),
    /// Script touched a forbidden API (os/io/net/dyn/ffi/…) or a
    /// sandboxed capability.
    ForbiddenApi(String),
    /// Script exceeded its evaluation time budget.
    Timeout,
    /// Serialized result exceeded [`MAX_RESULT_BYTES`].
    ResultTooLarge,
}

impl fmt::Display for JanetScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(m) => write!(f, "syntax error: {m}"),
            Self::Runtime(m) => write!(f, "runtime error: {m}"),
            Self::ForbiddenApi(m) => write!(f, "forbidden api: {m}"),
            Self::Timeout => write!(f, "script exceeded the 2000ms evaluation budget"),
            Self::ResultTooLarge => write!(f, "result exceeds the 262144 byte limit"),
        }
    }
}

impl std::error::Error for JanetScriptError {}

impl JanetScriptError {
    /// Stable machine-readable category tag (used in rendered widgets
    /// and suitable for protocol payloads).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Syntax(_) => "syntax",
            Self::Runtime(_) => "runtime",
            Self::ForbiddenApi(_) => "forbidden-api",
            Self::Timeout => "timeout",
            Self::ResultTooLarge => "result-too-large",
        }
    }
}

/// Wall-clock budget for one script evaluation.
#[cfg(not(target_arch = "wasm32"))]
pub const EVAL_TIMEOUT_MS: u64 = 2_000;
/// Serialized-result size cap.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_RESULT_BYTES: usize = 256 * 1024;
/// Upper bound on worker threads abandoned mid-run after timeouts.
///
/// janetrs exposes no interrupt hook, so a timed-out script keeps its
/// thread spinning forever. Every timeout leaks exactly one thread
/// (plus its VM); past this many, further evaluations fail fast with
/// [`JanetScriptError::Timeout`] instead of stacking up unbounded CPU
/// burn. Workers that eventually finish despite being abandoned
/// release their slot.
#[cfg(not(target_arch = "wasm32"))]
pub const MAX_LEAKED_WORKERS: usize = 16;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::{
        JanetScriptError, EVAL_TIMEOUT_MS, MAX_RESULT_BYTES, MAX_LEAKED_WORKERS,
        FORBIDDEN_EXACT, FORBIDDEN_PREFIXES,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use janetrs::client::JanetClient;
    use janetrs::lowlevel as jl;
    use janetrs::{Janet, JanetString};



    static LEAKED_WORKERS: AtomicUsize = AtomicUsize::new(0);

    /// Evaluate `script` in a fresh sandboxed Janet runtime and return JSON.
    pub fn eval_janet(script: &str) -> Result<serde_json::Value, String> { eval_janet_checked(script).map_err(|e| e.to_string()) }

    pub fn eval_janet_checked(script: &str) -> Result<serde_json::Value, JanetScriptError> {
        eval_janet_with_budget(script, Duration::from_millis(EVAL_TIMEOUT_MS))
    }

    pub fn eval_janet_with_budget(script: &str, budget: Duration) -> Result<serde_json::Value, JanetScriptError> {
        eval_janet_with_context(script, budget, None, MAX_RESULT_BYTES)
    }

    /// Immutable data made available to Janet query functions. It is produced
    /// by the Engine/Dispatcher; Janet receives no storage or writer handle.
    #[derive(Debug, Clone, Default)]
    pub struct JanetQueryContext {
        pub sources: serde_json::Value,
        pub search: serde_json::Value,
        pub objects: serde_json::Value,
        pub relations: serde_json::Value,
        pub reads: std::collections::BTreeMap<String, serde_json::Value>,
        pub render_list: serde_json::Value,
    }

    pub fn eval_janet_with_context(script: &str, budget: Duration, context: Option<&JanetQueryContext>, result_limit: usize) -> Result<serde_json::Value, JanetScriptError> {
        if let Some(sym) = scan_forbidden_symbol(script) { return Err(JanetScriptError::ForbiddenApi(format!("script uses forbidden symbol `{sym}`"))); }
        if LEAKED_WORKERS.load(Ordering::Relaxed) >= MAX_LEAKED_WORKERS { return Err(JanetScriptError::Timeout); }
        let script = script.to_string();
        let preamble = build_sandbox_preamble(context);
        let (tx, rx) = mpsc::channel();
        let abandoned = Arc::new(AtomicBool::new(false));
        let worker_abandoned = Arc::clone(&abandoned);
        let spawned = thread::Builder::new().name("janet-eval".into()).spawn(move || {
            let result = run_in_fresh_vm(&script, &preamble, result_limit);
            if worker_abandoned.load(Ordering::Relaxed) { LEAKED_WORKERS.fetch_sub(1, Ordering::Relaxed); }
            let _ = tx.send(result);
        });
        spawned.map_err(|e| JanetScriptError::Runtime(format!("failed to spawn eval worker: {e}")))?;
        match rx.recv_timeout(budget) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => { abandoned.store(true, Ordering::Relaxed); LEAKED_WORKERS.fetch_add(1, Ordering::Relaxed); Err(JanetScriptError::Timeout) }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(JanetScriptError::Runtime("eval worker died unexpectedly".into())),
        }
    }

    pub(super) fn run_in_fresh_vm(script: &str, preamble: &str, result_limit: usize) -> Result<serde_json::Value, JanetScriptError> {
        let client = JanetClient::init_with_default_env().map_err(|e| JanetScriptError::Runtime(format!("janet init failed: {e}")))?;
        client.run(preamble).map_err(|e| JanetScriptError::Runtime(format!("sandbox setup failed: {e}")))?;
        let env_raw = client.env().expect("default env loaded").table().as_raw() as *mut jl::JanetTable;
        let mut out: jl::Janet = unsafe { jl::janet_wrap_nil() };
        let status = unsafe { jl::janet_dobytes(env_raw, script.as_ptr(), script.len() as i32, c"notez-eval".as_ptr(), &mut out) };
        let value = Janet::from(out);
        match status {
            0 => { let json = janet_value_to_json(value); match serde_json::to_vec(&json) { Ok(bytes) if bytes.len() > result_limit => Err(JanetScriptError::ResultTooLarge), Ok(_) => Ok(json), Err(e) => Err(JanetScriptError::Runtime(format!("result serialization failed: {e}"))) } }
            0x01 => { let msg = janet_error_message(&value); if msg.contains("forbidden") { Err(JanetScriptError::ForbiddenApi(msg)) } else { Err(JanetScriptError::Runtime(msg)) } }
            0x02 | 0x04 => Err(JanetScriptError::Syntax(janet_error_message(&value))),
            other => Err(JanetScriptError::Runtime(format!("unexpected janet status {other:#x}"))),
        }
    }

    fn json_to_janet(v: &serde_json::Value) -> String {
        match v {
            serde_json::Value::Null => "nil".into(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            serde_json::Value::Array(a) => format!("[{}]", a.iter().map(json_to_janet).collect::<Vec<_>>().join(" ")),
            serde_json::Value::Object(o) => format!("{{{}}}", o.iter().map(|(k,v)| format!("\"{}\" {}", k.replace('\\', "\\\\").replace('"', "\\\""), json_to_janet(v))).collect::<Vec<_>>().join(" ")),
        }
    }

    pub(super) fn build_sandbox_preamble(context: Option<&JanetQueryContext>) -> String {
        let exact = FORBIDDEN_EXACT.iter().map(|s| format!("\"{s}\" true")).collect::<Vec<_>>().join(" ");
        let prefixes = FORBIDDEN_PREFIXES.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(" ");
        let data = context.map(|c| {
            let reads = serde_json::to_value(&c.reads).unwrap_or_default();
            format!("(def _sources {}) (def _search {}) (def _objects {}) (def _relations {}) (def _reads {}) (def _render {})", json_to_janet(&c.sources), json_to_janet(&c.search), json_to_janet(&c.objects), json_to_janet(&c.relations), json_to_janet(&reads), json_to_janet(&c.render_list))
        }).unwrap_or_else(|| "(def _sources []) (def _search []) (def _objects []) (def _relations []) (def _reads {}) (def _render [])".into());
        format!("(sandbox :all)\n(def _deny (fn [& _] (error \"notez-forbidden-api\")))\n(def _exact {{{exact}}})\n(def _prefixes [{prefixes}])\n(each k (keys (curenv)) (def n (string k)) (when (or (get _exact n) (find |(string/has-prefix? $ n) _prefixes)) (put (curenv) k @{{:value _deny}})))\n{data}\n(def notez/sources (fn [] _sources))\n(def notez/search (fn [& _] _search))\n(def notez/objects (fn [& _] _objects))\n(def notez/relations (fn [& _] _relations))\n(def notez/read (fn [r] (get _reads (string r))))\n(def notez/render-list (fn [& _] _render))")
    }


    /// Host-side tokenizer: walk the script skipping strings/comments,
    /// collecting every symbol-like token, and report the first one
    /// matching the deny lists.
    fn scan_forbidden_symbol(script: &str) -> Option<&'static str> {
        let bytes = script.as_bytes();
        let is_delim = |b: u8| {
            matches!(
                b,
                b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'"' | b'\'' | b',' | b';' | b'@'
                    | b'~' | b'#' | b'`' | b' ' | b'\t' | b'\n' | b'\r' | b'|'
            )
        };
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'#' => {
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'"' | b'`' => {
                    let quote = bytes[i];
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == b'\\' {
                            i += 2;
                            continue;
                        }
                        let done = bytes[i] == quote;
                        i += 1;
                        if done {
                            break;
                        }
                    }
                }
                b if is_delim(b) => i += 1,
                _ => {
                    let start = i;
                    while i < bytes.len() && !is_delim(bytes[i]) {
                        i += 1;
                    }
                    if let Some(tok) = script.get(start..i) {
                        if let Some(f) = forbidden_token(tok) {
                            return Some(f);
                        }
                    }
                }
            }
        }
        None
    }

    fn forbidden_token(tok: &str) -> Option<&'static str> {
        if tok.starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
        FORBIDDEN_EXACT
            .iter()
            .find(|s| **s == tok)
            .or_else(|| FORBIDDEN_PREFIXES.iter().find(|p| tok.starts_with(**p)))
            .copied()
    }

    fn janet_error_message(value: &Janet) -> String {
        if let Ok(s) = value.clone().try_unwrap::<JanetString>() {
            String::from_utf8_lossy(s.as_bytes()).into_owned()
        } else {
            janet_value_to_json(value.clone()).to_string()
        }
    }

    /// Best-effort conversion of a Janet value to JSON.
    fn janet_value_to_json(value: janetrs::Janet) -> serde_json::Value {
        use janetrs::{JanetArray, JanetBuffer, JanetKeyword, JanetString, JanetStruct,
            JanetSymbol, JanetTable, JanetTuple};
        // Whole numbers in i64 range render as integers, preserving
        // `(+ 1 2)` -> 3 rather than 3.0.
        fn number(f: f64) -> serde_json::Value {
            if f.is_finite() && f.fract() == 0.0 {
                serde_json::json!(f as i64)
            } else {
                serde_json::json!(f)
            }
        }
        fn bytes(b: &[u8]) -> serde_json::Value {
            serde_json::json!(String::from_utf8_lossy(b))
        }
    fn key_string(key: &janetrs::Janet) -> String {
        let raw = match janet_value_to_json(key.clone()) {
            serde_json::Value::String(s) => s,
            serde_json::Value::Number(n) => n.to_string(),
            other => other.to_string(),
        };
        // Janet keyword-as-key (":type", ":items") is JSON-serialized
        // with the leading colon; strip it so envelope lookups see
        // plain names like `type` and `items`.
        raw.strip_prefix(':').map(str::to_string).unwrap_or(raw)
    }

        if value.is_nil() {
            return serde_json::Value::Null;
        }
        if let Ok(f) = value.clone().try_unwrap::<f64>() {
            return number(f);
        }
        if let Ok(b) = value.clone().try_unwrap::<bool>() {
            return serde_json::json!(b);
        }
        if let Ok(s) = value.clone().try_unwrap::<JanetString>() {
            return bytes(s.as_bytes());
        }
        if let Ok(b) = value.clone().try_unwrap::<JanetBuffer>() {
            return bytes(b.as_bytes());
        }
        if let Ok(s) = value.clone().try_unwrap::<JanetSymbol>() {
            return bytes(s.as_bytes());
        }
        if let Ok(k) = value.clone().try_unwrap::<JanetKeyword>() {
            return serde_json::json!(format!(":{}", String::from_utf8_lossy(k.as_bytes())));
        }
        if let Ok(t) = value.clone().try_unwrap::<JanetTuple>() {
            return serde_json::Value::Array(
                t.iter().map(|v| janet_value_to_json(v.clone())).collect(),
            );
        }
        if let Ok(a) = value.clone().try_unwrap::<JanetArray>() {
            return serde_json::Value::Array(
                a.iter().map(|v| janet_value_to_json(v.clone())).collect(),
            );
        }
        if let Ok(st) = value.clone().try_unwrap::<JanetStruct>() {
            return serde_json::Value::Object(
                st.iter()
                    .map(|(k, v)| (key_string(&k), janet_value_to_json(v.clone())))
                    .collect(),
            );
        }
        if let Ok(t) = value.try_unwrap::<JanetTable>() {
            return serde_json::Value::Object(
                t.iter()
                    .map(|(k, v)| (key_string(&k), janet_value_to_json(v.clone())))
                    .collect(),
            );
        }
        serde_json::json!(format!("{value:?}"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use imp::{eval_janet, eval_janet_checked, eval_janet_with_budget};

/// Forbidden binding names (whole-symbol matches). Shared verbatim by
/// the in-VM scrub and the host-side scan.
#[cfg(not(target_arch = "wasm32"))]
const FORBIDDEN_EXACT: &[&str] = &[
    "os",
    "io",
    "net",
    "ffi",
    "dyn",
    "setdyn",
    "dynamic",
    "eval",
    "eval-string",
    "compile",
    "asm",
    "disasm",
    "marshal",
    "unmarshal",
    "require",
    "import",
    "dofile",
    "slurp",
    "spit",
    "quit",
    "debug",
    "root-env",
    "all-bindings",
    "all-dynamics",
    "curenv",
    "make-env",
    "sandbox",
];

/// Forbidden binding namespaces (symbol-prefix matches).
#[cfg(not(target_arch = "wasm32"))]
const FORBIDDEN_PREFIXES: &[&str] =
    &["os/", "io/", "file/", "net/", "ffi", "ev/", "debug/", "module/", "thread/", "bundle/"];
#[cfg(not(target_arch = "wasm32"))]
pub use imp::{eval_janet_with_context, JanetQueryContext};

// ---- notez card executor / renderer ----------------------------------------

/// Execute one notez card block: run the program in the sandboxed
/// Janet VM, then validate the result against the output envelope.
/// The executor never renders; the renderer never executes.
/// Execute one notez card block. Routes through the core
/// [`notez_core::application::CardExecutionService` so so] so every
/// surface shares one executor, one cache, and one envelope
/// contract. Janet errors are mapped back to the legacy error
/// type so existing handlers keep their semantics.
#[cfg(not(target_arch = "wasm32"))]
pub fn execute_card(
    block: &notez_core::document::NotezBlock,
) -> Result<notez_core::document::CardOutput, JanetScriptError> {
    use notez_core::application::{CardExecutionContext, CardExecutionService};
    let service = CardExecutionService::new(
        notez_core::application::CardCache::default(),
    )
    .with_executor(std::sync::Arc::new(
        notez_core::application::janet::JanetCardExecutor,
    ));
    let context = CardExecutionContext::for_locator("");
    let state = service.run(block, &context);
    match state {
        notez_core::application::CardState::Ready(out) => Ok(out),
        notez_core::application::CardState::Failed(detail) => {
            Err(JanetScriptError::Runtime(detail))
        }
        notez_core::application::CardState::Timeout => Err(JanetScriptError::Timeout),
        notez_core::application::CardState::Stale => {
            Err(JanetScriptError::Runtime("card source revision moved".to_string()))
        }
        notez_core::application::CardState::Denied(detail) => {
            Err(JanetScriptError::Runtime(format!("denied: {detail}")))
        }
    }
}

/// One executed card: the block plus its outcome. Rendering turns
/// this into markup; failures render as localized error cards and
/// never fail the surrounding page.
#[cfg(not(target_arch = "wasm32"))]
pub struct ExecutedCard {
    pub block: notez_core::document::NotezBlock,
    pub outcome: Result<notez_core::document::CardOutput, JanetScriptError>,
}

#[cfg(not(target_arch = "wasm32"))]
fn card_title(block: &notez_core::document::NotezBlock) -> String {
    html_escape::encode_text(block.attrs.get("title").unwrap_or(&block.id)).into_owned()
}

#[cfg(not(target_arch = "wasm32"))]
fn json_compact(value: &serde_json::Value) -> String {
    html_escape::encode_text(&value.to_string()).into_owned()
}

/// Render an executed card as a self-contained `<section>`.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_card_html(card: &ExecutedCard) -> String {
    let title = card_title(&card.block);
    match &card.outcome {
        Ok(notez_core::document::CardOutput::Json(value)) => format!(
            "<section class=\"notez-card\" data-card-id=\"{}\" data-state=\"ready\" data-output=\"json\"><h3 class=\"notez-card-title\">{}</h3><pre class=\"notez-card-body\">{}</pre></section>",
            html_escape::encode_text(&card.block.id),
            title,
            json_compact(value)
        ),
        Ok(notez_core::document::CardOutput::List(items)) => {
            let rows = items
                .iter()
                .map(|item| format!("<li>{}</li>", json_compact(item)))
                .collect::<Vec<_>>()
                .join("");
            format!(
                "<section class=\"notez-card\" data-card-id=\"{}\" data-state=\"ready\" data-output=\"list\"><h3 class=\"notez-card-title\">{}</h3><ul class=\"notez-card-list\">{}</ul></section>",
                html_escape::encode_text(&card.block.id),
                title,
                rows
            )
        }
        Ok(notez_core::document::CardOutput::Object { reference }) => format!(
            "<section class=\"notez-card\" data-card-id=\"{}\" data-state=\"ready\" data-output=\"object\"><h3 class=\"notez-card-title\">{}</h3><code class=\"notez-card-ref\">{}</code></section>",
            html_escape::encode_text(&card.block.id),
            title,
            html_escape::encode_text(reference)
        ),
        Ok(notez_core::document::CardOutput::Html(html)) => format!(
            "<section class=\"notez-card notez-card--html\" data-card-id=\"{}\" data-state=\"ready\" data-output=\"html\"><h3 class=\"notez-card-title\">{}</h3><div class=\"notez-card-html\">{}</div></section>",
            html_escape::encode_text(&card.block.id),
            title,
            html.as_str()
        ),
        Err(err) => format!(
            "<section class=\"notez-card\" data-card-id=\"{}\" data-state=\"failed\"><h3 class=\"notez-card-title\">{}</h3><pre class=\"notez-card-error\" data-kind=\"{}\">{}</pre></section>",
            html_escape::encode_text(&card.block.id),
            title,
            err.kind(),
            html_escape::encode_text(&err.to_string())
        ),
    }
}

/// Render a Markdown body containing both notez card blocks and
/// legacy `janet` fences.
///
/// Two passes, deliberately:
/// 1. notez blocks (parsed by the shared core parser, executed by
///    [`execute_card`], rendered by [`render_card_html`]) are
///    replaced by their card markup;
/// 2. the remaining plain `janet` fences keep the legacy inline
///    evaluation behavior.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_dynamic_blocks(text: &str) -> String {
    let blocks = notez_core::document::parse_markdown_notez_blocks(text);
    if blocks.is_empty() {
        return render_janet_blocks(text);
    }
    let mut out = text.to_string();
    // Replace from the end so earlier spans stay valid.
    for block in blocks.iter().rev() {
        let outcome = execute_card(block);
        let card = ExecutedCard { block: block.clone(), outcome };
        let (start, end) = block.span;
        out.replace_range(start..end, &render_card_html(&card));
    }
    render_janet_blocks(&out)
}

#[cfg(target_arch = "wasm32")]
pub fn render_dynamic_blocks(text: &str) -> String {
    render_janet_blocks(text)
}

/// Evaluate `janet` fenced code blocks in a document body and replace
/// each with its JSON result (or a structured local error widget) as a
/// `<pre>`. This gives documents an in-body dynamic block: ```` ```janet
//// ```` becomes the computed output when the page renders. A malformed,
/// blocked or slow script renders a visible categorized error instead of
/// failing the whole document.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_janet_blocks(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(fence_start) = rest.find("```") {
        out.push_str(&rest[..fence_start]);
        let rest2 = &rest[fence_start..];
        let line_end = rest2.find('\n').map(|i| i).unwrap_or(rest2.len());
        let fence_line = &rest2[..line_end];
        let lang = fence_line.trim_start_matches('`').trim();
        let after_line = rest2.get(line_end + 1..).unwrap_or("");
        if lang != "janet" {
            // Non-janet fenced block: emit verbatim, seek its closing fence.
            out.push_str(fence_line);
            if line_end < rest2.len() {
                out.push('\n');
            }
            if let Some(close) = after_line.find("```") {
                let close_end = close + 3;
                out.push_str(&after_line[..close_end]);
                rest = &after_line[close_end..];
            } else {
                out.push_str(after_line);
                rest = "";
            }
            continue;
        }
        // janet block: find the closing fence inside the body.
        if let Some(close) = after_line.find("```") {
            let script = after_line[..close].trim();
            let rendered = match eval_janet_checked(script) {
                Ok(v) => format!(
                    "\n<pre class=\"janet-block-result\">{}</pre>\n",
                    html_escape::encode_text(&v.to_string())
                ),
                Err(e) => format!(
                    "\n<pre class=\"janet-block-error\" data-kind=\"{}\">[{}] {}</pre>\n",
                    e.kind(),
                    e.kind(),
                    html_escape::encode_text(&e.to_string())
                ),
            };
            out.push_str(&rendered);
            rest = &after_line[close + 3..];
        } else {
            out.push_str(rest2);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

#[cfg(target_arch = "wasm32")]
pub fn render_janet_blocks(text: &str) -> String {
    text.to_string()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use imp::{eval_janet_checked, eval_janet_with_budget};
    use std::time::{Duration, Instant};

    fn ev(script: &str) -> Result<serde_json::Value, JanetScriptError> {
        eval_janet_checked(script)
    }

    #[test]
    fn evaluates_arithmetic() {
        assert_eq!(ev("(+ 1 2)").unwrap(), serde_json::json!(3));
    }

    #[test]
    fn evaluates_float_and_boolean() {
        assert_eq!(ev("(/ 5 2)").unwrap(), serde_json::json!(2.5));
        assert_eq!(ev("(> 3 1)").unwrap(), serde_json::json!(true));
    }

    #[test]
    fn evaluates_nil() {
        assert_eq!(ev("nil").unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn surfaces_syntax_error() {
        let err = ev("(").unwrap_err();
        assert!(matches!(err, JanetScriptError::Syntax(_)), "got: {err:?}");
    }

    #[test]
    fn direct_dangerous_symbols_are_forbidden() {
        for s in [
            "(os/exit)",
            "(os/spawn \"sh\")",
            "(io/mem)",
            "(net/connect \"h\" 80)",
            "(dynamic :a 1)",
            "(require x)",
            "(ffi/native \"lib.so\")",
        ] {
            let err = ev(s).unwrap_err();
            assert_eq!(err.kind(), "forbidden-api", "script `{s}`: {err}");
        }
    }

    #[test]
    fn constructed_os_exit_cannot_reach_os() {
        // Building the string alone is harmless data construction.
        assert_eq!(
            ev("(string \"os\" \"/exit\")").unwrap(),
            serde_json::json!("os/exit")
        );
        // Routing it into eval must yield a categorized error, not a
        // dead service process.
        let err = ev("(eval (string \"os\" \"/exit\"))").unwrap_err();
        assert_eq!(err.kind(), "forbidden-api", "{err}");
    }

    #[test]
    fn scrubbed_env_denies_dynamic_lookup() {
        // Deliberately bypasses the host-side AST scan (which would
        // reject `curenv` statically) to prove layer 2: even when the
        // symbol is computed at runtime, the scrubbed environment has
        // no live `os/exit` binding left to resolve.
        let script = "(def s (symbol (string \"os\" \"/exit\"))) (((curenv) s) 1)";
        let err = imp::run_in_fresh_vm(script, &imp::build_sandbox_preamble(None), MAX_RESULT_BYTES).unwrap_err();
        assert_eq!(err.kind(), "forbidden-api", "{err}");
    }
    #[test]
    fn read_only_query_api_uses_immutable_context() {
        let mut reads = std::collections::BTreeMap::new();
        reads.insert("document:one".to_string(), serde_json::json!({"title":"One"}));
        let context = JanetQueryContext { sources: serde_json::json!([{"id":"native"}]), reads, ..Default::default() };
        let value = imp::eval_janet_with_context("(tuple (notez/sources) (notez/read 'document:one))", Duration::from_secs(1), Some(&context), MAX_RESULT_BYTES).unwrap();
        assert_eq!(value[0], serde_json::json!([{"id":"native"}]));
        assert_eq!(value[1], serde_json::json!({"title":"One"}));
    }

    #[test]
    fn query_api_cannot_write() {
        let context = JanetQueryContext::default();
        let err = imp::eval_janet_with_context("(put (notez/sources) 0 1)", Duration::from_secs(1), Some(&context), MAX_RESULT_BYTES).unwrap_err();
        assert!(matches!(err, JanetScriptError::Runtime(_) | JanetScriptError::ForbiddenApi(_)));
    }

    #[test]
    fn infinite_loop_times_out() {
        let start = Instant::now();
        let err = eval_janet_with_budget("(while true nil)", Duration::from_millis(300)).unwrap_err();
        assert_eq!(err.kind(), "timeout", "{err}");
        assert!(start.elapsed() < Duration::from_secs(10), "took {:?}", start.elapsed());
    }

    #[test]
    fn oversized_result_rejected() {
        let err = ev("(string/repeat \"a\" 400000)").unwrap_err();
        assert_eq!(err.kind(), "result-too-large", "{err}");
    }

    #[test]
    fn result_under_cap_still_works() {
        let v = ev("(string/repeat \"a\" 1000)").unwrap();
        assert_eq!(v, serde_json::json!("a".repeat(1000)));
    }

    #[test]
    fn renders_janet_block_in_body() {
        let body = "Before.\n\n```janet\n(+ 1 2)\n```\n\nAfter.";
        let out = render_janet_blocks(body);
        assert!(out.contains("janet-block-result"), "missing result pre: {out}");
        assert!(out.contains("3"), "missing evaluated value: {out}");
        assert!(out.contains("Before.") && out.contains("After."), "context lost: {out}");
    }

    #[test]
    fn leaves_non_janet_blocks_and_shows_structured_error() {
        let body = "```rust\nlet x = 1;\n```\n\n```janet\n(os/exit)\n```";
        let out = render_janet_blocks(body);
        assert!(out.contains("let x = 1;"), "non-janet block lost: {out}");
        assert!(out.contains("janet-block-error"), "error pre missing: {out}");
        assert!(out.contains("data-kind=\"forbidden-api\""), "category tag missing: {out}");
    }
}
