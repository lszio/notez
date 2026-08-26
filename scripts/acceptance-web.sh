#!/usr/bin/env bash
# Acceptance test for the Dioxus fullstack web server (packages/web).
#
# Seeds a tiny space, scans it with the CLI, launches the SSR web
# server binary in the background, then verifies:
#
#   * /                              renders 200 (space picker)
#   * /source/<encoded>/list         renders the seeded resource title
#   * POST /api/sources/watch/start  starts watching (form contract)
#   * GET  /api/sources/watch/state  reports watching=true
#   * POST /api/sources/watch/stop   accepts `source_root` form field
#
# Finally tears the server down.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

echo "Building notez CLI and web server..."
cargo build -p cli --bin notez
cargo build -p web --bin web

NOTEZ_BIN="$PROJECT_ROOT/target/debug/notez"
WEB_BIN="$PROJECT_ROOT/target/debug/web"

BIND_IP="127.0.0.1"
PORT="3939"
BASE="http://$BIND_IP:$PORT"

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

mkdir -p "$SPACE_DIR/docs"
cat > "$SPACE_DIR/docs/README.org" <<EOF
#+TITLE: Web Acceptance Space
* hello world
:PROPERTIES:
:ID: 01J000000000000000000000AA
:END:
EOF

echo "Scanning space..."
"$NOTEZ_BIN" --space "$SPACE_DIR" scan --json > /dev/null

ENCODING="$(printf '%s' "$SPACE_DIR" | base64 -w0 | tr '+/' '-_' | tr -d '=')"

echo "Launching web server on $PORT..."
IP="$BIND_IP" PORT="$PORT" "$WEB_BIN" > /tmp/notez-web.log 2>&1 &
PID=$!

wait_http() {
    local url="$1"
    for _ in $(seq 1 60); do
        if curl -fsS -o /dev/null "$url" 2>/dev/null; then
            return 0
        fi
        if ! kill -0 "$PID" 2>/dev/null; then
            echo "Error: web server exited early; log follows."
            tail -40 /tmp/notez-web.log || true
            exit 1
        fi
        sleep 0.5
    done
    echo "Error: timed out waiting for $url"
    tail -40 /tmp/notez-web.log || true
    exit 1
}

echo "Waiting for server root..."
wait_http "$BASE/"

ROOT_STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/")
if [[ "$ROOT_STATUS" != "200" ]]; then
    echo "Error: / returned $ROOT_STATUS, expected 200"
    exit 1
fi
echo "  / = 200"

LIST_HTML=$(curl -fsS "$BASE/source/$ENCODING/list")
if ! grep -q "Web Acceptance Space" <<<"$LIST_HTML"; then
    echo "Error: /source/<encoded>/list does not contain the seeded title"
    exit 1
fi
echo "  /source/<encoded>/list contains the seeded title"

echo "Starting watch via form endpoint..."
STOP_FORM="$SPACE_DIR"
curl -fsS -o /dev/null -X POST \
    --data-urlencode "source_root=$STOP_FORM" \
    "$BASE/api/sources/watch/start"

STATE_JSON=$(curl -fsS "$BASE/api/sources/watch/state?path=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$SPACE_DIR")")
if ! grep -q '"watching":true' <<<"$STATE_JSON"; then
    echo "Error: watch state did not report watching=true: $STATE_JSON"
    exit 1
fi
echo "  watch state reports watching=true"

echo "Stopping watch via form endpoint (source_root field)..."
curl -fsS -o /dev/null -X POST \
    --data-urlencode "source_root=$STOP_FORM" \
    "$BASE/api/sources/watch/stop"

STATE_JSON=$(curl -fsS "$BASE/api/sources/watch/state?path=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$SPACE_DIR")")
if grep -q '"watching":true' <<<"$STATE_JSON"; then
    echo "Error: watch still reported active after stop: $STATE_JSON"
    exit 1
fi
echo "  watch stopped cleanly"

echo "web acceptance: PASS"
