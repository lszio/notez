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

SPACE_A="$TEMP_DIR/space_a"
SPACE_B="$TEMP_DIR/space_b"
SHARED="$TEMP_DIR/shared"

mkdir -p "$SPACE_A" "$SPACE_B" "$SHARED"

cat <<EOF > "$SPACE_A/shared_note.org"
#+title: Shared Sync Note
#+ID: 01J00000000000000000000088
* NEXT Shared item
:PROPERTIES:
:ID: 01J00000000000000000000089
:END:
EOF

echo "Pushing from Space A..."
"$NOTEZ_BIN" --space "$SPACE_A" --json sync push --actor actor_a --folder "$SHARED" > "$TEMP_DIR/pre_push.json"

echo "Pulling into Space B..."
"$NOTEZ_BIN" --space "$SPACE_B" --json sync pull --actor actor_b --folder "$SHARED" > "$TEMP_DIR/pre_pull.json"

echo "Testing MCP sync transcript..."
cat <<EOF > "$TEMP_DIR/mcp_req.jsonl"
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "sync_push", "arguments": {"space": "$SPACE_A", "actor": "actor_a", "folder": "$SHARED"}}}
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "sync_pull", "arguments": {"space": "$SPACE_B", "actor": "actor_b", "folder": "$SHARED"}}}
EOF

"$NOTEZ_BIN" --space "$SPACE_B" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/mcp_sync_resp.json"

echo "Deleting SQLite index on Space B..."
rm -f "$SPACE_B/.notez/index.sqlite"

echo "Rebuilding Space B index and pulling..."
"$NOTEZ_BIN" --space "$SPACE_B" space rebuild --json > /dev/null
"$NOTEZ_BIN" --space "$SPACE_B" --json sync pull --actor actor_b --folder "$SHARED" > "$TEMP_DIR/post_pull.json"

echo "Verifying synced file on Space B..."
if [ ! -f "$SPACE_B/shared_note.org" ]; then
    echo "Error: Synced file missing on Space B!"
    exit 1
fi

echo "sync acceptance: PASS"
