#!/usr/bin/env bash
# Generate the fixture music library: synthetic audio only, nothing copyrighted
# and no real file paths. Needs ffmpeg. Usage: ci/make-fixtures.sh <dir>
set -euo pipefail
dir=${1:?usage: make-fixtures.sh <dir>}
mkdir -p "$dir"
ff() { ffmpeg -loglevel error -y "$@"; }

# A kick on every beat plus an off-beat tick: 120 BPM, 24 s.
click() { # bpm
  local p; p=$(python3 -c "print(60/$1)")
  echo "0.8*sin(2*PI*70*t)*exp(-14*mod(t,$p))+0.3*sin(2*PI*2000*t)*exp(-60*mod(t+$p/2,$p))"
}

ff -f lavfi -i "aevalsrc='$(click 120)':s=44100:d=24" -ac 2 "$dir/fixture-120bpm.wav"
ff -f lavfi -i "aevalsrc='$(click 128)':s=44100:d=24" -c:a libmp3lame -b:a 128k \
  -metadata title="Fixture 128" -metadata artist="CI Fixture" -metadata album="Fixtures" \
  "$dir/fixture-128bpm.mp3"
# AIFF is not browser-playable, so the daemon serves it transcoded to WAV.
ff -f lavfi -i "sine=f=330:d=10" -ac 2 -f aiff "$dir/fixture-tone.aiff"
# Accented tags for the accent-insensitive search tests.
ff -f lavfi -i "sine=f=440:d=10" -metadata title="Café Crème" -metadata artist="Élève" \
  "$dir/fixture-tone.flac"
