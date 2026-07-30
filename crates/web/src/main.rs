//! `notez-web` binary — boots an axum server over a `notez` space.

use clap::Parser;

use web::server::serve;
use web::state::WebState;

#[derive(Parser, Debug)]
#[command(name = "notez-web", about = "Render a notez space over HTTP")]
struct Args {
    /// Path to the `notez` space directory.
    #[arg(long)]
    space: String,
    /// Bind address (e.g. `127.0.0.1:3030`).
    #[arg(long, default_value = "127.0.0.1:3030")]
    bind: String,
    /// Open the default browser at the server URL. Currently a no-op
    /// (logged only) — operators usually launch the browser themselves.
    #[arg(long, default_value_t = false)]
    open: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let state = WebState::from_space(std::path::Path::new(&args.space))?;
    serve(state, &args.bind, args.open).await
}