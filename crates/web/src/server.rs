//! `serve()` entry point. Builds the axum router from the provided state
//! and listens on `bind` until interrupted.
//!
//! Browser launching is intentionally a no-op (logged only) — opening the
//! default browser is left to the operator's shell.

use std::net::SocketAddr;

use crate::router::router;
use crate::state::WebState;

pub async fn serve(state: WebState, bind: &str, open: bool) -> anyhow::Result<()> {
    let app = router(state);
    let addr: SocketAddr = bind.parse()?;
    tracing::info!(%addr, "notez web listening");
    if open {
        let url = format!("http://{addr}/");
        tracing::info!(%url, "would open browser at (open=false in this build)");
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}