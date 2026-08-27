//! Native, sandboxed Janet query executor shared by CLI, MCP, and web.
//!
//! The executor only receives an immutable [`JanetQuerySnapshot`]. It never
//! receives storage, a database connection, or a source writer.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use janetrs::client::JanetClient;
use janetrs::lowlevel as jl;
use janetrs::{Janet, JanetString};

use super::service::{JanetExecutor, JanetQuerySnapshot};
use notez_protocol::request::ExecuteJanetRequest;

pub const DEFAULT_TIMEOUT_MS: u64 = 2_000;
pub const DEFAULT_RESULT_LIMIT: usize = 256 * 1024;
const MAX_LEAKED_WORKERS: usize = 16;
static LEAKED_WORKERS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JanetScriptError {
    Syntax(String), Runtime(String), ForbiddenApi(String), Timeout, ResultTooLarge,
}
impl JanetScriptError {
    pub fn kind(&self) -> &'static str { match self { Self::Syntax(_) => "syntax", Self::Runtime(_) => "runtime", Self::ForbiddenApi(_) => "forbidden-api", Self::Timeout => "timeout", Self::ResultTooLarge => "result-too-large" } }
}
impl std::fmt::Display for JanetScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { match self { Self::Syntax(m) => write!(f, "syntax error: {m}"), Self::Runtime(m) => write!(f, "runtime error: {m}"), Self::ForbiddenApi(m) => write!(f, "forbidden api: {m}"), Self::Timeout => write!(f, "script exceeded the {DEFAULT_TIMEOUT_MS}ms evaluation budget"), Self::ResultTooLarge => write!(f, "result exceeds the {DEFAULT_RESULT_LIMIT} byte limit") } }
}
impl std::error::Error for JanetScriptError {}

/// Adapter implementing the application executor port.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeJanetExecutor;
impl JanetExecutor for NativeJanetExecutor {
    fn execute(&mut self, request: &ExecuteJanetRequest, snapshot: &JanetQuerySnapshot) -> Result<serde_json::Value, (String, String)> {
        let context = snapshot.clone();
        eval_janet_with_context(&request.script, Duration::from_millis(request.timeout_ms.min(DEFAULT_TIMEOUT_MS)), Some(&context), request.result_limit.min(DEFAULT_RESULT_LIMIT)).map_err(|e| (e.kind().to_string(), e.to_string()))
    }
}

pub fn eval_janet_checked(script: &str) -> Result<serde_json::Value, JanetScriptError> { eval_janet_with_context(script, Duration::from_millis(DEFAULT_TIMEOUT_MS), None, DEFAULT_RESULT_LIMIT) }
pub fn eval_janet_with_context(script: &str, budget: Duration, context: Option<&JanetQuerySnapshot>, result_limit: usize) -> Result<serde_json::Value, JanetScriptError> {
    if let Some(sym) = scan_forbidden_symbol(script) { return Err(JanetScriptError::ForbiddenApi(format!("script uses forbidden symbol `{sym}`"))); }
    if LEAKED_WORKERS.load(Ordering::Relaxed) >= MAX_LEAKED_WORKERS { return Err(JanetScriptError::Timeout); }
    let script = script.to_owned();
    let preamble = build_sandbox_preamble(context);
    let (tx, rx) = mpsc::channel();
    let abandoned = Arc::new(AtomicBool::new(false));
    let worker_abandoned = Arc::clone(&abandoned);
    thread::Builder::new().name("janet-eval".into()).spawn(move || {
        let result = run_in_fresh_vm(&script, &preamble, result_limit);
        if worker_abandoned.load(Ordering::Relaxed) { LEAKED_WORKERS.fetch_sub(1, Ordering::Relaxed); }
        let _ = tx.send(result);
    }).map_err(|e| JanetScriptError::Runtime(format!("failed to spawn eval worker: {e}")))?;
    match rx.recv_timeout(budget) { Ok(result) => result, Err(mpsc::RecvTimeoutError::Timeout) => { abandoned.store(true, Ordering::Relaxed); LEAKED_WORKERS.fetch_add(1, Ordering::Relaxed); Err(JanetScriptError::Timeout) }, Err(_) => Err(JanetScriptError::Runtime("eval worker died unexpectedly".into())) }
}

