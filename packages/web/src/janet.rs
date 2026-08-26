//! Janet runtime foundation for dynamic document content.
//!
//! This module embeds the Janet language so scripts can be evaluated
//! server-side as part of the "Janet query / render" capability in
//! `docs/refactoring-v1.org` §7.
//!
//! Security note (design doc §7.2): this is the *foundation* — it
//! runs a script in a fresh Janet runtime and returns its result as
//! JSON. The full whitelist sandbox (no arbitrary os/io/file access,
//! injected `notez/*` API, timeout + result cap) is a follow-up. The
//! default env is loaded so arithmetic and data-construction forms
//! work; the whitelist narrowing is the next hardening step.

use janetrs::client::JanetClient;
use serde::Serialize;

/// Evaluate `script` in a fresh Janet runtime and return the result
/// as JSON. Errors (init / eval) are surfaced as strings so a call
/// site can render a structured `script_error`.
#[cfg(not(target_arch = "wasm32"))]
pub fn eval_janet(script: &str) -> Result<serde_json::Value, String> {
    // First-line guard (design doc §7.2): Janet's core env still
    // exposes the `os`/`io`/`net` modules, and a script running
    // against an evaluated document must not reach arbitrary OS
    // operations (e.g. `(os/exit)`, `(os/spawn)`). This blocks the
    // known dangerous entry points before the VM runs. A proper
    // whitelist env (no os/io/spawn bound at all) is the follow-up
    // hardening; this guard closes the immediate kill/escape path.
    const FORBIDDEN: &[&str] = &[
        "os/", "io/", "net/", "spawn", "dyn", "ffi", "require", "import",
    ];
    if let Some(f) = FORBIDDEN.iter().find(|f| script.contains(**f)) {
        return Err(format!("script uses a forbidden symbol ({f})"));
    }
    let client = JanetClient::init_with_default_env()
        .map_err(|e| format!("janet init failed: {e}"))?;
    let value = client
        .run(script)
        .map_err(|e| format!("janet eval failed: {e}"))?;
    Ok(janet_value_to_json(value))
}

/// Best-effort conversion of a Janet value to JSON. Handles numbers
/// (integers and reals), booleans and nil; anything else is rendered
/// as a readable string. Full table/array marshalling is a follow-up.
fn janet_value_to_json(value: janetrs::Janet) -> serde_json::Value {
    if value.is_nil() {
        return serde_json::Value::Null;
    }
    if let Ok(f) = value.try_unwrap::<f64>() {
        // Whole numbers in i64 range render as integers, preserving
        // `(+ 1 2)` -> 3 rather than 3.0.
        if f.is_finite() && f.fract() == 0.0 {
            return serde_json::json!(f as i64);
        }
        return serde_json::json!(f);
    }
    if let Ok(b) = value.try_unwrap::<bool>() {
        return serde_json::json!(b);
    }
    serde_json::json!(format!("{value:?}"))
}

/// Evaluate `janet` fenced code blocks in a document body and replace
/// each with its JSON result (or a structured error) as a `<pre>`.
/// This gives documents an in-body dynamic block: ```` ```janet … ````
/// becomes the computed output when the page renders. A malformed or
/// blocked script renders a visible error instead of crashing.
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
            let rendered = match eval_janet(script) {
                Ok(v) => format!(
                    "\n<pre class=\"janet-block-result\">{}</pre>\n",
                    html_escape::encode_text(&v.to_string())
                ),
                Err(e) => format!(
                    "\n<pre class=\"janet-block-error\">janet error: {}</pre>\n",
                    html_escape::encode_text(&e)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_arithmetic() {
        assert_eq!(eval_janet("(+ 1 2)").unwrap(), serde_json::json!(3));
    }

    #[test]
    fn evaluates_float_and_boolean() {
        assert_eq!(eval_janet("(/ 5 2)").unwrap(), serde_json::json!(2.5));
        assert_eq!(eval_janet("(> 3 1)").unwrap(), serde_json::json!(true));
    }

    #[test]
    fn evaluates_nil() {
        assert_eq!(eval_janet("nil").unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn surfaces_eval_error() {
        assert!(eval_janet("(").is_err());
    }

    #[test]
    fn blocks_dangerous_symbols() {
        for s in ["(os/exit)", "(os/spawn \"sh\")", "(io/mem)", "(net/connect ...)", "(dynamic ...)"] {
            let err = eval_janet(s).unwrap_err();
            assert!(err.contains("forbidden"), "script {s} not blocked: {err}");
        }
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
    fn leaves_non_janet_blocks_and_shows_error() {
        let body = "```rust\nlet x = 1;\n```\n\n```janet\n(os/exit)\n```";
        let out = render_janet_blocks(body);
        assert!(out.contains("let x = 1;"), "non-janet block lost: {out}");
        assert!(out.contains("janet-block-error"), "error pre missing: {out}");
    }
}
