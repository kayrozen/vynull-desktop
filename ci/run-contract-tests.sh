#!/usr/bin/env bash
# Start a Vynull daemon on a generated fixture library and run the contract
# tests (the `#[ignore]` tests) against it.
#
#   VYNULL_BIN   path to a built vynull binary (required)
#   PORT         listen port on 127.0.0.1 (default 19443)
#   ISOLATE=1    run inside a private network namespace (`unshare -rn`), for a
#                machine where a real daemon already holds the Pro DJ Link ports
#                (UDP 50000, NFS 2049, ...). Not needed on a CI runner.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ ${ISOLATE:-} == 1 && -z ${_IN_NETNS:-} ]]; then
  exec unshare -rn env _IN_NETNS=1 bash -c 'ip link set lo up && exec "$0" "$@"' "$0" "$@"
fi

bin=$(realpath "${VYNULL_BIN:?set VYNULL_BIN to a built vynull binary}")
port=${PORT:-19443}
work=$(mktemp -d)
log=$work/daemon.log
trap 'kill "${pid:-}" 2>/dev/null || true; rm -rf "$work"' EXIT

ci/make-fixtures.sh "$work/music"
mkdir "$work/data"
"$bin" --interface lo --tui=false --mpris=false --web \
  --listen "127.0.0.1:$port" --data-dir "$work/data" --music-dir "$work/music" >"$log" 2>&1 &
pid=$!

fail() { echo "$1" >&2; echo "---- daemon log ----" >&2; tail -40 "$log" >&2; exit 1; }
for _ in $(seq 60); do
  curl -sf -m1 "localhost:$port/api/status" >/dev/null && break
  kill -0 "$pid" 2>/dev/null || fail "daemon exited during startup"
  sleep 0.5
done
for _ in $(seq 120); do
  curl -s -m1 "localhost:$port/api/diag/status" | grep -q Ready && break
  sleep 0.5
done
curl -s -m1 "localhost:$port/api/diag/status" | grep -q Ready || fail "analysis never became ready"

# The /app route and cargo run from the daemon's own filesystem view; only the
# URL matters to the tests.
VYNULL_URL="http://127.0.0.1:$port" VYNULL_FIXTURE=1 cargo test --workspace -- --ignored --test-threads=4 \
  || fail "contract tests failed"
