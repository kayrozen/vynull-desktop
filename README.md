# vynull-rs

A client for the [Vynull](https://github.com/vynulldev/vynull) DJ library daemon, written in Rust. One code base
gives a **web client** (WebAssembly) and a **desktop client** (a native window with its own audio output).
It talks to the daemon's HTTP API (version **0.6.0**) and needs a running daemon; it does not replace it.

> **Status: 0.1.2, an early release.** Everything below works against a scripted fake daemon and a fixture daemon
> in the test suite. The desktop client has been tried by hand on Windows and Linux. It has had little time on a
> real library and real decks, and none on macOS, Safari or iPhone. See [Limits](#limits) before relying on it on stage.

## What it does

Players, Library and Tags are tabs; Settings (which also holds Diag and Overlay) is the gear at the top right:

- **Players** shows every deck on the Pro DJ Link network: track, tempo, key, beat and bar, master and sync,
  on-air state, and an overview waveform with cues and song phrases (a microphone marks sung ones). A
  **waveform stack** puts one scrolling waveform per loaded deck on a shared centre line, and adds and removes
  rows as tracks load and unload. Decks that play from their own USB or SD card are shown too.
- **Library** lists the whole library (thousands of tracks) with instant search, tag filters, playlists, smart
  playlists, a queue, today's history, configurable columns and click-to-sort on two keys. Key chips are tinted
  against the master deck. Playlists can be reordered by dragging. Import and export panels drive the daemon.
- **Track drawer** (click a row): zoom waveform with the real beat grid, bar numbers, cues and phrases; overview;
  playback in the browser or the desktop app with a metronome; tempo, tap, phase and grid shift; set the
  downbeat; hot and memory cues (add, drag, delete, loops); colour, tags and every metadata field.
- **Tags**, and **Settings** (the gear): web look with theme, density and an accent color (six presets or any
  color from a wheel), library source, the streaming **overlay** configuration, **Diag** (counters and a live log),
  the player menu, and the daemon's own player, mixer and waveform settings.

Waveforms follow the daemon's own colour setting (Pioneer RGB, blue or three-band) and its overview type.

## Install

Download a package from the [releases page](../../releases) of this repository.

| Platform | Packages |
|---|---|
| Linux x86_64 | `.deb`, `.rpm`, `.AppImage`, Flatpak bundle |
| Linux aarch64 | `.deb`, `.rpm`, Flatpak bundle |
| macOS (Apple silicon) | `.dmg` |
| Windows x86_64 | `.msi`, `-setup.exe` |
| Any browser | `vynull-web-0.1.2.zip` (static files) |

`SHA256SUMS` lists every file. **None of the packages is signed or notarised.** macOS will refuse to open the
app until you allow it in *System Settings > Privacy & Security*, and Windows SmartScreen will warn.

Linux packages need WebKitGTK 4.1, GTK 3 and ALSA (the `.deb` and `.rpm` declare them; the AppImage and Flatpak
carry what they need).

```sh
sudo apt install ./vynull-desktop_0.1.2_amd64.deb      # Debian, Ubuntu
sudo dnf install ./vynull-desktop-0.1.2-1.x86_64.rpm   # Fedora
chmod +x vynull-desktop_0.1.2_x86_64.AppImage && ./vynull-desktop_0.1.2_x86_64.AppImage
flatpak install --user ./vynull-rs-0.1.2-x86_64.flatpak
```

If the window opens blank (seen with some NVIDIA setups, with a `Failed to create GBM buffer` message), start it with
`WEBKIT_DISABLE_DMABUF_RENDERER=1`. The Flatpak sets that already.

## Run it

1. Start the Vynull daemon. By default it listens on `127.0.0.1:9443`. To reach it from another machine it has to
   be started with `--listen`; read the security note below first.
2. Start the desktop app, or open the web client. On the connection screen type the daemon's `host:port`. The
   address is remembered.

The **web client** is a folder of static files. Until the daemon can serve it itself (a planned `--app-dir`
option), serve it with any static file server and open that address:

```sh
unzip vynull-web-0.1.2.zip -d vynull-web
cd vynull-web && python3 -m http.server 8080 --bind 127.0.0.1
```

A page served over HTTPS cannot call the daemon's plain-HTTP API, so serve it over HTTP. Cross-origin, a browser
cannot read the response headers the daemon does not expose, so the client detects library changes by polling
instead; everything else works.

### Security

The daemon has **no authentication**, and so does this client. Use it on a trusted LAN only, keep the daemon on
`127.0.0.1` unless you need otherwise, and do not expose either to the internet. The connection screen warns when
the address is not a private one. The client stores no secrets.

## Build from source

You need Rust (see `rust-toolchain.toml`) and, for the web client, the `wasm32-unknown-unknown` target and
[`dx`](https://dioxuslabs.com) 0.7.10 (`cargo install dioxus-cli --version 0.7.10 --locked`, which needs OpenSSL
development headers).

```sh
# desktop client (Linux also needs libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libasound2-dev)
cargo run --release -p vynull-desktop

# web client, output in target/dx/vynull-web/release/web/public
cd crates/vynull-web && dx build --release --platform web

# installers for this machine, from crates/vynull-desktop
dx bundle --release --package-types deb      # or rpm, appimage, dmg, msi, nsis
```

Tests:

```sh
cargo test --workspace                                   # unit tests
VYNULL_BIN=/path/to/vynull ci/run-contract-tests.sh      # API contract tests against a fixture daemon
VYNULL_BIN=/path/to/vynull ISOLATE=1 ui-tests/run.sh     # browser tests (Firefox or Chrome), see the script
```

`CLAUDE.md` describes the code in detail, including what was verified and what was not.

## Layout

| Crate | Role |
|---|---|
| `vynull-api` | Typed client for the HTTP API (serde types, `reqwest`) |
| `vynull-core` | Pure logic: search, columns, sorting, beat grid and waveform maths, key colours, queue, settings |
| `vynull-platform` | `Store`, audio and event-stream traits with web and native backends (`symphonia` + `cpal` natively) |
| `vynull-ui` | The screens, written once with Dioxus |
| `vynull-web` | The WebAssembly entry point |
| `vynull-desktop` | The desktop entry point |

## Limits

- **Built against Vynull 0.6.0.** The status endpoint has no version field, so a different daemon version may
  behave differently; the contract tests are the drift check.
- **macOS, Safari and iPhone are untested.** The desktop client has been tried by hand on Windows and Linux (which
  packages, and for how long, was not recorded). The macOS `.dmg` is built by CI and has never been opened.
  Firefox and Chrome are tested headless.
- **Audio:** playback in the desktop app decodes the whole file into memory (about 60 MB for six minutes) and has
  not been listened to for quality. Browser playback uses the browser's decoder, which can differ from the
  daemon's analysis by a few milliseconds; the daemon only reports the analysed length in whole seconds.
- **Read-only where the API is:** there are no mixer faders or crossfader, and the client shows on-air state only.
  There is no per-track "analysing" flag.
- **The grid lock is client-side.** The LOCK button in the drawer stops *this client* editing or re-analysing a
  track; the daemon has no such lock, so another client can still do it.
- Some writes (library import and export, settings) have only been exercised against test daemons, never a real
  library of someone else's. Keep a backup of anything you care about.

## Licence

MIT, see `LICENSE`. This project is independent of the Vynull daemon (GPLv3) and contains none of its code. The
bundled fonts (Roboto, Cousine) are Apache-2.0; see `crates/vynull-ui/assets/fonts/README.md`.
