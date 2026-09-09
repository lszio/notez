# notez web (`packages/web`)

The notez web surface: a server-rendered workspace plus the process host
for the protocol API and MCP.

One binary (`web`) mounts three routers on one axum server, sharing a
single composition `Runtime` (one engine cache + watcher per process):

| Router | Paths | Source |
|---|---|---|
| workspace UI | `/`, `/s/*`, `/raw/*`, `/new/*`, `/save/*`, `/scan/*`, `/register`, `/app.css` | `src/ui/` |
| web form endpoints | `/api/janet/eval` | `src/routes.rs` |
| protocol + MCP | `/api/v1/*`, `/mcp` | `crates/api`, `crates/mcp` |

## The UI

Plain server-rendered HTML. No wasm, no hydration, no client framework,
no `#[server]` functions: a click is a normal navigation and a write is
a normal `<form method="post">`. The only JavaScript is ~40 lines inline
(page filter, editor shortcuts, unsaved-changes guard).

- **Sidebar** — every document and attachment in the space, filterable
  (`/` focuses the box). Documents and attachments are listed
  separately; titles come from the projection where indexed.
- **Reading** — `/s/<space>/<locator>` renders markdown/org through the
  `notez-preview` catalog. Relative links and images are rewritten to
  `/s/...` and `/raw/...`, resolved against the document's directory.
- **Editing** — `?edit=1` opens a textarea; `Ctrl/Cmd-S` saves through
  `POST /save/<space>` with the content hash as `expected_revision`. A
  conflict returns 409, keeps your text, and switches the revision to
  the on-disk one so a second save is a deliberate overwrite.
- **Attachments** — any non-document path renders a preview (image,
  PDF, Office, CSV, archive, …) with a raw-bytes link.

Spaces are addressed by base64url-encoded absolute path so one server
can serve several roots. `/` redirects to the only registered space, or
shows the picker (`POST /register` writes the global config).

## Running

```bash
just web                 # debug build + run on 127.0.0.1:8765
just web RELEASE=1       # release build — recommended for daily use
NOTEZ_SPACE_ROOT=~/Notes just web
```

`NOTEZ_MODE=server` runs the same binary headless (protocol API + MCP
only, no UI).

## Layout

```text
src/main.rs     binary entry: bind + router assembly + serve
src/host.rs     host assembly (UI + API + MCP against one Runtime)
src/ui/         the workspace UI (mod.rs router/handlers, page.rs HTML,
                space.rs data access, urls.rs URL helpers, style.css)
src/body.rs     PreviewModel -> HTML (delegates to notez-preview)
src/routes.rs   process-global state, source registration, Janet eval
src/server.rs   projection query helpers + save outcome types
src/model.rs    ResourceRow view model
src/janet.rs    restricted Janet runtime
```

## Tests

`cargo test -p web` covers URL helpers, the HTML rewriter, the locator
index merge, save-failure serialization and the Janet sandbox. The
end-to-end surface is covered by `scripts/acceptance-web.sh`
(picker → list → render → save → raw → stale-revision 409).
