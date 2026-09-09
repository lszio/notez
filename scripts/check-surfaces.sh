#!/usr/bin/env bash
# Verify all form-factor compilation paths before pushing.
#
# This is the local equivalent of the GitHub Actions `surfaces` job:
#   - `cargo check -p desktop --features desktop`
#   - `cargo check -p mobile  --features mobile`
#   - `cargo check -p web` (server-rendered workspace + API/MCP host)
#
# The web surface no longer has a wasm client: it is plain SSR, so
# there is nothing to build with `dx`.

set -euo pipefail

echo "== cargo check -p desktop --features desktop =="
cargo check -p desktop --features desktop

echo
echo "== cargo check -p mobile  --features mobile =="
cargo check -p mobile  --features mobile

echo
echo "== cargo check -p web =="
cargo check -p web --bin web
