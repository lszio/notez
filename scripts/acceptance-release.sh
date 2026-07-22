#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

echo "=== Running Full Notez Release Acceptance Suite ==="

echo "[1/6] Running Core Acceptance..."
bash "$SCRIPT_DIR/acceptance-core.sh"

echo "[2/6] Running Rules Acceptance..."
bash "$SCRIPT_DIR/acceptance-rules.sh"

echo "[3/6] Running Federation Acceptance..."
bash "$SCRIPT_DIR/acceptance-federation.sh"

echo "[4/6] Running Attachments Acceptance..."
bash "$SCRIPT_DIR/acceptance-attachments.sh"

echo "[5/6] Running Artifacts Acceptance..."
bash "$SCRIPT_DIR/acceptance-artifacts.sh"

echo "[6/6] Running Sync Acceptance..."
bash "$SCRIPT_DIR/acceptance-sync.sh"

echo "=================================================="
echo "release acceptance: PASS"
