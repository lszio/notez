//! Handler for the restricted, read-only Janet query command.

use std::process::exit;

use super::{Service, exit_code_for};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::request::{ExecuteJanetRequest, Request};

/// Execute Janet through the protocol dispatcher. This handler never creates
/// or owns a Janet VM; the composed service supplies the executor when one is
/// available.
pub fn run_janet(
    json: bool,
    service: &mut Service,
    script: String,
    source_id: Option<String>,
    document_ref: Option<String>,
    timeout_ms: u64,
    result_limit: usize,
) {
    let request = Request::ExecuteJanet(ExecuteJanetRequest {
        script,
        source_id,
        document_ref,
        actor_id: "cli".to_string(),
        expected_revision: None,
        trace_id: None,
        timeout_ms,
        result_limit,
    });
    match ApplicationDispatcher::new(service).dispatch(request) {
        Ok(Response::Janet(result)) => {
            if json {
                println!("{}", serde_json::json!({ "value": result.value }));
            } else {
                println!("{}", result.value);
            }
        }
        Ok(other) => {
            eprintln!("Janet error: unexpected dispatcher response: {other:?}");
            exit(1);
        }
        Err(error) => {
            if json {
                println!("{}", error_json(&error));
            } else {
                eprintln!("Janet error: {error}");
            }
            exit(exit_code_for(&error));
        }
    }
}

fn error_json(error: &notez_core::application::ApplicationError) -> serde_json::Value {
    let kind = match error {
        notez_core::application::ApplicationError::Janet { .. } => "janet",
        notez_core::application::ApplicationError::UnsupportedCapability { .. } => "unsupported_capability",
        notez_core::application::ApplicationError::InvalidRequest { .. } => "invalid_request",
        _ => "application_error",
    };
    serde_json::json!({ "error": { "kind": kind, "message": error.to_string() } })
}

#[cfg(test)]
mod tests {
    use super::error_json;
    use notez_core::application::ApplicationError;

    #[test]
    fn forbidden_and_timeout_errors_keep_janet_category() {
        for kind in ["forbidden", "timeout"] {
            let error = ApplicationError::Janet {
                kind: kind.to_string(),
                message: "script rejected".to_string(),
            };
            let value = error_json(&error);
            assert_eq!(value["error"]["kind"], "janet");
            assert!(value["error"]["message"].as_str().unwrap().contains(kind));
        }
    }

    #[test]
    fn missing_native_executor_is_structured_unsupported() {
        let error = ApplicationError::UnsupportedCapability {
            capability: "execute_janet",
        };
        let value = error_json(&error);
        assert_eq!(value["error"]["kind"], "unsupported_capability");
    }
}
