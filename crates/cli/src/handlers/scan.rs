//! Handler for `Commands::Scan`.

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::request::{Request, ScanNativeRequest};

pub fn run_scan(json: bool, service: &mut Service) {
    match ApplicationDispatcher::new(service).dispatch(Request::ScanNative(ScanNativeRequest {})) {
        Ok(Response::Scan(report)) => {
            if json {
                println!(
                    "{}",
                    json!({
                        "scanned_files": report.scanned_files,
                        "scanned_resources": report.scanned_resources,
                        "scanned_relations": report.scanned_relations,
                    })
                );
            } else {
                println!(
                    "Scanned {} files, {} resources, {} relations.",
                    report.scanned_files, report.scanned_resources, report.scanned_relations
                );
            }
        }
        Err(e) => {
            eprintln!("Scan error: {e}");
            exit(exit_code_for(&e));
        }
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}
