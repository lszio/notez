use app::app;
use dioxus::LaunchBuilder;

fn main() {
    // NOTEZ_BIND is read by Dioxus fullstack's axum runtime internally.
    // We only set a sane default when the operator did not pin one.
    if std::env::var("NOTEZ_BIND").is_err() {
        // SAFETY: called before any threads are spawned, so no concurrent
        // readers of the environment can observe a partial state.
        unsafe {
            std::env::set_var("NOTEZ_BIND", "127.0.0.1:3030");
        }
    }

    LaunchBuilder::new()
        .launch(app);
}
