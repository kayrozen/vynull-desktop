# Vynull Rust client: development plan

Oct 11, 2026 · @Francis

Build one Rust workspace that makes the web client (WebAssembly) and the desktop clients from the same code.

## API facts that shape the design

The Vynull 0.6.0 API is enough for a full client. A client can build every mockup screen from it.

- The API has no authentication. Vynull binds to `127.0.0.1:9443` by default. A remote client needs the `--listen` option.
- The server sends CORS headers for any origin on every response. A WebAssembly client on another origin can call it.
- `GET /api/events` is a Server-Sent Events stream. It sends the deck and now-playing state about 4 times each second.
- The field `beat_age_ms` gives the sub-beat phase. The client moves the playhead between two events.
- `GET /api/tracks` returns the whole library in one array. It supports ETag. Each track has `comment`, `tags` and `color_id`.
- Because of this, the client searches all fields on its own side, like the mockup does.
- Waveform data comes in three forms. These are JSON entries (spectral or 3-band), raw PWAV bytes, and a ready PNG.
- `GET /api/audio/{id}` streams a track with Range support. Vynull converts AIFF and M4A to WAV with ffmpeg.

The client must store some data itself, because the API has no field for it. This data is the search history, the column layout, the color history and the waveform colors.

The API does not give the mixer fader levels or the crossfader position. The `mixer` object only has `master_device`, `master_bpm` and `channel_on_air`. The Players mockup must show on-air state only. Do not draw faders there.

The API creates, edits and deletes tags and tag categories. Tag screens need no workaround.

## Architecture

&#91;embedded content: client architecture · 6 crates, 3 targets\]

The workspace has six crates. The UI and the core have no platform code. The platform layer holds the web and native backends. This design lets the same screens run in the browser and on the desktop.

## Stack choices

Use Dioxus for the UI. It builds web, desktop and mobile from one codebase. The mockup is HTML and CSS, so the layout moves across with little change. Run a one-week spike in phase 0 to confirm this choice.

| Area | Choice | Reason | Fallback |
| --- | --- | --- | --- |
| UI | Dioxus (WebAssembly in the browser, system webview on desktop) | Reuses the mockup CSS. One layout for all targets. | egui with eframe, if the list or the waveform is too slow |
| HTTP | reqwest | Same code on native and on wasm32 (fetch backend) | gloo-net behind the same trait |
| Live state | EventSource through web-sys on the web, eventsource-stream on native | One stream trait for both | Poll `/api/players` every 500 ms |
| Data types | serde structs written by hand from `openapi.yaml` | The spec has quirks, such as `OK` and `ok` ack keys | Generate with progenitor, then patch |
| Audio | HTMLAudioElement and WebAudio on the web, symphonia and cpal on native | Vynull sends WAV, MP3, FLAC or OGG, which these decode | Turn off playback on mobile in version 1 |
| Waveform | Draw the zoom window once, then move it with a CSS transform | The client does not rebuild paths at every frame | Use the PNG endpoint for thumbnails |
| Storage | One `Store` trait: localStorage on the web, a config file on native | The API cannot hold client data | None needed |
| Build | Cargo workspace, `dx` for web and desktop bundles | One command for each target | trunk for the web build |
| Tests | Unit tests in core, contract tests against a real Vynull 0.6.0 daemon, wasm-bindgen-test | Catches API drift early | None needed |

## Screen to API map

The API covers every mockup screen. Only the Players mixer strip needs a design change.

| Screen | Endpoints | Data kept in the client | Note |
| --- | --- | --- | --- |
| Players | `GET /api/events` (SSE), `/api/players`, `/api/nowplaying`, `/api/peers`, `/api/history` | None | Show on-air state only. The playhead moves with `beat_age_ms`. |
| Library list | `GET /api/tracks` (ETag), `/api/tracks/rev`, `/api/playlists`, `/api/playlists/{id}/tracks`, `/api/tags` | Search history, column order, column width, hidden columns | Search runs in the client. |
| Drawer waveform | `GET /api/analysis/waveform/{id}?type=detail&style=3band`, `/api/cues`, `/api/tracks/{id}/cues`, `/api/tracks/{id}/beats` | Waveform colors | Pre-draw the zoom window. |
| Drawer edits | `PUT /api/tracks/{id}/metadata`, `/color`, `/beats`. `POST /api/analysis/beatgrid/adjust`, `/api/analysis/reanalyze/{id}`, `/api/tracks/{id}/cues`, `/api/tracks/{id}/tags` | Color history | The downbeat keeps its place when the BPM changes. The client sends `set_downbeat_ms`. |
| Playback | `GET /api/audio/{id}` with Range | Volume, metronome state | Read `X-Audio-Source` for the DIRECT or TRANSCODED label. |
| Import and export | `POST /api/import/rekordbox`, `GET /api/import/status`, `GET /api/export/preview`, `POST /api/export`, `GET /api/fs/list` | None | Poll the status endpoint. |
| Tags | `GET`, `POST`, `PUT`, `DELETE` on `/api/tags` and `/api/tag-categories` | None | Full create, edit and delete. |
| Settings | `GET` and `POST /api/settings`, `/api/settings/enums`, `/api/menu-items`, `/api/library/remap-paths` | Theme, row density, waveform colors | DJMMYSETTING and DEVSETTING are read only. |
| Overlay | `GET` and `PUT /api/overlay/config` | None | The daemon serves `/overlay` itself. The client only edits the config and shows a preview. |
| Diag | `GET /api/diag`, `/api/diag/logs?since=`, `/api/diag/status`, `/api/link`, `POST /api/unlink` | Log filter | Poll every 2 seconds. |

