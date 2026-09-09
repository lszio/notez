# notez web (`packages/web`)

The notez web surface: a Dioxus-rendered workspace plus the process
host for the protocol API and MCP.

One binary (`web`) mounts three routers on one axum server, sharing a
single composition `Runtime` (one engine cache + watcher per process):

| Router | Paths | Source |
|---|---|---|
| workspace pages | `/s/*` | `src/app/` (Dioxus SSR) |
| data + utility endpoints | `/`, `/raw/*`, `/save/*`, `/scan/*`, `/new/*`, `/register`, `/app.css`, `/app.js`, `/api/render`, `/api/space-status`, `/api/janet/eval` | `src/data/` |
| protocol + MCP | `/api/v1/*`, `/mcp` | `crates/api`, `crates/mcp` |

## Rendering model

Pages are **Dioxus components rendered server-side**, but data is read
*synchronously during the render* — there are no `#[server]` functions
and no client-side data fetching, so the HTML the browser receives is
complete (the old "stuck on Loading…" failure mode is impossible) and
there is nothing to hydrate.

The presentational core lives in `packages/ui/src/workspace.rs`
(`NzShell`, `NzTree`, `NzDocBody`, `build_tree`) so desktop and mobile
can render the same workspace natively. Data loading stays per surface.

Interaction that HTML cannot express lives in one small island
(`src/data/app.js`):

- sidebar filter (`/` focuses it) and per-space folder open state;
- editor toolbar, `Ctrl/Cmd-S`, Tab indent, localStorage drafts, and the
  live preview (`POST /api/render` uses the same renderer as the read
  page, so preview and saved page can never disagree);
- "changed on disk — reload" pill driven by `/api/space-status`.

## Pages

| Route | What it does |
|---|---|
| `/` | redirects to the configured default source, else the space picker |
| `/s/{space}` | README/index page when present, otherwise the page list |
| `/s/{space}/{locator}` | read a document, preview an attachment |
| `/s/{space}/{locator}?edit=1` | editor: toolbar + textarea + live preview |
| `/s/{space}/new` | new-note form |
| `GET /raw/{space}/{locator}` | raw bytes (images, PDFs, downloads) |
| `POST /save/{space}` | revision-guarded save (content sha256) |
| `POST /scan/{space}` | re-index, then return to `next` |
| `POST /register` | register a directory in the global config |

Spaces are addressed by base64url-encoded absolute path; locators are
percent-encoded path segments, so URLs stay readable
(`/s/<space>/projects/index.org`).

## Editing

`POST /save/{space}` goes through `UpdateDocument` (journal + revision
guard + projection refresh). On success it redirects to the read page
with a `?saved=<revision>` banner. On failure it stashes the submitted
text under a one-shot token and redirects to
`?edit=1&restore=<token>`; the editor restores exactly what the user
typed, shows the conflict banner, and switches to the on-disk revision
so a second save is a deliberate overwrite. Nothing is lost, with or
without JavaScript.

## Running

```bash
just web                 # release build + run on 127.0.0.1:8765
just web DEV=1           # debug build
NOTEZ_SPACE_ROOT=~/Notes just web
```

`NOTEZ_MODE=server` runs the same binary headless (protocol API + MCP
only, no UI). The Dioxus SSR renderer needs `public/index.html` next to
the binary; `build.rs` copies `packages/web/public/` into the target
profile directory (the Dockerfile copies it to `/usr/local/bin/public`).

## Layout

```text
src/main.rs     binary entry: bind + router assembly + serve
src/host.rs     host assembly (UI + API + MCP against one Runtime)
src/app/        Dioxus pages (mod, route, shell, pages, edit)
src/data/       space/file/document access, URL helpers, HTML rewriting,
                embedded style.css + app.js, utility axum routes
src/body.rs     PreviewModel -> HTML (delegates to notez-preview)
src/routes.rs   process-global state, source registration, Janet eval
src/server.rs   projection query helpers + save outcome types
src/model.rs    ResourceRow view model
src/janet.rs    restricted Janet runtime
```

## Tests

`cargo test -p web` covers URL helpers, the HTML rewriter, the locator
index merge, tree building (in `ui`), save-failure handling and the
Janet sandbox. The end-to-end surface is `scripts/acceptance-web.sh`
(picker → tree → render → save → raw → conflict + text restore).
