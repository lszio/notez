#!/usr/bin/env bash
set -euo pipefail

notez_bin=${NOTEZ_BIN:-target/debug/notez}
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
mkdir -p "$workdir"
printf '# Before\n' > "$workdir/note.md"
"$notez_bin" --space "$workdir" scan >/dev/null

revision=$("$notez_bin" --space "$workdir" --json query --kind document --limit 100 | jq -r '.[] | select(.locator == "note.md") | .revision')
ref=$("$notez_bin" --space "$workdir" --json query --kind document --limit 100 | jq -r '.[] | select(.locator == "note.md") | .ref')
object_id=$("$notez_bin" --space "$workdir" --json query --kind document --limit 100 | jq -r '.[] | select(.locator == "note.md") | .object_id')

cat > "$workdir/update.json" <<EOF
{"ref":"$ref","kind":"document","title":"Note","revision":"writer-1","source_id":"native","locator":"note.md","properties":{},"object_id":"$object_id"}
EOF

"$notez_bin" --space "$workdir" resource upsert --from "$workdir/update.json" --expected-revision "$revision" >/dev/null

set +e
"$notez_bin" --space "$workdir" resource upsert --from "$workdir/update.json" --expected-revision "$revision" >"$workdir/stale.out" 2>"$workdir/stale.err"
status=$?
set -e
[ "$status" -eq 9 ]
grep -q 'revision conflict' "$workdir/stale.err"
printf 'acceptance-revision: PASS\n'
