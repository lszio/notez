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
VAULT_DIR="$TEMP_DIR/vault"
GIT_DIR="$TEMP_DIR/git_repo"

mkdir -p "$SPACE_DIR" "$VAULT_DIR" "$GIT_DIR/.git"

# 1. Native Org Note
cat <<EOF > "$SPACE_DIR/index.org"
#+title: Native Space Index
#+ID: 01J00000000000000000000001
* NEXT Native task
:PROPERTIES:
:ID: 01J00000000000000000000002
:END:
EOF

# 2. Obsidian Vault Note (.md)
cat <<EOF > "$VAULT_DIR/vault_note.md"
---
title: External Vault Note
id: 01J00000000000000000000003
type: project
---
# Vault Section <!-- id: 01J00000000000000000000004 -->
See [[id:01J00000000000000000000002][Native task]].
EOF

# 3. Git Repo Note (.org)
cat <<EOF > "$GIT_DIR/git_note.org"
#+title: Git Mounted Note
#+ID: 01J00000000000000000000005
EOF

echo "Adding external sources via CLI..."
"$NOTEZ_BIN" --space "$SPACE_DIR" source add --id vault --kind obsidian --path "$VAULT_DIR" --read-only > /dev/null
"$NOTEZ_BIN" --space "$SPACE_DIR" source add --id git_repo --kind git --path "$GIT_DIR" --read-only > /dev/null

echo "Syncing federated sources..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json source sync > "$TEMP_DIR/pre_sync.json"

echo "Querying cross-source index..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json query > "$TEMP_DIR/pre_query.json"

echo "Testing MCP source_list and query transcript..."
cat <<EOF > "$TEMP_DIR/mcp_req.jsonl"
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "source_list", "arguments": {"space": "$SPACE_DIR"}}}
{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "query", "arguments": {}}}
EOF

"$NOTEZ_BIN" --space "$SPACE_DIR" mcp serve < "$TEMP_DIR/mcp_req.jsonl" > "$TEMP_DIR/pre_mcp.json"

echo "Deleting SQLite index database..."
rm -f "$SPACE_DIR/.notez/index.sqlite"

echo "Rebuilding federated space index..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json source sync > "$TEMP_DIR/post_sync.json"

echo "Executing post-rebuild query..."
"$NOTEZ_BIN" --space "$SPACE_DIR" --json query > "$TEMP_DIR/post_query.json"

echo "Comparing pre/post query outputs..."
cmp -s "$TEMP_DIR/pre_query.json" "$TEMP_DIR/post_query.json" || {
    echo "Error: Federation query mismatch after rebuild!"
    diff -u "$TEMP_DIR/pre_query.json" "$TEMP_DIR/post_query.json"
    exit 1
}

echo "federation acceptance: PASS"
