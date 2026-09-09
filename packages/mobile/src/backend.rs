//! `HttpBackend` — mobile-side implementation of [`ui::Backend`].
//!
//! Talks to a running notez server over the HTTP protocol API via
//! `notez_api::NotezClient`. The mobile surface (iOS / Android Dioxus
//! shells) is otherwise embed-capable, but `HttpBackend` is the
//! current default — it keeps the mobile binary lean (no rusqlite /
//! janetrs / notify linked) and lets a single desktop-class notez host
//! serve many devices.
//!
//! Source resolution: every call carries the space path the user is
//! currently looking at; we send it as `X-Notez-Source` so the server
//! resolves it through its own composition runtime. If the host has
//! `NOTEZ_API_SOURCE` set, `client.with_default_source(...)` can pin
//! a default and we can pass empty string for unselected calls.


use notez_api::NotezClient;
use notez_protocol::request::{
    QueryResourcesRequest, Request, ScanNativeRequest,
};

use ui::{Backend, ResourceRow, SpaceRow};

/// Backend that talks to a remote notez HTTP API server.
#[derive(Clone)]
pub struct HttpBackend {
    client: NotezClient,
}

impl HttpBackend {
    /// Build from a `NOTEZ_REMOTE_URL` style base URL (`http://host:port`).
    /// Pass `Some(source)` to pin `X-Notez-Source` for every request;
    /// `None` leaves it to per-call `X-Notez-Source` headers (we use
    /// the same default via `with_default_source`).
    pub fn new(base_url: impl Into<String>, default_source: Option<&str>) -> Result<Self, String> {
        let client = NotezClient::new(base_url).map_err(|e| e.to_string())?;
        let client = if let Some(source) = default_source {
            client.with_default_source(source.to_string())
        } else {
            client
        };
        Ok(Self { client })
    }


    /// Convenience constructor that reads the URL from the environment
    /// (`NOTEZ_REMOTE_URL`, default `http://127.0.0.1:8700`) and the
    /// default source from `NOTEZ_DEFAULT_SOURCE`.
    pub fn from_env() -> Result<Self, String> {
        let base_url =
            std::env::var("NOTEZ_REMOTE_URL").unwrap_or_else(|_| "http://127.0.0.1:8700".to_string());
        let default_source = std::env::var("NOTEZ_DEFAULT_SOURCE").ok();
        Self::new(base_url, default_source.as_deref())
    }
}

impl Backend for HttpBackend {
    async fn list_spaces(&self) -> Result<Vec<SpaceRow>, String> {
        // The HTTP API doesn't expose a list-spaces endpoint yet;
        // mobile hosts MUST set `NOTEZ_DEFAULT_SOURCE` on the server so
        // a workspace entry is available. The view will show an empty
        // state and instruct the user to configure the server otherwise.
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