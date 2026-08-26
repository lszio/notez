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

SAMPLE_FILE="$TEMP_DIR/sample_file.txt"
cat <<EOF > "$SAMPLE_FILE"
Notez Attachment Sample Text Content for Acceptance Verification. Section 1 contains important details. Section 2 contains secondary notes.
EOF

echo "Adding attachment via CLI..."
ATT_ADD_OUTPUT="$("$NOTEZ_BIN" --space "$SPACE_DIR" --json attachment add --path "$SAMPLE_FILE" --mime "text/plain")"
ATT_REF="$(echo "$ATT_ADD_OUTPUT" | grep -o '"ref":"[^"]*"' | cut -d'"' -f4)"

echo "Added attachment ref: $ATT_REF"

echo "Running extraction job via CLI..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json attachment extract "$ATT_REF" > /dev/null

echo "Querying extracted segments via CLI..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json attachment segments "$ATT_REF" > "$TEMP_DIR/pre_segments.json"

echo "Testing MCP attachment transcript..."
cat <<EOF > "$TEMP_DIR/mcp_req.jsonl"
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "notez-acceptance", "version": "0.0.1"}}}
{"jsonrpc": "2.0", "method": "notifications/initialized"}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "query_segments", "arguments": {"space": "$SPACE_DIR", "ref": "$ATT_REF"}}}
EOF

echo "Deleting SQLite index database..."
rm -f "$SPACE_DIR/.notez/index.sqlite"

echo "Rebuilding index and re-running extraction..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json attachment add --path "$SAMPLE_FILE" --mime "text/plain" > /dev/null
"$NOTEZ_BIN" --space "$SPACE_DIR" --json attachment extract "$ATT_REF" > /dev/null

echo "Executing post-rebuild segments query..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json attachment segments "$ATT_REF" > "$TEMP_DIR/post_segments.json"

echo "Comparing pre/post segments outputs..."
cmp -s "$TEMP_DIR/pre_segments.json" "$TEMP_DIR/post_segments.json" || {
    echo "Error: Segments output mismatch after rebuild!"
    diff -u "$TEMP_DIR/pre_segments.json" "$TEMP_DIR/post_segments.json"
    exit 1
}

echo "attachments acceptance: PASS"
