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

echo "Testing CLI agenda..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json agenda > "$TEMP_DIR/cli_agenda.json"

echo "Testing CLI inspect --rules..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json inspect heading:01J00000000000000000000301 --rules > "$TEMP_DIR/cli_rules.json"

echo "Testing CLI task transition..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json task transition heading:01J00000000000000000000301 --to DONE > "$TEMP_DIR/cli_transition.json"

echo "Testing MCP rules, agenda, task transition..."
cat <<EOF > "$TEMP_DIR/mcp_req.jsonl"
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "agenda", "arguments": {}}}
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "inspect_rules", "arguments": {"ref": "heading:01J00000000000000000000301"}}}
{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "task_transition", "arguments": {"ref": "heading:01J00000000000000000000301", "to": "DONE"}}}
EOF

"$NOTEZ_BIN" --space "$SPACE_DIR" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/mcp_rules_resp.json"

echo "Deleting SQLite index database..."
rm -f "$SPACE_DIR/.notez/index.sqlite"

echo "Rebuilding space index..."
"$NOTEZ_BIN" --space "$SPACE_DIR" space rebuild --json > /dev/null

echo "Executing post-rebuild CLI agenda..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json agenda > "$TEMP_DIR/post_cli_agenda.json"

echo "Comparing pre/post CLI agenda outputs..."
cmp -s "$TEMP_DIR/cli_agenda.json" "$TEMP_DIR/post_cli_agenda.json" || {
    echo "Error: CLI agenda mismatch after rebuild!"
    diff -u "$TEMP_DIR/cli_agenda.json" "$TEMP_DIR/post_cli_agenda.json"
    exit 1
}

echo "rules acceptance: PASS"
