<!--
Draft for the vynulldev/vynull repository. NOT POSTED.
Suggested title and labels are in the first lines below; the body follows.
-->

**Title:** `GET /api/analysis/{id}`: add `duration_ms` (the length is truncated to whole seconds)

**Labels (suggestion):** `api`, `enhancement`

---

## Summary

`GET /api/analysis/{id}` reports the track length only as `duration_sec`, an integer number of seconds, and it is **truncated**, not rounded. A client cannot tell how the analysed length compares with the length a browser or another decoder reports, which is the one comparison that tells whether a beat grid will line up with playback.

I'd like a `duration_ms` field next to `duration_sec`.

## Why a client needs it

The analysis decodes with ffmpeg; the browser (or a native player) decodes the same file with its own decoder. The two can disagree by a few milliseconds up to tens of milliseconds (encoder delay and padding, for example in mp3 and AAC). When they do, the playhead drifts against the beat grid and cue points.

The `/api/audio/{id}` endpoint already supports forcing the ffmpeg WAV path (`?transcode=1`) so that playback uses the same decoder as the analysis, and that is useful. But a client that wants to *show* the difference (so a DJ can tell "the grid is off by 26 ms because of the decoder, not because the grid is wrong") needs the analysed length at millisecond precision.

Today the best a client can do is count the detail waveform entries (150 per second, so a step of 6.7 ms) and compare that with the player's `duration`. That works but is indirect, needs the whole waveform fetched, and is quantised.

## What the code does today (v0.6.0)

- `analysis/analysis.go`: the length is computed from the decoded samples as a float, then truncated:

  ```go
  durationSec := float64(len(samples)) / float64(AnalysisRate)   // AnalysisRate = 44100
  ...
  Duration: uint16(durationSec),
  ```

- `api/api.go` (`handleAnalysis`) copies that into `duration_sec` (`uint16`).
- `docs/api/openapi.yaml`: `duration_sec: { type: integer }`.

So a 24.9 s file reports `24`, and the information is lost before it leaves the analysis package. The precise value exists one line earlier.

Other parts of the API already have a millisecond duration: `PlayerInfo.duration_ms` and `GET /api/analysis/ext/{player}/{slot}/{trackID}` (`duration_ms`). The library analysis is the odd one out.

## Proposal

Add a field and keep the old one, so nothing existing breaks:

```jsonc
// GET /api/analysis/123
{
  "track_id": 123,
  "bpm": 124,
  "duration_sec": 24,        // unchanged (still truncated)
  "duration_ms": 24937,      // new: round(len(samples) * 1000 / AnalysisRate)
  ...
}
```

- Compute it from the sample count that is already available (`len(samples)`), at the analysis rate (44.1 kHz), so it is exact to about 0.02 ms.
- Add `duration_ms: { type: integer, format: int64 }` to `AnalysisResult` in `openapi.yaml`.
- Store it with the cached analysis (the cache is versioned, so bumping the cache version when the field is added would populate it for existing tracks on the next analysis; until then clients should treat a missing or zero value as "unknown").

A smaller variant that would also help: make `duration_sec` round instead of truncate. That changes existing values by up to a second, so I would not do it without a note in the changelog; the new field makes it unnecessary.

## What I'd do with it

I'm building a separate Rust client (web and desktop) against the 0.6.0 API. In its track drawer it shows, next to DIRECT or TRANSCODED, the difference between the length the player decoded and the analysed length (`Δlen +2ms`), and offers the WAV option when it is large. With `duration_ms` that number would be exact; without it, it is only accurate to 6.7 ms and needs the full waveform.

## Notes

- No change to existing clients: this only adds a field.
- Optional, related: `sample_rate` of the analysis (always 44100 today) in the same response would let a client reason about the precision. Not needed for this request.
- I'm happy to send a PR for the Go change, the OpenAPI entry and a test if that is welcome.
