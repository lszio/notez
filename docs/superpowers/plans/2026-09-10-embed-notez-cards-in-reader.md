# Embed Notez Cards in Reader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render `notez` Janet card results inline in Markdown/Org Reader and live Editor preview, so `projects/index.org` and `areas/index.org` visibly show dynamic lists/statistics instead of only source blocks.

**Architecture:** Keep parsing/execution in `notez-core::CardExecutionService` and make Web rendering a projection step. The Web body renderer will detect `notez` blocks through the existing core parser, execute them with the same Engine-bound card service, and replace each source block with a sanitized result or structured failure. Reader and `/api/render` will use one rendering entry point; source editing remains unchanged.

**Tech Stack:** Rust, Dioxus SSR, Axum preview endpoint, `notez-core` card executor, existing Org/Markdown previewers, HTML escaping/sanitization.

---

## Scope

Included:

- Inline rendering of `notez` card blocks in document Reader.
- Same inline rendering in Editor live preview.
- `list` output as navigable object rows showing title and locator.
- `html` output as sanitized HTML.
- `json` / `object` output as escaped diagnostic content or object link.
- Failed/timeout/denied card state as visible, non-fatal inline status.
- Tests for Projects/Areas-style prefix queries and reader rendering.

Excluded:

- Block editing or AST span patches.
- JavaScript execution.
- New query language or new Janet APIs.
- Remote writes, whiteboards, dashboard layout persistence.

## File map

- `packages/web/src/body.rs` — document body rendering and card replacement boundary.
- `packages/web/src/data/space.rs` — pass source root/locator through the same render path for Reader and live preview.
- `packages/web/src/janet.rs` — only if a small Web-specific CardProjection-to-HTML adapter is needed; do not duplicate execution policy.
- `packages/web/src/app/edit.rs` — no behavior change expected; verify it calls the unified renderer.
- `packages/web/src/body.rs` tests — card output and failure-state regression tests.
- `packages/web/tests/*` or existing top-level integration target — one observable Reader rendering test if current test conventions require a separate target.

### Task 1: Add failing inline card rendering tests

- [ ] Add a body-render test fixture containing an Org `notez` card with `:card yes`, `:id`, `:output list`, and a query that returns an object list. Assert the rendered HTML contains the card id/title and at least one object locator, not the raw `#+begin_src` block.
- [ ] Add a Markdown fixture with ```` ```notez ... ``` ```` and `{:type "html" :value "<p>dynamic</p>"}` output through the real Janet executor. Assert the rendered result includes `<p>dynamic</p>` and strips `<script>`.
- [ ] Add a failed-card fixture (`{:type "wat"}`) and assert Reader HTML contains a visible failed state while still containing surrounding document text.
- [ ] Run the focused Web tests and observe failure because `render_body` currently sends raw `notez` blocks to the legacy previewer and has no Engine card execution context.

### Task 2: Introduce one Web card projection adapter

- [ ] Define a small internal function in `packages/web/src/body.rs`:

```rust
fn render_notez_cards(
    source_root: &Path,
    locator: &str,
    content: &str,
) -> String
```

It must:

1. Select Org or Markdown parser from `locator`.
2. Parse only existing `notez` blocks; leave ordinary code blocks unchanged.
3. Open the bound Engine through the existing Web runtime for the source root, not a second Engine cache.
4. Execute each block with `Engine::card_service().run(&block, &CardExecutionContext::for_locator(locator))`.
5. Convert `CardProjection`/`CardState` to escaped HTML:
   - `list`: `<section class="notez-card ..."><h3>...</h3><ul>...</ul></section>`; escape title/text and make known object refs/locators normal Notez links.
   - `html`: use the already sanitized `CardOutput::Html` string; do not sanitize after string interpolation as a substitute for escaping other fields.
   - `json`: pretty-print and HTML-escape in `<pre>`.
   - `object`: escape the ref and render a normal object link.
   - failed/timeout/denied/stale: render a status section with escaped error text.
6. Replace source spans from right to left, preserving text around each block.

Do not call Janet directly from Web. Do not accept arbitrary HTML from raw source; only `CardOutput::Html` may enter as sanitized HTML.

- [ ] Run the focused body tests; expected PASS.

### Task 3: Wire Reader and Editor preview to the adapter

- [ ] Change `render_document` in `packages/web/src/data/space.rs` to run the card projection before `render_body`, using the same `content` for live preview and on-disk content for Reader.
- [ ] Ensure the rendered card HTML survives the Markdown/Org preview conversion without being escaped or executed as source code. Use a placeholder marker strategy if the underlying previewer escapes raw HTML.
- [ ] Keep relative document links and raw attachment links rewritten after card replacement.
- [ ] Verify `packages/web/src/app/edit.rs` remains unchanged except for tests; its `render_document` call must show the same dynamic result as Reader.
- [ ] Run `cargo test -p web --lib` and the focused card tests.

### Task 4: Verify actual Projects and Areas documents

- [ ] Run `notez scan` against `/home/lszio/Notes`.
- [ ] Render `projects/index.org` and `areas/index.org` through the Web rendering path using a temporary SSR request or existing Web smoke helper.
- [ ] Assert both pages show dynamic card output and no raw `#+begin_src` block in the rendered document body.
- [ ] Verify a card execution failure does not remove the surrounding title/body or make the page return 500.

### Task 5: Final regression and review

- [ ] Run `cargo check --workspace`.
- [ ] Run `cargo test --workspace`.
- [ ] Run `git diff --check`.
- [ ] Review output for HTML escaping, duplicate Engine/Runtime creation, direct Janet bypasses, and changes outside Web rendering/tests.

## Self-review

- All approved behavior is covered: dynamic object list, dynamic HTML, safe failures, Reader/Editor parity, Projects/Areas verification.
- No new protocol or persistence contract is introduced.
- The only intentional execution authority remains `CardExecutionService`; Web only renders its typed result.
- No placeholders or unspecified APIs are left in the plan.
