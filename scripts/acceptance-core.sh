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
cp -r "$PROJECT_ROOT/tests/fixtures/space" "$SPACE_DIR"

echo "Scanning space..."
"$NOTEZ_BIN" --space "$SPACE_DIR" scan --json > /dev/null

echo "Executing pre-rebuild CLI query..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json query --kind heading --title-contains sync > "$TEMP_DIR/pre_cli.json"

echo "Executing pre-rebuild MCP transcript..."
cat <<EOF > "$TEMP_DIR/mcp_req.jsonl"
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "notez-acceptance", "version": "0.0.1"}}}
{"jsonrpc": "2.0", "method": "notifications/initialized"}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "query", "arguments": {"kind": "heading", "title_contains": "sync"}}}
EOF

"$NOTEZ_BIN" --space "$SPACE_DIR" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/pre_mcp.json"

echo "Deleting SQLite index database..."
rm -f "$SPACE_DIR/.notez/index.sqlite"

echo "Rebuilding space index..."
"$NOTEZ_BIN" --space "$SPACE_DIR" workspace rebuild --json > /dev/null

echo "Executing post-rebuild CLI query..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json query --kind heading --title-contains sync > "$TEMP_DIR/post_cli.json"

echo "Executing post-rebuild MCP transcript..."
"$NOTEZ_BIN" --space "$SPACE_DIR" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/post_mcp.json"

echo "Comparing pre/post CLI outputs..."
cmp -s "$TEMP_DIR/pre_cli.json" "$TEMP_DIR/post_cli.json" || {
    echo "Error: CLI output mismatch after rebuild!"
    diff -u "$TEMP_DIR/pre_cli.json" "$TEMP_DIR/post_cli.json"
    exit 1
}

echo "Comparing pre/post MCP outputs..."
cmp -s "$TEMP_DIR/pre_mcp.json" "$TEMP_DIR/post_mcp.json" || {
    echo "Error: MCP output mismatch after rebuild!"
    diff -u "$TEMP_DIR/pre_mcp.json" "$TEMP_DIR/post_mcp.json"
    exit 1
}

echo "core acceptance: PASS"
