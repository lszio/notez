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

cat <<EOF > "$SPACE_DIR/note.org"
#+title: Artifact Test Space
#+ID: 01J00000000000000000000077

* NEXT Core architecture sync
:PROPERTIES:
:ID: 01J00000000000000000000078
:TYPE: project
:END:
EOF

echo "Scanning space..."
"$NOTEZ_BIN" --space "$SPACE_DIR" scan --json > /dev/null

echo "Creating community via CLI..."
"$NOTEZ_BIN" --space "$SPACE_DIR" community create --id comm_core --name "CoreComm" --title-contains sync > /dev/null

echo "Deriving summary artifact..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json derive --community comm_core --recipe summary > "$TEMP_DIR/pre_summary.json"

echo "Exporting skill package..."
SKILL_DIR="$TEMP_DIR/exported_skill"
"$NOTEZ_BIN" --space "$SPACE_DIR" skill export --community comm_core --description "Core Skill" --out "$SKILL_DIR" > /dev/null

echo "Testing MCP artifact transcript..."
cat <<EOF > "$TEMP_DIR/mcp_req.jsonl"
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "derive_artifact", "arguments": {"space": "$SPACE_DIR", "community": "comm_core", "recipe": "summary"}}}
EOF

"$NOTEZ_BIN" --space "$SPACE_DIR" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/pre_mcp_art.json"

echo "Deleting SQLite index database..."
rm -f "$SPACE_DIR/.notez/index.sqlite"

echo "Rebuilding space index..."
"$NOTEZ_BIN" --space "$SPACE_DIR" space rebuild --json > /dev/null

echo "Executing post-rebuild artifact derivation..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json derive --community comm_core --recipe summary > "$TEMP_DIR/post_summary.json"

echo "Comparing pre/post summary outputs..."
cmp -s "$TEMP_DIR/pre_summary.json" "$TEMP_DIR/post_summary.json" || {
    echo "Error: Derived summary mismatch after rebuild!"
    diff -u "$TEMP_DIR/pre_summary.json" "$TEMP_DIR/post_summary.json"
    exit 1
}

echo "artifacts acceptance: PASS"
