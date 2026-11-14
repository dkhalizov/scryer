#!/bin/sh
# The parity net in one command: validate the catalogue, run every suite the
# catalogue names (one focused nextest run per crate), write
# parity/report/latest.md. Pass --baseline to freeze the result as baseline.
set -u
cd "$(dirname "$0")/.."
rm -rf parity/report/results
report="cargo run --locked -q -p scryer-parity --bin parity-report --"
status=0
cargo nextest run --locked -p scryer-parity || status=1
$report targets | awk '{ t[$1] = t[$1] " --test " $2 } END { for (c in t) print c t[c] }' |
  while read -r crate tests; do
    # shellcheck disable=SC2086
    cargo nextest run --locked --no-fail-fast -p "$crate" $tests || echo "suite failures in $crate" >&2
  done
$report "$@" >/dev/null || status=1
echo "report: parity/report/latest.md"
exit $status
