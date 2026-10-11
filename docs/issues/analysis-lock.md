vynulldev/vynull NOT POSTED. Draft only.

Suggested title: `Analysis: let a track's beat grid and analysis be locked against re-analysis`
Suggested labels: `api`, `enhancement`

## Summary

After a DJ has corrected a track's beat grid by hand (BPM, downbeat, phase shift, an offset), nothing stops
the daemon or another client from replacing that work. I'd like a per-track **lock** flag: while it is set,
the daemon does not re-run analysis for the track and refuses calls that would rewrite its grid.

## Why

Today, a hand-corrected grid is only as safe as everyone who talks to the daemon:

- `POST /api/analysis/reanalyze/{id}` invalidates the cache and re-runs analysis. It takes no confirmation,
  so one stray click in any client (or script) throws the corrected grid away.
- `PUT /api/tracks/{id}/beats` with `{"clear": true}` also resets the override.
- I have not checked whether a relink (`PUT /api/tracks/{id}/path`) or a re-import also analyses the track
  again; if they do, they are further ways to lose a corrected grid.
- `GET /api/analysis/{id}` on a track that has not been analysed yet starts an analysis on demand
  (`api: on-demand analysis for track N` in the log). That is helpful for a new track, but not for one the
  DJ has already fixed.

A client can stop *itself* from doing these (the Rust client in `vynulldev/vynull-rs` now has a per-browser
lock button), but that protects nothing against a second browser, another machine, a script or the daemon
itself, and it is lost when browser storage is cleared. The only reliable place for the flag is the daemon.

## Proposal

A boolean per track, stored with the other per-track data (next to the beat grid override):

- `GET /api/tracks` and `GET /api/tracks/{id}`: add `"analysis_locked": true|false` (omitted when false is fine,
  like `color_id`).
- `PUT /api/tracks/{id}/lock` with `{"locked": true|false}` (or a field on `PUT /api/tracks/{id}/metadata`,
  whichever fits better). Reply `{"OK": true}`.
- While locked, answer these with `409 Conflict` and a plain-text reason:
  - `POST /api/analysis/reanalyze/{id}`
  - `POST /api/analysis/beatgrid/adjust` for that `track_id`
  - `PUT /api/tracks/{id}/beats` (any field)
- While locked, do not start an automatic or on-demand analysis for the track if an analysis already exists.
  If none exists, analyse once as today (so locking an unanalysed track does not leave it without a grid).
- Unlocking restores today's behavior. No migration is needed: tracks without the flag are unlocked.

Not part of this proposal: locking cues or metadata. Those are plain edits and are not overwritten by analysis.

## Questions for the maintainers

- Should a bulk re-analysis (if one exists or is added) skip locked tracks silently or report them?
- Is `409` the right status, or would you rather return `423 Locked`?
- Should a re-import from rekordbox, if it carries a grid, overwrite a locked grid? I would say no.

I can send a PR for the flag, the three guards and tests if you agree on the shape.
