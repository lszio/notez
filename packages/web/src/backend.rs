//! `web::backend` — surface-agnostic data port for the web host.
//!
//! Two implementations of [`ui::Backend`]:
//!
//! * `EmbeddedBackend` — in-process; opens engines through the
//!   composition `Runtime` and dispatches typed protocol requests.
//!   This is the default path.
//! * `HttpBackend` — talks to a remote notez server through
//!   `notez_api::NotezClient`. Selected by `NOTEZ_DATA_BACKEND=http`.
//!
//! Both share the same `ui::Backend` trait, so any page or `#[server]`
//! function can pick the implementation via
//! `crate::routes::state_snapshot().backend()` and stay free of the
//! underlying transport.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use notez_api::NotezClient;
use notez_composition::native::Runtime;
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::config::discovery::{SelectedSource, SourceSelector};
use notez_core::storage::SqliteProjection;
use notez_protocol::request::{QueryResourcesRequest, Request, ScanNativeRequest};
use notez_protocol::Error as ProtocolError;
use ui::{Backend, ResourceRow, SpaceRow};

type EngineHandle =
    Arc<Mutex<notez_core::application::Engine<SqliteProjection>>>;

fn open_space(root: &PathBuf) -> Result<EngineHandle, String> {
    let handle = notez_composition::native::open_space(SourceSelector::Path(root.as_path()), None)
        .map_err(|e| e.to_string())?;
    Ok(Arc::new(Mutex::new(handle.engine)))
}

// ---- EmbeddedBackend --------------------------------------------------------

/// In-process backend: opens engines through the composition Runtime
/// and dispatches typed protocol requests.
#[derive(Clone)]
pub struct EmbeddedBackend {
    runtime: Runtime,
    default_root: Option<PathBuf>,
}

impl EmbeddedBackend {
    pub fn new(default_root: Option<PathBuf>) -> Self {
        Self {
            runtime: Runtime::new(),
            default_root,
        }
    }

    fn resolved_root(&self, root: &str) -> Result<PathBuf, String> {
        if root.is_empty() {
            return self
                .default_root
                .clone()
                .ok_or_else(|| "no space selected".to_string());
        }
        let path = PathBuf::from(root);
        if path.exists() {
            return Ok(path);
        }
        Err(format!("cannot resolve space '{root}'"))
    }

    fn dispatch_blocking(
        &self,
        root: &PathBuf,
        request: Request,
    ) -> Result<Response, String> {
        let engine = open_space(root)?;
        let mut guard = engine
            .lock()
            .map_err(|e| format!("engine lock poisoned: {e}"))?;
        ApplicationDispatcher::new(&mut guard)
            .dispatch(request)
            .map_err(|e| e.to_string())
    }
}

impl Backend for EmbeddedBackend {
    async fn list_spaces(&self) -> Result<Vec<SpaceRow>, String> {
        let backend = self.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<SpaceRow>, String> {
            if let Some(default) = backend.default_root.as_ref() {
                return Ok(vec![SpaceRow {
                    source_id: "<default>".to_string(),
                    display_name: default
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("space")
                        .to_string(),
                    root: default.clone(),
                }]);
            }
            Ok(Vec::new())
        })
        .await
        .map_err(|e| format!("join error: {e}"))?
    }

    async fn scan_space(&self, root: &str) -> Result<u32, String> {
        let backend = self.clone();
        let root_text = root.to_string();
        tokio::task::spawn_blocking(move || -> Result<u32, String> {
            let root = backend.resolved_root(&root_text)?;
            let response = backend.dispatch_blocking(
                &root,
                Request::ScanNative(ScanNativeRequest {}),
            )?;
            match response {
                Response::Scan(report) => Ok(report.scanned_resources as u32),
                other => Err(format!("unexpected scan response: {other:?}")),
            }
        })
        .await
        .map_err(|e| format!("join error: {e}"))?
    }

    async fn query_resources(
        &self,
        root: &str,
        title_contains: Option<&str>,
        kind: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<ResourceRow>, String> {
        let backend = self.clone();
        let root_text = root.to_string();
        let title_contains = title_contains.map(|s| s.to_string());
        let kind = kind.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || -> Result<Vec<ResourceRow>, String> {
            let root = backend.resolved_root(&root_text)?;
            let request = Request::QueryResources(QueryResourcesRequest {
                kind,
                title_contains,
                exact_ref: None,
                source_id: None,
                limit,
            });
            let response = backend.dispatch_blocking(&root, request)?;
            match response {
                Response::ResourcePage(page) => Ok(page
                    .items
                    .into_iter()
                    .map(|r| ResourceRow {
                        id: r.ref_,
                        kind: format!("{:?}", r.kind),
                        title: r.title,
                        locator: r.locator,
                    })
                    .collect()),
                other => Err(format!("unexpected query response: {other:?}")),
            }
        })
        .await
        .map_err(|e| format!("join error: {e}"))?
    }
}

// ---- HttpBackend ------------------------------------------------------------

/// Remote backend: talks to a notez HTTP API server. Select via
/// `NOTEZ_REMOTE_URL` (defaults to `http://127.0.0.1:8700`) and
/// optionally `NOTEZ_DEFAULT_SOURCE`.
#[derive(Clone)]
pub struct HttpBackend {
    client: NotezClient,
}

impl HttpBackend {
    pub fn from_env() -> Result<Self, String> {
        let base_url = std::env::var("NOTEZ_REMOTE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8700".to_string());
        let default_source = std::env::var("NOTEZ_DEFAULT_SOURCE").ok();
        let client = NotezClient::new(base_url).map_err(|e| e.to_string())?;
        let client = if let Some(source) = default_source {
            client.with_default_source(source)
        } else {
            client
        };
        Ok(Self { client })
    }
}

impl Backend for HttpBackend {
    async fn list_spaces(&self) -> Result<Vec<SpaceRow>, String> {
        // The HTTP API does not yet expose a list-spaces endpoint; the
        // mobile/desktop UI shows an empty state and instructs the user
        // to set NOTEZ_DEFAULT_SOURCE on the server. Same contract here.
        Ok(Vec::new())
    }

    async fn scan_space(&self, root: &str) -> Result<u32, String> {
        let mut client = self.client.clone();
        if !root.is_empty() {
            client = client.with_default_source(root);
        }
        let response = client
            .dispatch(&Request::ScanNative(ScanNativeRequest {}))
            .await
            .map_err(|e| e.to_string())?;
        match response {
            notez_protocol::Response::Scan(report) => Ok(report.scanned_resources as u32),
            other => Err(format!("unexpected scan response: {other:?}")),
        }
    }

    async fn query_resources(
        &self,
        root: &str,
        title_contains: Option<&str>,
        kind: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<ResourceRow>, String> {
        let mut client = self.client.clone();
        if !root.is_empty() {
            client = client.with_default_source(root);
        }
        let kind_str = kind.filter(|k| *k != "document").map(|k| k.to_string());
        let request = Request::QueryResources(QueryResourcesRequest {
            kind: kind_str,
            title_contains: title_contains.map(|s| s.to_string()),
            exact_ref: None,
            source_id: None,
            limit,
        });
        let response = client
            .dispatch(&request)
            .await
            .map_err(|e| e.to_string())?;
        match response {
            notez_protocol::Response::ResourcePage(page) => Ok(page
                .items
                .into_iter()
                .map(|r| ResourceRow {
                    id: r.ref_,
                    kind: format!("{:?}", r.kind),
                    title: r.title,
                    locator: r.locator,
                })
                .collect()),

            other => Err(format!("unexpected query response: {other:?}")),
        }
    }
}