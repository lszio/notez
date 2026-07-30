#!/usr/bin/env bash
# Acceptance test for the `notez web` subcommand.
#
# Builds `notez` with the `web` cargo feature, seeds a tiny space, runs a
# scan, then launches the server in the background. Verifies:
#
#   * /healthz returns "ok"
#   * /             renders the space picker
#   * /s/<space>/   renders the hub page (the seed file's title appears)
#
# Finally tears the server down.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

echo "Building notez with web feature..."
cargo build -p cli --features web

NOTEZ_BIN="$PROJECT_ROOT/target/debug/notez"

BIND="127.0.0.1:3939"
BASE="http://$BIND"

SPACE_DIR="$(mktemp -d -t notez-web-XXXXXX)"
PID=""
cleanup() {
  if [[ -n "$PID" ]] && kill -0 "$PID" 2>/dev/null; then
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true
  fi
  rm -rf "$SPACE_DIR"
}
trap cleanup EXIT

echo "Seeding space at $SPACE_DIR..."
mkdir -p "$SPACE_DIR/projects"
cat > "$SPACE_DIR/README.org" <<EOF
#+TITLE: Web Acceptance Space
* hello world
EOF

echo "Scanning space..."
"$NOTEZ_BIN" --space "$SPACE_DIR" scan --json > /dev/null

echo "Launching notez web on $BIND..."
"$NOTEZ_BIN" --space "$SPACE_DIR" web --bind "$BIND" > /tmp/notez-web.log 2>&1 &
PID=$!

echo "Waiting for /healthz ..."
for _ in $(seq 1 30); do
  if curl -fsS "$BASE/healthz" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done

HEALTH=$(curl -fsS "$BASE/healthz")
if [[ "$HEALTH" != "ok" ]]; then
  echo "FAIL: /healthz returned '$HEALTH', expected 'ok'" >&2
  cat /tmp/notez-web.log >&2 || true
  exit 1
fi
echo "  /healthz = ok"

ROOT_HTML=$(curl -fsS "$BASE/")
if ! grep -q "Notez" <<<"$ROOT_HTML"; then
  echo "FAIL: / did not contain 'Notez'" >&2
  echo "$ROOT_HTML" >&2
  exit 1
fi
echo "  / contains 'Notez'"

SPACE_NAME="$(basename "$SPACE_DIR")"
HUB_HTML=$(curl -fsS "$BASE/s/$SPACE_NAME/")
if ! grep -q "Web Acceptance Space" <<<"$HUB_HTML"; then
  echo "FAIL: /s/$SPACE_NAME/ did not contain the README title" >&2
  echo "$HUB_HTML" >&2
  exit 1
fi
echo "  /s/$SPACE_NAME/ contains the README title"

# /s/.../agenda also returns 200 (may be empty).
AG_STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/s/$SPACE_NAME/agenda")
if [[ "$AG_STATUS" != "200" ]]; then
  echo "FAIL: /s/$SPACE_NAME/agenda returned HTTP $AG_STATUS, expected 200" >&2
  exit 1
fi
echo "  /s/$SPACE_NAME/agenda = 200"

# Bundled static asset is served out of the box.
MERMAID_STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/static/js/preview/mermaid.mjs")
if [[ "$MERMAID_STATUS" != "200" ]]; then
  echo "FAIL: /static/js/preview/mermaid.mjs returned HTTP $MERMAID_STATUS, expected 200" >&2
  exit 1
fi
echo "  /static/js/preview/mermaid.mjs = 200"

echo "web acceptance: PASS"