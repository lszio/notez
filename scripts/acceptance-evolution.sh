#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

echo "Building notez binary..."
cargo build

NOTEZ_BIN="$PROJECT_ROOT/target/debug/notez"

SPACE_DIR="$TEMP_DIR/space"
mkdir -p "$SPACE_DIR"

cat <<EOF > "$SPACE_DIR/block_note.org"
#+title: Evolution Block Note
#+ID: 01J00000000000000000000033

* NEXT Evolution block
:PROPERTIES:
:ID: 01J00000000000000000000034
:END:

#+NAME: src_block_evolution
#+BEGIN_SRC rust
fn evolution_block() {}
#+END_SRC
EOF

echo "Scanning space for block entities..."
"$NOTEZ_BIN" --space "$SPACE_DIR" scan --json > /dev/null

echo "Querying block entities via CLI..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json query --kind block > "$TEMP_DIR/blocks.json"

echo "Verifying block presence in query output..."
if ! grep -q '"kind":"block"' "$TEMP_DIR/blocks.json"; then
    echo "Error: No block entity found!"
    exit 1
fi

echo "Testing MCP heading query (valid kind) and asserting output..."
cat <<EOF > "$TEMP_DIR/mcp_req.jsonl"
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "notez-acceptance", "version": "0.0.1"}}}
{"jsonrpc": "2.0", "method": "notifications/initialized"}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "query", "arguments": {"kind": "heading"}}}
EOF

"$NOTEZ_BIN" --space "$SPACE_DIR" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/mcp_blocks.json"

if ! grep -q 'heading:01J00000000000000000000034' "$TEMP_DIR/mcp_blocks.json"; then
    echo "Error: MCP query did not surface the seeded heading ref!"
    exit 1
fi

echo "evolution acceptance: PASS"
