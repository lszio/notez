# Workspace rebuild — Task 1 impact baseline

Date: 2026-09-07

## GitNexus availability

The repository contains no `.gitnexus/` directory and no project-local GitNexus impact-analysis skill. The required GitNexus `impact(..., direction: upstream)` analysis cannot run in this worktree. This document records the required conservative fallback: symbol-specific `rg` caller discovery. Each migration task must repeat the analysis immediately before changing its target symbol.

## Public seam callers found by fallback search

| Seam | Direct callers / affected surfaces | Conservative risk | Migration gate |
|---|---|---:|---|
| `notez_core::domain::Resource` | Core integration tests; Markdown/Org adapter API tests; CLI resource commands; Web models/routes; preview builders | HIGH | Extract pure domain types first; keep `notez-core` API bridge until all consumers move. |
| `notez_core::application::Engine` | Core tests; API transport; MCP server/tests; CLI handlers; composition Runtime; Web/desktop backends | CRITICAL | Introduce `notez-engine::Dispatcher` and `composition::RuntimeHandle` before removing raw `Engine` exposure. |
| `notez_core::application::dispatcher::Dispatcher` | Indirectly consumed by core use cases, API, MCP and UI transport through `Engine` | HIGH | Move request handling only after protocol/ports contracts and parity tests exist. |
| `notez_composition::native::Runtime` | API transport, Web routes/backend, desktop backend, CLI host | CRITICAL | Retain public Runtime adapter until API/Web/Desktop all use a typed `RuntimeHandle`. |
| `ui::Backend` | Web server/host/backend, desktop backend, mobile backend and their page callers | HIGH | Extend through a compatibility adapter and Backend contract tests before deleting current methods. |

## Task 1 decision

Task 1 modifies only `Cargo.toml` workspace membership and creates empty target crate roots. It intentionally does not change the five high-fan-out seams. The follow-up Task 2 through Task 8 each have a separate compile/test gate before changing their respective seam.