fn run_in_fresh_vm(script: &str, preamble: &str, result_limit: usize) -> Result<serde_json::Value, JanetScriptError> {
    let client = JanetClient::init_with_default_env().map_err(|e| JanetScriptError::Runtime(format!("janet init failed: {e}")))?;
    client.run(preamble).map_err(|e| JanetScriptError::Runtime(format!("sandbox setup failed: {e}")))?;
    let env_raw = client.env().expect("default env loaded").table().as_raw() as *mut jl::JanetTable;
    let mut out: jl::Janet = unsafe { jl::janet_wrap_nil() };
    let status = unsafe { jl::janet_dobytes(env_raw, script.as_ptr(), script.len() as i32, c"notez-eval".as_ptr(), &mut out) };
    let value = Janet::from(out);
    match status { 0 => { let json = janet_value_to_json(value); match serde_json::to_vec(&json) { Ok(bytes) if bytes.len() > result_limit => Err(JanetScriptError::ResultTooLarge), Ok(_) => Ok(json), Err(e) => Err(JanetScriptError::Runtime(format!("result serialization failed: {e}"))) } }, 0x01 => { let msg = janet_error_message(&value); if msg.contains("forbidden") { Err(JanetScriptError::ForbiddenApi(msg)) } else { Err(JanetScriptError::Runtime(msg)) } }, 0x02 | 0x04 => Err(JanetScriptError::Syntax(janet_error_message(&value))), other => Err(JanetScriptError::Runtime(format!("unexpected janet status {other:#x}"))) }
}

