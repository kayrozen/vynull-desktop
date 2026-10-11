#!/usr/bin/env bash
# Run the browser tests against a fixture daemon and the built web bundle.
#
#   VYNULL_BIN   built vynull binary (required)
#   BUNDLE       dir with the built client (default target/dx/vynull-web/release/web/public)
#   FIREFOX      browser (default /usr/bin/firefox)
#   ISOLATE=1    run inside a private network namespace (`unshare -rn`), for a
#                machine where a real daemon already holds the Pro DJ Link ports.
#
# `drawer.mjs` uses the fixture daemon; `library.mjs` and `players.mjs` use
# `fake-daemon.py` (scripted decks, a synthetic library). BROWSER=chrome with
# FIREFOX=/path/to/chrome runs them in Chrome.
#
# Needs ffmpeg, python3, node and npm. First run: `npm install` in ui-tests/.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ ${ISOLATE:-} == 1 && -z ${_IN_NETNS:-} ]]; then
  exec unshare -rn env _IN_NETNS=1 bash -c 'ip link set lo up && exec "$0" "$@"' "$0" "$@"
fi

bin=$(realpath "${VYNULL_BIN:?set VYNULL_BIN to a built vynull binary}")
bundle=$(realpath "${BUNDLE:-target/dx/vynull-web/release/web/public}")
api=${API_PORT:-19443}
app=${APP_PORT:-18080}
fake=${FAKE_PORT:-18444}
work=$(mktemp -d)
trap 'kill ${daemon:-} ${web:-} ${fakepid:-} 2>/dev/null || true' EXIT

ci/make-fixtures.sh "$work/music"
mkdir "$work/data"
"$bin" --interface lo --tui=false --mpris=false --web --listen "127.0.0.1:$api" \
  --data-dir "$work/data" --music-dir "$work/music" >"$work/daemon.log" 2>&1 &
daemon=$!
python3 -m http.server "$app" --bind 127.0.0.1 --directory "$bundle" >"$work/http.log" 2>&1 &
web=$!
python3 ui-tests/fake-daemon.py "$fake" >"$work/fake.log" 2>&1 &
fakepid=$!

for _ in $(seq 120); do
  curl -s -m1 "localhost:$api/api/diag/status" | grep -q Ready && break
  kill -0 "$daemon" 2>/dev/null || { tail -30 "$work/daemon.log" >&2; echo "daemon exited" >&2; exit 1; }
  sleep 0.5
done

cd ui-tests
[[ -d node_modules ]] || npm install --silent
status=0
for t in ${TESTS:-drawer.mjs library.mjs players.mjs screens.mjs}; do
  echo "== $t"
  APP="http://127.0.0.1:$app" API="http://127.0.0.1:$api" FAKE="http://127.0.0.1:$fake" FIREFOX="${FIREFOX:-/usr/bin/firefox}" node "$t" || status=1
done
exit $status
