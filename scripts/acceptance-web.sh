#!/usr/bin/env bash
# Acceptance test for the notez web surface (packages/web).
#
# Seeds a tiny space, scans it with the CLI, launches the web server,
# then verifies the server-rendered workspace end to end:
#
#   * GET  /                       renders the space picker (200)
#   * GET  /s/<encoded>            lists the seeded document title
#   * GET  /s/<encoded>/<locator>  renders the org body
#   * POST /save/<encoded>         saves an edit (revision guarded)
#   * GET  /s/<encoded>/<locator>  shows the saved text
#   * GET  /raw/<encoded>/<locator> serves the raw bytes
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

- one
- two
EOF

echo "Scanning space..."
"$NOTEZ_BIN" --space "$SPACE_DIR" scan --json > /dev/null

ENCODING="$(printf '%s' "$SPACE_DIR" | base64 -w0 | tr '+/' '-_' | tr -d '=')"
DOC_URL="$BASE/s/$ENCODING/docs/README.org"

echo "Launching web server on $PORT..."
IP="$BIND_IP" PORT="$PORT" NOTEZ_SPACE_ROOT="$SPACE_DIR" "$WEB_BIN" > /tmp/notez-web.log 2>&1 &
PID=$!

wait_http() {
    local url="$1"
    for _ in $(seq 1 120); do
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
if [[ "$ROOT_STATUS" != "200" && "$ROOT_STATUS" != "303" ]]; then
    echo "Error: / returned $ROOT_STATUS, expected 200 or 303"
    exit 1
fi
echo "  / = $ROOT_STATUS"

SPACE_HTML=$(curl -fsS "$BASE/s/$ENCODING")
if ! grep -q "Web Acceptance Space" <<<"$SPACE_HTML"; then
    echo "Error: /s/<encoded> does not list the seeded title"
    exit 1
fi
echo "  /s/<encoded> lists the seeded title"

DOC_HTML=$(curl -fsS "$DOC_URL")
if ! grep -q "hello world" <<<"$DOC_HTML"; then
    echo "Error: document view does not render the org body"
    exit 1
fi
if ! grep -q "<li>one</li>" <<<"$DOC_HTML"; then
    echo "Error: document view does not render the org list"
    exit 1
fi
echo "  document view renders body + list"

echo "Saving an edit through the form endpoint..."
REVISION=$(sha256sum < "$SPACE_DIR/docs/README.org" | cut -d' ' -f1)
curl -fsS -o /dev/null -X POST \
    --data-urlencode "locator=docs/README.org" \
    --data-urlencode "revision=$REVISION" \
    --data-urlencode "content=#+TITLE: Web Acceptance Space
* edited heading
" \
    "$BASE/save/$ENCODING"

if ! grep -q "edited heading" "$SPACE_DIR/docs/README.org"; then
    echo "Error: save did not write the new content to disk"
    exit 1
fi
echo "  save wrote the new content"

if ! curl -fsS "$DOC_URL" | grep -q "edited heading"; then
    echo "Error: saved content is not visible in the document view"
    exit 1
fi
echo "  saved content is visible"

RAW=$(curl -fsS "$BASE/raw/$ENCODING/docs/README.org")
if ! grep -q "edited heading" <<<"$RAW"; then
    echo "Error: raw endpoint did not return the file bytes"
    exit 1
fi
echo "  raw endpoint serves the bytes"

echo "Rejecting a stale revision..."
STALE_HEADERS=$(curl -s -D - -o /dev/null -X POST \
    --data-urlencode "locator=docs/README.org" \
    --data-urlencode "revision=$REVISION" \
    --data-urlencode "content=stale write" \
    "$BASE/save/$ENCODING")
STALE_STATUS=$(head -1 <<<"$STALE_HEADERS" | awk '{print $2}')
if [[ "$STALE_STATUS" != "303" ]]; then
    echo "Error: stale save returned $STALE_STATUS, expected 303"
    exit 1
fi
if grep -q "stale write" "$SPACE_DIR/docs/README.org"; then
    echo "Error: stale save overwrote the file"
    exit 1
fi
RESTORE_URL=$(grep -i '^location:' <<<"$STALE_HEADERS" | tr -d '\r' | awk '{print $2}')
RESTORE_HTML=$(curl -fsS "$BASE$RESTORE_URL")
if ! grep -q "stale write" <<<"$RESTORE_HTML"; then
    echo "Error: failed save did not restore the user's text"
    exit 1
fi
if ! grep -q "banner warn" <<<"$RESTORE_HTML"; then
    echo "Error: failed save did not surface the conflict banner"
    exit 1
fi
echo "  stale save rejected, text restored, banner shown"

echo "web acceptance: PASS"