## Repository and install with Vynull

The client lives in its own repository and has its own release cycle. A user can install it with Vynull at the same time, or use it alone.

1. The client repository builds a static web bundle and desktop binaries for each release.
2. Vynull serves the web bundle from a directory at `/app`. Add one flag, for example `--app-dir`. This is a small change in the Vynull repository.
3. Vynull does not embed the bundle. The two projects stay separate.
4. The release installers put the daemon and the client together. Linux packages ship first. Windows, macOS and a Docker Compose file follow. The installer sets the `--app-dir` path.
5. The desktop client also works alone. The user types the host and the port of any Vynull daemon on the network.
6. Each client release lists the Vynull versions it supports. A CI job runs the contract tests against each listed version.

## Phases

The plan takes about 16 weeks for one developer. This is an estimate. Each phase ends with a test that must pass before the next phase starts.

| Phase | Scope | Exit test | Weeks |
| --- | --- | --- | --- |
| 0. Foundations | Cargo workspace. CI for Linux (x86\_64 and aarch64), macOS, Windows and wasm32. A Vynull 0.6.0 test daemon with a fixture library. A UI spike: a 4,000-row list and one moving waveform. | The spike runs at 30 frames per second or more in Chrome and on one low-power ARM board. If not, switch to egui. | 1 |
| 1. API client and core | Typed client for all endpoints in the map. SSE stream. The `Store` trait. Search, suggestions and recent-search ranking. Beat-grid math and tap tempo. | Contract tests pass against the 0.6.0 daemon. Core tests match the mockup: accent-insensitive search, AND between words, downbeat kept when the BPM changes. | 2 |
| 2. Shell and Players | Theme tokens from the mockup. Tabs. A connection screen for host and port. Live deck cards. | Four decks update live. The client reconnects after a daemon restart. | 2 |
| 3. Library and drawer | Virtual list. Column reorder, hide and resize. Search with history. Playlists. Row tint by track color. Zoom waveform, BPM, grid, cues, color and metadata edits. | An edit of BPM, downbeat, color and one cue shows the same values in the Vynull web UI. | 4 |
| 4. Playback | The audio trait. Web and native backends. Metronome. Cue jump. Volume. | Play, seek and cue jump work on web and desktop. The client shows the length difference (Δlen). | 2 |
| 5. Tags, Settings, Diag, Overlay | The four remaining screens. Waveform color settings. | Every mockup screen works against 0.6.0. | 2 |
| 6. Release | Web bundle served by the daemon from `--app-dir`. Linux packages that install Vynull and the client together. Desktop binaries. README and release notes. | A user installs Vynull and the client with one installer on Linux. The web client loads from the daemon at `/app`. The desktop client connects to a remote daemon. | 3 |
| 7. Later | iOS and Android builds. A phone remote layout. | Not planned yet. | None |

## Risks

| Risk | Effect | Fix |
| --- | --- | --- |
| A web page served over HTTPS cannot call the daemon over HTTP. Chrome also asks permission for requests to a local network. | The hosted web client cannot reach a booth daemon. | Serve the wasm bundle from the daemon itself, at the same origin. This needs a small change in Vynull. |
| The library is one large JSON array (about 4,000 tracks in the mockup). | A slow first load and a slow parse in wasm. | Use ETag and `/api/tracks/rev`. Keep the parsed library in memory. Render only the visible rows. |
| The browser and the analysis can decode a file with different lengths. | The playhead drifts from the beat grid. | Show the length difference. Offer the WAV option. Both are in the mockup. |
| The Dioxus desktop target depends on the system webview. | Missing or old webview on some Linux systems. | Document the dependency. Keep egui as the fallback decided in phase 0. |
| The wasm bundle is large. | Slow first load on a weak network. | Optimize with `wasm-opt`. Compress the bundle. Load fonts once. |
| The API has no authentication. | Anyone on the network can edit the library. | Keep the client LAN-only. Never store secrets. Show a warning when the host is not private. |
| The API changes after 0.6.0. | The client breaks on a newer daemon. | The status endpoint has no version field. Add one to Vynull. Until then, run contract tests in CI for each release. |

## Decisions

1. Decided: keep Dioxus. The phase 0 spike ran at 60 fps on desktop Firefox and an iPhone 11. It missed 30 fps for scrolling on a throttled core (a stand-in for a low-power board), so the scroll path gets tuned as the UI is built.
2. Decided: the client license is MIT.
3. Decided: ship Linux packages first, with Vynull and the client together.
4. Decided: Vynull gets the `--app-dir` flag to serve the web client at `/app`.
5. Decided: the web client ships first. The desktop client follows.
6. Decided: the Players screen shows the on-air state only, with no faders.

## Sources

- [Vynull HTTP API specification, version 0.6.0](https://github.com/vynulldev/vynull/blob/v0.6.0/docs/api/openapi.yaml). All API facts come from this file.
- The Vynull web UI mockup, in this workspace. It sets the screens and the behavior.
