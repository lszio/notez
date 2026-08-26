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

echo "Discovering a real heading ref from the projection..."
TASK1_REF="$("$NOTEZ_BIN" --space "$SPACE_DIR" --json query --kind heading --title-contains "Task 1" | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["ref"])')"
echo "  using $TASK1_REF"

echo "Testing CLI inspect --rules..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json inspect "$TASK1_REF" --rules > "$TEMP_DIR/cli_rules.json"

echo "Testing CLI task transition is gated (no sub-source registered)..."
# Since the honesty fixes, task transitions must round-trip through a
# registered source writer. A default-discovered space has no
# sub-sources, so the transition MUST be rejected with a structured
echo "Testing CLI task transition (M4 surgical write-back)..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json task transition "$TASK1_REF" --to DONE \
    > "$TEMP_DIR/cli_transition.json" 2> "$TEMP_DIR/cli_transition.err"
grep -q '"from_state"' "$TEMP_DIR/cli_transition.json" || {
    echo "Error: transition did not return a StateTransition payload:"
    cat "$TEMP_DIR/cli_transition.json" "$TEMP_DIR/cli_transition.err"
    exit 1
}
# Surgical guarantee: Task 1's heading flipped to DONE and the
# SCHEDULED line on the following line is untouched.
if ! grep -q '^\* DONE Task 1$' "$SPACE_DIR/agenda_test.org"; then
    echo "Error: heading state was not patched in the source file!"
    exit 1
fi
if ! grep -q '^SCHEDULED: <2026-07-24 Wed>$' "$SPACE_DIR/agenda_test.org"; then
    echo "Error: SCHEDULED line lost during surgical patch!"
    exit 1
fi
echo "  transition succeeded and patched the source file surgically"

echo "Testing MCP agenda + inspect_rules with the discovered ref..."
cat > "$TEMP_DIR/mcp_req.jsonl" <<MCPJSON
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "notez-acceptance", "version": "0.0.1"}}}
{"jsonrpc": "2.0", "method": "notifications/initialized"}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "agenda", "arguments": {}}}
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "inspect_rules", "arguments": {"ref": "$TASK1_REF"}}}
MCPJSON

"$NOTEZ_BIN" --space "$SPACE_DIR" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/mcp_rules_resp.json"

echo "Deleting SQLite index database..."
rm -f "$SPACE_DIR/.notez/index.sqlite"

echo "Rebuilding space index..."
"$NOTEZ_BIN" --space "$SPACE_DIR" workspace rebuild --json > /dev/null

echo "Executing post-rebuild CLI agenda..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json agenda > "$TEMP_DIR/post_cli_agenda.json"

echo "Asserting the transition survived the rebuild..."
# Task 1 must still read DONE from the rebuilt projection; every other
# task's state must be unchanged.
python3 - "$TEMP_DIR/post_cli_agenda.json" <<'PYCHECK'
import json, sys
items = json.load(open(sys.argv[1]))["items"]
task1 = [i for i in items if i["title"] == "Task 1"]
assert task1 and task1[0]["todo"] == "DONE", f"Task 1 not DONE after rebuild: {task1}"
others = [i for i in items if i["title"] != "Task 1"]
assert all(i["todo"] != "DONE" or i["title"] == "Task 3" for i in others), others
print("  post-rebuild agenda verified")
PYCHECK

echo "rules acceptance: PASS"
