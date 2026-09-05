//! End-to-end integration tests for `notez serve` + `notez remote`.
//!
//! Spawns the `notez` binary's `serve` subcommand on a loopback port
//! via std::process, then drives the API through the same
//! `NotezClient` the CLI uses. Exercises auth, dispatch, and the
//! notez card protocol arms end-to-end.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use notez_api::NotezClient;
use notez_protocol::request::{
    ExecuteCardRequest, ListCardsRequest, ReadCardRequest, Request,
};
use notez_protocol::response::Response;

const TOKEN: &str = "remote-cards-integration-secret";

fn seed_space(root: &std::path::Path) {
    std::fs::write(
        root.join("notez.toml"),
        "version = 2\n[source]\nname = \"remote-cards\"\ndatabase = \".notez/index.sqlite\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("notes")).unwrap();
    std::fs::write(
        root.join("notes/cards.org"),
        "#+name: inbox\n#+begin_src janet :card yes :title \"Inbox\" :output list\n(let [t (table/new 4)] (put t :type \"list\") (put t :items [\"a\" \"b\"]) t)\n#+end_src\n#+name: stats\n#+begin_src janet :card yes :id stats :title \"Stats\" :output json\n(let [t (table/new 4)] (put t :type \"json\") (put t :value {:count 7}) t)\n#+end_src\n",
    )
    .unwrap();
}

fn notez_bin() -> std::process::Command {
    let path = assert_cmd::cargo::cargo_bin("notez");
    std::process::Command::new(path)
}

fn wait_for_server(addr: &str) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let url = format!("{addr}/api/v1/healthz");
    for _ in 0..50 {
        if client.get(&url).send().is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("server never became ready at {url}");
}

fn spawn_server(root: &std::path::Path) -> (String, Box<dyn std::any::Any + Send>) {
    seed_space(root);
    // Allocate a port through the OS, drop the listener, and hand the
    // address to the child. Tiny race window but acceptable for tests.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let addr = format!("127.0.0.1:{port}");
    let child = notez_bin()
        .env("NOTEZ_API_TOKEN", TOKEN)
        .args(["serve", "--bind", &addr, "--source", &root.to_string_lossy()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn notez serve");
    let base = format!("http://{addr}");
    wait_for_server(&base);
    (base, Box::new(child))
}

#[test]
fn list_cards_round_trips_through_remote_client() {
    let dir = tempfile::tempdir().unwrap();
    let (base, _child) = spawn_server(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = runtime.block_on(async {
        NotezClient::new(&base).unwrap().with_token(TOKEN)
    });
    let response = runtime.block_on(async {
        client
            .dispatch(&Request::ListCards(ListCardsRequest { source: None, limit: Some(8) }))
            .await
    });
    let Response::Dashboard { cards } = response.expect("list cards") else {
        panic!("expected Dashboard, got");
    };
    assert_eq!(cards.len(), 2);
    let ids: Vec<&str> = cards.iter().map(|c| c.id.as_str()).collect();
    assert!(ids.contains(&"inbox"));
    assert!(ids.contains(&"stats"));
}

#[test]
fn read_card_returns_live_projection_via_remote_client() {
    let dir = tempfile::tempdir().unwrap();
    let (base, _child) = spawn_server(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = runtime.block_on(async {
        NotezClient::new(&base).unwrap().with_token(TOKEN)
    });
    let response = runtime.block_on(async {
        client
            .dispatch(&Request::ReadCard(ReadCardRequest {
                card_id: "inbox".to_string(),
                source: String::new(),
                locator: "notes/cards.org".to_string(),
            }))
            .await
    });
    let Response::Card(card) = response.expect("read card") else {
        panic!("expected Card, got");
    };
    assert_eq!(card.id, "inbox");
    assert_eq!(card.state, "ready");
    assert_eq!(card.output_type, "list");
}

#[test]
fn execute_card_invalid_id_surfaces_typed_protocol_error() {
    let dir = tempfile::tempdir().unwrap();
    let (base, _child) = spawn_server(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = runtime.block_on(async {
        NotezClient::new(&base).unwrap().with_token(TOKEN)
    });
    let err = runtime.block_on(async {
        client
            .dispatch(&Request::ExecuteCard(ExecuteCardRequest {
                card_id: "no-such".to_string(),
                source: String::new(),
                locator: "notes/cards.org".to_string(),
                timeout_ms: None,
            }))
            .await
    })
    .expect_err("invalid card id must fail");
    match err {
        notez_api::ClientError::Protocol { status, error } => {
            assert_eq!(status, 400);
            assert!(matches!(error, notez_protocol::Error::InvalidRequest { .. }));
        }
        other => panic!("expected Protocol 400, got {other:?}"),
    }
}

#[test]
fn unauthenticated_request_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (base, _child) = spawn_server(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = runtime.block_on(async { NotezClient::new(&base).unwrap() });
    let err = runtime.block_on(async {
        client
            .dispatch(&Request::ListCards(ListCardsRequest { source: None, limit: Some(1) }))
            .await
    })
    .expect_err("missing token must fail");
    match err {
        notez_api::ClientError::Protocol { status, error } => {
            assert_eq!(status, 401);
            assert!(matches!(error, notez_protocol::Error::Unauthorized { .. }));
        }
        other => panic!("expected Protocol 401, got {other:?}"),
    }
}