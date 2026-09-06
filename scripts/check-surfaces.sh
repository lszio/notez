#!/usr/bin/env bash
# Verify all form-factor compilation paths before pushing.
#
# This is the local equivalent of the GitHub Actions `surfaces` job:
#   - `cargo check -p desktop --features desktop`
#   - `cargo check -p mobile  --features mobile`
# plus a wasm client check via `dx build --platform web` when `dx` is
# installed. The wasm client build is currently broken (Dioxus 0.7.10
# hydration walk crashes; tracked in packages/web/src/main.rs), so we
# only attempt it when the operator explicitly opts in via
# NOTEZ_WASM_GATE=1.

set -euo pipefail

echo "== cargo check -p desktop --features desktop =="
cargo check -p desktop --features desktop

echo
echo "== cargo check -p mobile  --features mobile =="
cargo check -p mobile  --features mobile

if [[ "${NOTEZ_WASM_GATE:-0}" == "1" ]]; then
    if ! command -v dx >/dev/null 2>&1; then
        echo "dx not on PATH; install dioxus-cli to enable wasm gate"
        exit 1
    fi
    dx build --platform web --package web --bin web
    echo "== dx build --platform web =="
    dx build --platform web --package web --bin web
fi