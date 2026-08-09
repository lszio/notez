# notez web (v0.2 — full operations in the UI)

The v0.1 reader-only web client (`/`, `/space/.../list`, `/space/.../resource/...`)
is **read-only**. v0.2 adds three operations that are reachable from the UI
without a hydrated client bundle:

- **Add space** — a `<details>` form on the home page that POSTs to
  `/api/spaces/register`. The server validates the path, writes the
  global XDG config, and `303 See Other`s to the new space's list page.
- **Scan** — a button on the list page that POSTs to
  `/api/spaces/scan`. The server runs `ApplicationService::scan_native`
  on the active space and `303`s back to the list page.
- **Watch** — a button on the list page that POSTs to
  `/api/spaces/watch/start` (inotify-backed `WatchService` in core).
  A second button stops the watch. The server's
  `GET /api/spaces/watch/state?path=...` returns the current state and
  the last 50 events as JSON; the watch panel on the list page links
  to it so the user can see live activity.

The `LaunchBuilder` flow is **not** used. `main.rs` switches to
`dioxus_server::serve` and builds a merged `axum::Router` containing
both the Dioxus SSR app and the custom POST routes. This is required
because the WASM client bundle is not built in this repo (it would
need a separate `cargo build --target wasm32-unknown-unknown` step
that pulls in the SQLite C bindings, which don't build for wasm32
without a C toolchain target).

## What this is NOT

There is no client-side hydration in v0.2. Every interaction is a
plain HTML form submit or anchor click. That is the right trade-off
for a reader that must work without a client bundle; the v0.2 surface
is sufficient to exercise all `notez` operations from a browser.

## Building and running

The Dioxus fullstack runtime still expects the WASM bundle at
`./public/assets/`. v0.2 ships **without** it, so the SSR pages render
but no JS runs in the browser. To rebuild with hydration later,
use `dx build --fullstack` from this directory (requires the `web`
feature to be in the default set; see `Cargo.toml`).

For the v0.2 reader:

```bash
cargo run -p web --bin web
# or via the justfile
just web
```

Then open <http://127.0.0.1:8765/>.

## Layout

```
web/
├─ assets/      # CSS + favicon
├─ src/
│  ├─ main.rs         # entry: builds axum router, starts server
│  ├─ lib.rs          # mounts the App() component
│  ├─ routes.rs       # custom POST routes (register / scan / watch)
│  ├─ server.rs       # Dioxus #[server] functions (list_resources, get_resource)
│  ├─ space_ctx.rs    # Signal<Option<SpaceState>> for the active space
│  ├─ router.rs       # Dioxus router + base64 URL-safe encoding
│  ├─ model.rs        # ResourceRow DTO
│  ├─ layout.rs       # global app shell + error banner
│  └─ pages/          # per-route components (home, list, detail, picker, ...)
```
