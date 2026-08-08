use dioxus::LaunchBuilder;
use notez_web::app;

fn main() {
    // Dioxus fullstack's axum runtime reads `IP` and `PORT` env vars
    // (see dioxus_cli_config::fullstack_address_or_localhost). We set
    // a sane default when the operator did not pin one.
    if std::env::var("PORT").is_err() {
        // SAFETY: this is a single-threaded early-init before any
        // worker threads are spawned, so no other thread can observe
        // the env var in a partially-updated state.
        unsafe {
            std::env::set_var("PORT", "8765");
        }
    }
    if std::env::var("IP").is_err() {
        // SAFETY: see above.
        unsafe {
            std::env::set_var("IP", "127.0.0.1");
        }
    }

    LaunchBuilder::new().launch(app);
}