fn json_to_janet(v: &serde_json::Value) -> String { match v { serde_json::Value::Null => "nil".into(), serde_json::Value::Bool(b) => b.to_string(), serde_json::Value::Number(n) => n.to_string(), serde_json::Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")), serde_json::Value::Array(a) => format!("[{}]", a.iter().map(json_to_janet).collect::<Vec<_>>().join(" ")), serde_json::Value::Object(o) => format!("{{{}}}", o.iter().map(|(k,v)| format!("\"{}\" {}", k.replace('\\', "\\\\").replace('"', "\\\""), json_to_janet(v))).collect::<Vec<_>>().join(" ")) } }
fn build_sandbox_preamble(context: Option<&JanetQuerySnapshot>) -> String {
    let exact = FORBIDDEN_EXACT.iter().map(|s| format!("\"{s}\" true")).collect::<Vec<_>>().join(" ");
    let prefixes = FORBIDDEN_PREFIXES.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(" ");
    let data = context.map(|c| { let reads = serde_json::to_value(&c.reads).unwrap_or_default(); format!("(def _sources {}) (def _search {}) (def _objects {}) (def _relations {}) (def _reads {}) (def _render {})", json_to_janet(&c.sources), json_to_janet(&c.search), json_to_janet(&c.objects), json_to_janet(&c.relations), json_to_janet(&reads), json_to_janet(&c.render_list)) }).unwrap_or_else(|| "(def _sources []) (def _search []) (def _objects []) (def _relations []) (def _reads {}) (def _render [])".into());
    format!("(sandbox :all)\n(def _deny (fn [& _] (error \"notez-forbidden-api\")))\n(def _exact {{{exact}}})\n(def _prefixes [{prefixes}])\n(each k (keys (curenv)) (def n (string k)) (when (or (get _exact n) (find |(string/has-prefix? $ n) _prefixes)) (put (curenv) k @{{:value _deny}})))\n{data}\n(def notez/sources (fn [] _sources))\n(def notez/search (fn [& _] _search))\n(def notez/objects (fn [& _] _objects))\n(def notez/relations (fn [& _] _relations))\n(def notez/read (fn [r] (get _reads (string r))))\n(def notez/render-list (fn [& _] _render))")
}
fn scan_forbidden_symbol(script: &str) -> Option<&'static str> { let mut i=0; let b=script.as_bytes(); let delim=|x| matches!(x,b'('|b')'|b'['|b']'|b'{'|b'}'|b'"'|b'\''|b','|b';'|b'@'|b'~'|b'#'|b'`'|b' '|b'\t'|b'\n'|b'\r'|b'|'); while i<b.len() { match b[i] { b'#' => { while i<b.len()&&b[i]!=b'\n' {i+=1;} }, b'"'|b'`' => { let q=b[i]; i+=1; while i<b.len() { if b[i]==b'\\' {i+=2;} else {let done=b[i]==q;i+=1;if done{break;}} } }, x if delim(x)=>i+=1, _=>{let s=i;while i<b.len()&&!delim(b[i]){i+=1;}if let Some(t)=script.get(s..i){if let Some(f)=forbidden_token(t){return Some(f);}}} } } None }
fn forbidden_token(tok: &str) -> Option<&'static str> { if tok.starts_with(|c:char|c.is_ascii_digit()){return None;} FORBIDDEN_EXACT.iter().find(|s|**s==tok).or_else(||FORBIDDEN_PREFIXES.iter().find(|p|tok.starts_with(**p))).copied() }
fn janet_error_message(value: &Janet) -> String { if let Ok(s)=value.clone().try_unwrap::<JanetString>() {String::from_utf8_lossy(s.as_bytes()).into_owned()} else {janet_value_to_json(value.clone()).to_string()} }
fn janet_value_to_json(value: Janet) -> serde_json::Value {
    use janetrs::{JanetArray, JanetBuffer, JanetKeyword, JanetString, JanetStruct, JanetSymbol, JanetTable, JanetTuple};
    fn number(f:f64)->serde_json::Value{if f.is_finite()&&f.fract()==0.0{serde_json::json!(f as i64)}else{serde_json::json!(f)}} fn bytes(b:&[u8])->serde_json::Value{serde_json::json!(String::from_utf8_lossy(b))} fn key(v:&Janet)->String{janet_value_to_json(v.clone()).to_string().trim_matches('"').to_string()}
    if value.is_nil(){return serde_json::Value::Null} if let Ok(f)=value.clone().try_unwrap::<f64>(){return number(f)} if let Ok(v)=value.clone().try_unwrap::<bool>(){return serde_json::json!(v)} if let Ok(v)=value.clone().try_unwrap::<JanetString>(){return bytes(v.as_bytes())} if let Ok(v)=value.clone().try_unwrap::<JanetBuffer>(){return bytes(v.as_bytes())} if let Ok(v)=value.clone().try_unwrap::<JanetSymbol>(){return bytes(v.as_bytes())} if let Ok(v)=value.clone().try_unwrap::<JanetKeyword>(){return serde_json::json!(format!(":{}",String::from_utf8_lossy(v.as_bytes())))} if let Ok(v)=value.clone().try_unwrap::<JanetTuple>(){return serde_json::Value::Array(v.iter().map(|x|janet_value_to_json(x.clone())).collect())} if let Ok(v)=value.clone().try_unwrap::<JanetArray>(){return serde_json::Value::Array(v.iter().map(|x|janet_value_to_json(x.clone())).collect())} if let Ok(v)=value.clone().try_unwrap::<JanetStruct>(){return serde_json::Value::Object(v.iter().map(|(k,x)|(key(&k),janet_value_to_json(x.clone()))).collect())} if let Ok(v)=value.try_unwrap::<JanetTable>(){return serde_json::Value::Object(v.iter().map(|(k,x)|(key(&k),janet_value_to_json(x.clone()))).collect())} serde_json::json!(format!("{value:?}"))
}

const FORBIDDEN_EXACT: &[&str] = &["os","io","net","ffi","dyn","setdyn","dynamic","eval","eval-string","compile","asm","disasm","marshal","unmarshal","require","import","dofile","slurp","spit","quit","debug","root-env","all-bindings","all-dynamics","curenv","make-env","sandbox"];
const FORBIDDEN_PREFIXES: &[&str] = &["os/","io/","file/","net/","ffi","ev/","debug/","module/","thread/","bundle/"];

#[cfg(test)]
mod tests { use super::*; #[test] fn arithmetic_works(){assert_eq!(eval_janet_checked("(+ 1 2)").unwrap(),serde_json::json!(3));} #[test] fn dangerous_api_rejected(){assert_eq!(eval_janet_checked("(os/exit)").unwrap_err().kind(),"forbidden-api");} #[test] fn executor_reads_snapshot(){let snapshot=JanetQuerySnapshot{sources:serde_json::json!([{"id":"native"}]),..Default::default()};let request=ExecuteJanetRequest{script:"(notez/sources)".into(),source_id:None,document_ref:None,actor_id:"test".into(),expected_revision:None,trace_id:None,timeout_ms:DEFAULT_TIMEOUT_MS,result_limit:DEFAULT_RESULT_LIMIT};let mut executor=NativeJanetExecutor;assert_eq!(executor.execute(&request,&snapshot).unwrap(),serde_json::json!([{"id":"native"}]));} }
