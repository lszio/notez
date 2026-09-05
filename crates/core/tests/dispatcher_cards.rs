//! Dispatcher integration for the notez card protocol arms.
//!
//! Round-trips protocol Request variants through the dispatcher and
//! asserts the engine returns the expected CardProjection shape.

#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

use notez_core::application::Engine;
use notez_core::application::dispatcher::ApplicationDispatcher;
use notez_core::config::model::{SourceConfig, SourceIdentity};
use notez_core::storage::SqliteProjection;
use notez_protocol::request::{
    ExecuteCardRequest, ListCardsRequest, ReadCardRequest, Request,
};
use notez_protocol::response::Response;
use tempfile::tempdir;

fn open_engine(root: PathBuf) -> Engine<SqliteProjection> {
    let cfg = SourceConfig {
        version: 2,
        source: SourceIdentity {
            name: "card-test".to_string(),
            database: ".notez/index.sqlite".into(),
        },
        workflow: notez_core::config::model::WorkflowConfig::default(),
        sources: Vec::new(),
        link_overrides: serde_json::Value::Null,
        scan: notez_core::config::model::ScanConfig::default(),
    };
    let store = SqliteProjection::in_memory().expect("in_memory");
    let mut engine = Engine::with_source(store, notez_core::application::context::SourceContext::new(
        "card-test".to_string(),
        root,
        cfg,
    ));
    engine.attach_card_executor(std::sync::Arc::new(
        notez_core::application::janet::JanetCardExecutor,
    ));
    engine
}

fn seed_card(root: &std::path::Path, name: &str, body: &str) {
    std::fs::create_dir_all(root.join("notes")).unwrap();
    std::fs::write(root.join("notes").join(name), body).unwrap();
}

#[test]
fn list_cards_projects_every_card_in_a_source() {
    let dir = tempdir().unwrap();
    seed_card(
        dir.path(),
        "cards.org",
        "#+name: inbox\n#+begin_src janet :card yes :id inbox :output list\n(let [t (table/new 4)] (put t :type \"list\") (put t :items [\"a\" \"b\"]) t)\n#+end_src\n",
    );
    let mut engine = open_engine(dir.path().to_path_buf());
    let mut dispatcher = ApplicationDispatcher::new(&mut engine);
    let response = dispatcher
        .dispatch(Request::ListCards(ListCardsRequest { source: None, limit: Some(8) }))
        .expect("dispatch");
    let Response::Dashboard { cards: projections } = response else {
        panic!("expected Dashboard, got {response:?}");
    };
    assert_eq!(projections[0].id, "inbox");
    assert_eq!(projections[0].output_type, "list");
}

#[test]
fn read_card_returns_one_projection() {
    let dir = tempdir().unwrap();
    seed_card(
        dir.path(),
        "cards.org",
        "#+name: inbox\n#+begin_src janet :card yes :id inbox :output json\n{:type \"json\" :value {:count 7}}\n#+end_src\n",
    );
    let mut engine = open_engine(dir.path().to_path_buf());
    let mut dispatcher = ApplicationDispatcher::new(&mut engine);
    let response = dispatcher
        .dispatch(Request::ReadCard(ReadCardRequest {
            source: "card-test".to_string(),
            locator: "notes/cards.org".to_string(),
            card_id: "inbox".to_string(),
        }))
        .expect("dispatch");
    let Response::Card(projection) = response else {
        panic!("expected Card, got {response:?}");
    };
    assert_eq!(projection.id, "inbox");
    assert_eq!(projection.state, "ready");
    assert_eq!(projection.output_type, "json");
}

#[test]
fn execute_card_respects_timeout_override() {
    let dir = tempdir().unwrap();
    seed_card(
        dir.path(),
        "cards.org",
        "#+name: slow\n#+begin_src janet :card yes :id slow :timeout 200 :output json\n{:type \"json\" :value 1}\n#+end_src\n",
    );
    let mut engine = open_engine(dir.path().to_path_buf());
    let mut dispatcher = ApplicationDispatcher::new(&mut engine);
    let response = dispatcher
        .dispatch(Request::ExecuteCard(ExecuteCardRequest {
            source: "card-test".to_string(),
            locator: "notes/cards.org".to_string(),
            card_id: "slow".to_string(),
            timeout_ms: Some(1_500),
        }))
        .expect("dispatch");
    let Response::CardExecution(projection) = response else {
        panic!("expected CardExecution, got {response:?}");
    };
    assert_eq!(projection.id, "slow");
    assert_eq!(projection.state, "ready");
}

#[test]
fn execute_card_invalid_id_returns_application_error() {
    let dir = tempdir().unwrap();
    seed_card(dir.path(), "cards.org", "#+begin_src janet :card yes\n(+ 1 2)\n#+end_src\n");
    let mut engine = open_engine(dir.path().to_path_buf());
    let mut dispatcher = ApplicationDispatcher::new(&mut engine);
    let err = dispatcher
        .dispatch(Request::ExecuteCard(ExecuteCardRequest {
            source: "card-test".to_string(),
            locator: "notes/cards.org".to_string(),
            card_id: "no-such".to_string(),
            timeout_ms: None,
        }))
        .expect_err("invalid card id must fail");
    let notez_core::application::service::ApplicationError::InvalidRequest { message } = err else {
        panic!("expected InvalidRequest, got {err:?}");
    };
    assert!(message.contains("no-such"));
}