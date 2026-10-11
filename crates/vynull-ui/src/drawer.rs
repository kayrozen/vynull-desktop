//! Track drawer: zoom waveform with the real beat grid, overview, playback,
//! BPM and grid editing, metadata, cues, color, tags and file.
//!
//! The grid is the daemon's: lines are drawn from `GET /api/analysis/{id}`
//! beats, and every grid edit goes to the daemon and is read back, so the
//! drawer never invents a grid of its own.

use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use vynull_api::{
    BeatInfo, BeatSummary, BeatgridAdjust, BeatsPatch, CueInfo, CueKind, PhraseInfo, TagInfo,
    TrackInfo, TrackMetadataPatch, WaveEntry3Band, WaveEntrySpectral,
};
use vynull_core::grid::{self, Grid, Zoom};
use vynull_core::palette;
use vynull_core::tap::TapTempo;
use vynull_core::waveform::{self, OverviewPaths};
use vynull_platform::{time, PlayStatus, StoreExt};

use crate::library_data::LibState;
use crate::phrases::PhraseStrip;
use crate::shell::{ClientCtx, LookCtx, Platform};

/// Props wrapper that compares by pointer, so a big shared value does not get
/// compared (or cloned) on every render.
#[derive(Debug)]
pub struct Shared<T>(pub Rc<T>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, PartialEq)]
pub struct DeckBtn {
    pub n: i64,
    pub cls: &'static str,
    pub title: String,
}

/// Formats the daemon decodes natively. Anything else is served as WAV, so its
/// length can differ from the analysis.
const NATIVE: [&str; 4] = ["mp3", "wav", "flac", "ogg"];

/// Waveform and overview of a track. Fetched once per track.
#[derive(Clone, PartialEq)]
struct Wave {
    /// Three-band detail data, for the band styles.
    entries: Shared<Vec<WaveEntry3Band>>,
    /// Spectral (Pioneer PWV5) detail data, for the RGB and blue styles.
    spectral: Shared<Vec<WaveEntrySpectral>>,
    overview: OverviewPaths,
}

/// Everything that a grid edit changes. Fetched again after each edit.
#[derive(Clone, PartialEq)]
struct GridData {
    beats: Shared<Vec<BeatInfo>>,
    summary: BeatSummary,
    /// The tempo of the grid as drawn. Read from the analysis: after a grid
    /// adjustment the summary endpoint still reports the old override.
    bpm: f64,
    duration_ms: i64,
}

const COLOR_HISTORY: &str = "color_history";
const VOLUME: &str = "volume";
const METRONOME: &str = "metronome";
const ZOOM_KEY: &str = "zoom_bars";

fn cue_color(c: &CueInfo) -> String {
    if let Some((_, hex)) = palette::color(c.color_id) {
        return hex.to_string();
    }
    match c.number {
        1 => "#FF6B6B",
        2 => "#FFC15A",
        3 => "#3FD6C0",
        4 => "#6FB1FF",
        5 => "#B07CFF",
        6 => "#FF8AD0",
        7 => "#5FD38D",
        8 => "#FF9A3C",
        _ => "#9AA4B2",
    }
    .to_string()
}

fn cue_name(c: &CueInfo) -> String {
    if !c.label.is_empty() {
        c.label.clone()
    } else if (1..=8).contains(&c.number) {
        format!("Cue {}", grid::cue_letter(c.number))
    } else {
        format!("Memory {}", c.number - 8)
    }
}

fn art_style(n: i64) -> String {
    let h = (n * 47 + 7) % 360;
    format!(
        "background:linear-gradient(135deg,hsl({h},42%,34%),hsl({},42%,16%))",
        (h + 50) % 360
    )
}

/// `1.5 MB`, `812 KB`.
fn bytes_label(n: i64) -> String {
    if n <= 0 {
        return "—".into();
    }
    let n = n as f64;
    if n >= 1e9 {
        format!("{:.2} GB", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.1} MB", n / 1e6)
    } else {
        format!("{:.0} KB", n / 1e3)
    }
}

/// The read-only facts about a track: what the file is and how the library has used it.
fn info_rows(t: &TrackInfo, bpm: f64) -> Vec<(&'static str, String)> {
    let dash = |s: String| {
        if s.trim().is_empty() {
            "—".to_string()
        } else {
            s
        }
    };
    vec![
        ("BPM", format!("{bpm:.2}")),
        (
            "DURATION",
            if t.duration > 0 {
                mmss(t.duration)
            } else {
                "—".into()
            },
        ),
        (
            "BITRATE",
            if t.bitrate > 0 {
                format!("{} kbps", t.bitrate)
            } else {
                "—".into()
            },
        ),
        ("TYPE", dash(t.file_type.to_uppercase())),
        ("SIZE", bytes_label(t.file_size)),
        (
            "TRACK NO.",
            if t.track_num > 0 {
                t.track_num.to_string()
            } else {
                "—".into()
            },
        ),
        (
            "RATING",
            if t.rating > 0 {
                format!("{} / 5", t.rating)
            } else {
                "—".into()
            },
        ),
        ("COLOR", dash(t.color_name.clone())),
        ("PLAY COUNT", t.play_count.to_string()),
        ("ADDED", dash(t.date_added.clone())),
    ]
}

fn mmss(secs: i64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[derive(Props, Clone, PartialEq)]
pub struct DrawerProps {
    pub track: TrackInfo,
    pub lib: Signal<LibState>,
    pub decks: Vec<DeckBtn>,
    pub on_close: EventHandler<()>,
    pub on_load: EventHandler<i64>,
    pub on_flash: EventHandler<String>,
}

#[component]
pub fn Drawer(props: DrawerProps) -> Element {
    let client = use_context::<ClientCtx>().0;
    let cue_colors = use_context::<LookCtx>().cue_colors;
    let locks = use_context::<LookCtx>().locks;
    let platform = use_context::<Platform>();
    let audio = platform.audio.clone();
    let track = props.track.clone();
    let id = track.id;

    let mut refresh = use_signal(|| 0u32);
    let mut cursor = use_signal(|| 0.0_f64);
    let mut last_click = use_signal(|| None::<usize>);
    let mut metro = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<bool>(METRONOME)
                .ok()
                .flatten()
                .unwrap_or(false)
        }
    });
    let mut wav = use_signal(|| false);
    let mut volume = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<u32>(VOLUME)
                .ok()
                .flatten()
                .unwrap_or(80)
                .min(100)
        }
    });
    let mut bars = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<u32>(ZOOM_KEY)
                .ok()
                .flatten()
                .filter(|b| grid::ZOOM_BARS.contains(b))
                .unwrap_or(grid::DEFAULT_BARS)
        }
    });
    let zoom_step = use_callback({
        let store = platform.store.clone();
        move |out: bool| {
            let next = grid::step_bars(*bars.peek(), out);
            if next != *bars.peek() {
                bars.set(next);
                let _ = store.set_json(ZOOM_KEY, &next);
            }
        }
    });
    let mut prev_volume = use_signal(|| 80u32);
    let mut astatus = use_signal(|| PlayStatus::Empty);
    let mut adur = use_signal(|| None::<f64>);
    let mut aerr = use_signal(|| None::<String>);
    let mut draft_bpm = use_signal(String::new);
    let mut taps = use_signal(TapTempo::new);
    let mut shift_ms = use_signal(|| 0_i64);
    // `(start, end)` in seconds while a loop cue plays.
    let mut active_loop = use_signal(|| None::<(f64, f64)>);
    let mut busy = use_signal(|| false);
    let mut colors_hist = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<Vec<i64>>(COLOR_HISTORY)
                .ok()
                .flatten()
                .unwrap_or_else(|| vec![2, 5, 7, 8])
        }
    });
    let mut tag_pick = use_signal(|| false);
    let mut relink = use_signal(String::new);
    let mut dh = use_signal(|| 0.0_f64); // drawer height in px, 0 = half
    let mut resizing = use_signal(|| None::<(f64, f64)>);

    // ---- data: the waveform once per track, the grid again after each edit
    let c = client.clone();
    let LookCtx {
        colors,
        daemon_wave,
        daemon_overview,
        ..
    } = use_context::<LookCtx>();
    let spectral_style = colors().spectral(&daemon_wave());
    let half = daemon_overview() == "half";
    let wave = use_resource(use_reactive!(|(id, spectral_style, half)| {
        let c = c.clone();
        async move {
            match spectral_style {
                Some(style) => {
                    let w = c
                        .detail_waveform::<WaveEntrySpectral>(id, "spectral")
                        .await
                        .ok()?;
                    Some(Wave {
                        overview: waveform::spectral_overview(&w.entries, style, half),
                        spectral: Shared(Rc::new(w.entries)),
                        entries: Shared(Rc::new(Vec::new())),
                    })
                }
                None => {
                    let w = c
                        .detail_waveform::<WaveEntry3Band>(id, "3band")
                        .await
                        .ok()?;
                    let ov = c.color_preview(id).await.ok()?;
                    Some(Wave {
                        entries: Shared(Rc::new(w.entries)),
                        spectral: Shared(Rc::new(Vec::new())),
                        overview: waveform::overview_paths(&ov.entries, half),
                    })
                }
            }
        }
    }));
    let c = client.clone();
    let gdata = use_resource(use_reactive!(|(id,)| {
        let _ = refresh();
        let c = c.clone();
        async move {
            let a = c.analysis(id).await.ok()?;
            let summary = c.track_beats(id).await.unwrap_or_default();
            Some(GridData {
                beats: Shared(Rc::new(a.beats)),
                summary,
                bpm: a.bpm,
                duration_ms: a.duration_sec * 1000,
            })
        }
    }));
    let c = client.clone();
    let cues_res = use_resource(use_reactive!(|(id,)| {
        let _ = refresh();
        let c = c.clone();
        async move { c.track_cues(id).await.unwrap_or_default() }
    }));
    let c = client.clone();
    let phrases_res = use_resource(use_reactive!(|(id,)| {
        let c = c.clone();
        async move { c.phrases(id).await.unwrap_or_default() }
    }));
    let c = client.clone();
    let track_tags = use_resource(use_reactive!(|(id,)| {
        let _ = refresh();
        let c = c.clone();
        async move { c.track_tags(id).await.unwrap_or_default() }
    }));

    // The draft BPM follows the saved one.
    let saved_bpm = gdata
        .read()
        .as_ref()
        .and_then(|g| g.as_ref())
        .map(|g| g.summary.bpm);
    use_effect(use_reactive!(|(saved_bpm,)| {
        if let Some(b) = saved_bpm {
            draft_bpm.set(format!("{b:.2}"));
        }
    }));

    // ---- audio: a new track reloads the player
    let audio_url = if wav() {
        client.audio_url_transcoded(id)
    } else {
        client.audio_url(id)
    };
    {
        let audio = audio.clone();
        use_effect(use_reactive!(|(audio_url,)| {
            let resume = cursor.peek().max(0.0);
            audio.load(&audio_url);
            audio.set_volume(*volume.peek() as f64 / 100.0);
            if resume > 0.0 {
                audio.seek(resume);
            }
            last_click.set(None);
        }));
    }
    // Reset the cursor when the track changes (not when only the WAV option does).
    use_effect(use_reactive!(|(id,)| {
        let _ = id;
        cursor.set(0.0);
        shift_ms.set(0);
        tag_pick.set(false);
    }));
    {
        let audio = audio.clone();
        use_drop(move || audio.pause());
    }

    // ---- one tick: playhead, status, and metronome look-ahead
    {
        let audio = audio.clone();
        use_future(move || {
            let audio = audio.clone();
            async move {
                loop {
                    // A loop is checked more often so the jump back lands close to the loop end.
                    time::sleep(Duration::from_millis(if active_loop.peek().is_some() {
                        15
                    } else {
                        50
                    }))
                    .await;
                    let st = audio.state();
                    if *astatus.peek() != st.status {
                        astatus.set(st.status);
                    }
                    if *adur.peek() != st.duration_secs {
                        adur.set(st.duration_secs);
                    }
                    if *aerr.peek() != st.error {
                        aerr.set(st.error.clone());
                    }
                    if st.status == PlayStatus::Playing {
                        let looped = *active_loop.peek();
                        if let Some((start, end)) = looped {
                            if st.position_secs >= end {
                                audio.seek(start);
                                cursor.set(start);
                                last_click.set(None);
                                continue;
                            }
                        }
                        cursor.set(st.position_secs);
                        if metro() {
                            if let Some(Some(g)) = gdata.peek().as_ref().map(|g| g.clone()) {
                                let mut last = *last_click.peek();
                                let clicks = grid::next_clicks(
                                    &g.beats.0,
                                    st.position_secs,
                                    0.25,
                                    &mut last,
                                );
                                if last != *last_click.peek() {
                                    last_click.set(last);
                                }
                                for (delay, accent) in clicks {
                                    audio.click_at(delay, accent);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    let seek = {
        let audio = audio.clone();
        use_callback(move |secs: f64| {
            let secs = secs.max(0.0);
            cursor.set(secs);
            last_click.set(None);
            active_loop.set(None);
            audio.seek(secs);
        })
    };
    let play_from = {
        let audio = audio.clone();
        use_callback(move |secs: f64| {
            let secs = secs.max(0.0);
            cursor.set(secs);
            last_click.set(None);
            audio.seek(secs);
            audio.play();
        })
    };

    // ---- keyboard: Space plays or pauses, [ and ] jump between cues, M toggles the
    // metronome. Keys typed in a field are left alone. The page-level listener is
    // JavaScript (through `eval`) so the same code works in a browser and a webview.
    {
        let audio = audio.clone();
        let store = platform.store.clone();
        use_future(move || {
            let audio = audio.clone();
            let store = store.clone();
            async move {
                let mut ev = document::eval(
                    r#"
                    if (window.__vynullKeys) window.removeEventListener('keydown', window.__vynullKeys);
                    window.__vynullKeys = (e) => {
                        const t = e.target, tag = t && t.tagName;
                        if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || t.isContentEditable) return;
                        if (e.metaKey || e.ctrlKey || e.altKey) return;
                        if ([' ', '[', ']', 'm', 'M'].includes(e.key)) {
                            e.preventDefault();
                            // A focused button would also fire a click on key release.
                            if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
                            dioxus.send(e.key);
                        }
                    };
                    window.addEventListener('keydown', window.__vynullKeys);
                    "#,
                );
                while let Ok(key) = ev.recv::<String>().await {
                    match key.as_str() {
                        " " => {
                            if *astatus.peek() == PlayStatus::Playing {
                                audio.pause();
                            } else {
                                audio.play();
                            }
                        }
                        "[" | "]" => {
                            let pos = *cursor.peek();
                            let cues = cues_now(&cues_res);
                            if let Some(t) =
                                grid::step_cue(&cues, pos, if key == "[" { -1 } else { 1 })
                            {
                                seek.call(t);
                            }
                        }
                        _ => {
                            metro.toggle();
                            last_click.set(None);
                            let _ = store.set_json(METRONOME, &*metro.peek());
                        }
                    }
                }
            }
        });
        use_drop(|| {
            document::eval("if (window.__vynullKeys) { window.removeEventListener('keydown', window.__vynullKeys); window.__vynullKeys = null; }");
        });
    }

    // ---- edits
    let c = client.clone();
    let grid_edit = use_callback(move |what: GridEdit| {
        if busy() {
            return;
        }
        if locks.peek().has(id) {
            props
                .on_flash
                .call("THE GRID IS LOCKED. UNLOCK IT TO EDIT OR ANALYZE AGAIN.".into());
            return;
        }
        busy.set(true);
        let c = c.clone();
        let beats = gdata
            .peek()
            .as_ref()
            .and_then(|g| g.as_ref())
            .map(|g| g.beats.clone());
        spawn(async move {
            let res = what.apply(&c, id, beats.as_ref().map(|b| &b.0[..])).await;
            match res {
                Ok(delta) => {
                    shift_ms.set(shift_ms() + delta);
                    refresh.set(refresh() + 1);
                }
                Err(e) => props.on_flash.call(format!("GRID EDIT FAILED: {e}")),
            }
            busy.set(false);
        });
    });

    let c = client.clone();
    let mut lib = props.lib;
    let set_color = use_callback({
        let store = platform.store.clone();
        move |color_id: i64| {
            let c = c.clone();
            let store = store.clone();
            spawn(async move {
                match c.set_track_color(id, color_id).await {
                    Ok(()) => {
                        let cur = lib.peek().clone();
                        if let Some(&i) = cur.by_id.get(&id) {
                            let mut t = cur.tracks[i].clone();
                            t.color_id = color_id;
                            lib.set(cur.with_track(t));
                        }
                        if color_id != 0 {
                            let mut h = colors_hist.peek().clone();
                            h.retain(|x| *x != color_id);
                            h.insert(0, color_id);
                            h.truncate(6);
                            let _ = store.set_json(COLOR_HISTORY, &h);
                            colors_hist.set(h);
                        }
                    }
                    Err(e) => props.on_flash.call(format!("COLOR FAILED: {e}")),
                }
            });
        }
    });

    let c = client.clone();
    let put_cue = use_callback(move |cue: CueInfo| {
        let c = c.clone();
        spawn(async move {
            match c.put_cue(id, &cue).await {
                Ok(()) => refresh.set(refresh() + 1),
                Err(e) => props.on_flash.call(format!("CUE FAILED: {e}")),
            }
        });
    });
    let c = client.clone();
    let del_cue = use_callback(move |number: i64| {
        let c = c.clone();
        spawn(async move {
            match c.delete_cue(id, number).await {
                Ok(()) => refresh.set(refresh() + 1),
                Err(e) => props.on_flash.call(format!("CUE FAILED: {e}")),
            }
        });
    });

    // Shift-click on the zoom view: a hot cue in the first free slot, in its default color.
    let add_cue_at = use_callback(move |secs: f64| {
        let existing = cues_now(&cues_res);
        match grid::free_hot_cue(&existing) {
            Some(n) => put_cue.call(CueInfo {
                number: n,
                kind: CueKind::Cue,
                time_ms: (secs * 1000.0).round() as i64,
                loop_ms: -1,
                color_id: cue_colors().for_cue(n),
                label: String::new(),
            }),
            None => props.on_flash.call("ALL 8 HOT CUES ARE USED".into()),
        }
    });
    // Dragging a flag: the same cue number at the new time; a loop keeps its length.
    let move_cue = use_callback(move |(number, secs): (i64, f64)| {
        let Some(old) = cues_now(&cues_res).into_iter().find(|c| c.number == number) else {
            return;
        };
        let time_ms = (secs * 1000.0).round() as i64;
        let loop_ms = if old.kind == CueKind::Loop && old.loop_ms > old.time_ms {
            time_ms + (old.loop_ms - old.time_ms)
        } else {
            old.loop_ms
        };
        put_cue.call(CueInfo {
            time_ms,
            loop_ms,
            ..old
        });
    });

    let c = client.clone();
    let set_tags = use_callback(move |ids: Vec<i64>| {
        let c = c.clone();
        spawn(async move {
            match c.set_track_tags(id, &ids).await {
                Ok(()) => {
                    // The list shows tag names, from the track record.
                    if let Ok(t) = c.track(id).await {
                        let cur = lib.peek().clone();
                        let mut t2 = cur
                            .by_id
                            .get(&id)
                            .map(|&i| cur.tracks[i].clone())
                            .unwrap_or(t.clone());
                        t2.tags = t.tags;
                        lib.set(cur.with_track(t2));
                    }
                    refresh.set(refresh() + 1);
                }
                Err(e) => props.on_flash.call(format!("TAGS FAILED: {e}")),
            }
        });
    });

    // metadata draft: fields start from the track and are saved together
    let mut meta = use_signal(|| Meta::from(&track));
    use_effect(use_reactive!(|(id,)| {
        let _ = id;
        meta.set(Meta::from(
            &lib.peek()
                .by_id
                .get(&id)
                .map(|&i| lib.peek().tracks[i].clone())
                .unwrap_or_default(),
        ));
    }));
    let c = client.clone();
    let save_meta = use_callback(move |_: ()| {
        let c = c.clone();
        let cur_track = lib
            .peek()
            .by_id
            .get(&id)
            .map(|&i| lib.peek().tracks[i].clone())
            .unwrap_or_default();
        let patch = meta.peek().patch_against(&cur_track);
        if patch == TrackMetadataPatch::default() {
            props.on_flash.call("NOTHING TO SAVE".into());
            return;
        }
        spawn(async move {
            match c.set_metadata(id, &patch).await {
                Ok(updated) => {
                    let cur = lib.peek().clone();
                    // The reply is a detail record: keep the list's own fields (tags, art).
                    let mut t = cur
                        .by_id
                        .get(&id)
                        .map(|&i| cur.tracks[i].clone())
                        .unwrap_or(updated.clone());
                    t.title = updated.title;
                    t.artist = updated.artist;
                    t.album = updated.album;
                    t.genre = updated.genre;
                    t.label = updated.label;
                    t.remixer = updated.remixer;
                    t.original_artist = updated.original_artist;
                    t.mix_name = updated.mix_name;
                    t.year = updated.year;
                    t.key = updated.key;
                    t.comment = updated.comment;
                    lib.set(cur.with_track(t));
                    props.on_flash.call("SAVED".into());
                }
                Err(e) => props.on_flash.call(format!("SAVE FAILED: {e}")),
            }
        });
    });
    let c = client.clone();
    let relink_file = use_callback(move |path: String| {
        let c = c.clone();
        spawn(async move {
            match c.relink_track(id, &path).await {
                Ok(updated) => {
                    let cur = lib.peek().clone();
                    if let Some(&i) = cur.by_id.get(&id) {
                        let mut t = cur.tracks[i].clone();
                        t.file_path = updated.file_path;
                        t.file_missing = updated.file_missing;
                        lib.set(cur.with_track(t));
                    }
                    props.on_flash.call("FILE RELINKED".into());
                }
                Err(e) => props.on_flash.call(format!("RELINK FAILED: {e}")),
            }
        });
    });

    // ---- derived values for rendering
    let grid_now = gdata.read().clone().flatten();
    let wave_now = wave.read().clone().flatten();
    let cues = cues_res.read().clone().unwrap_or_default();
    let tags_now = track_tags.read().clone().unwrap_or_default();
    let bpm_eff = grid_now.as_ref().map_or(track.bpm, |g| g.bpm);
    let pxps = grid::px_per_sec_bars(bpm_eff, bars());

    // These two are heavy: recompute only when the data or the tempo changes.
    let zoom = use_memo(use_reactive!(|(id, spectral_style, half)| {
        let _ = id;
        let w = wave.read().clone().flatten();
        let b = gdata
            .read()
            .clone()
            .flatten()
            .map_or(track_bpm_fallback(), |g| g.summary.bpm);
        let n = bars();
        let pxps = grid::px_per_sec_bars(b, n);
        w.map(|w| {
            Shared(Rc::new(match spectral_style {
                Some(style) => grid::zoom_spectral(&w.spectral.0, pxps, style, half),
                None => grid::zoom_paths(&w.entries.0, pxps, half),
            }))
        })
    }));
    let lines = use_memo(use_reactive!(|(id,)| {
        let _ = id;
        let b = gdata.read().clone().flatten();
        let n = bars();
        b.map(|g| {
            Shared(Rc::new(grid::grid_lines(
                &g.beats.0,
                grid::px_per_sec_bars(g.summary.bpm, n),
            )))
        })
    }));

    let duration = adur()
        .or_else(|| grid_now.as_ref().map(|g| g.duration_ms as f64 / 1000.0))
        .unwrap_or(track.duration as f64);
    let analysis_ms = wave_now
        .as_ref()
        .map(|w| (w.entries.0.len() as f64 / grid::ENTRIES_PER_SEC * 1000.0).round());
    let delta_len = match (adur(), analysis_ms) {
        (Some(a), Some(b)) => Some((a * 1000.0 - b).round() as i64),
        _ => None,
    };
    let transcoded = wav() || !NATIVE.contains(&track.file_type.as_str());
    let loop_now = active_loop();
    let info = format!(
        "{}{}{}{}",
        if transcoded { "TRANSCODED" } else { "DIRECT" },
        match delta_len {
            Some(d) => format!(" \u{00B7} \u{0394}len {d:+}ms"),
            None => String::new(),
        },
        match loop_now {
            Some((a, b)) => format!(
                " \u{00B7} LOOP {} - {}",
                grid::time_label((a * 1000.0) as i64),
                grid::time_label((b * 1000.0) as i64)
            ),
            None => String::new(),
        },
        match aerr() {
            Some(e) => format!(" \u{00B7} {e}"),
            None => String::new(),
        }
    );
    let playing = astatus() == PlayStatus::Playing;

    let det_text = grid_now.as_ref().map_or(String::new(), |g| {
        let s = &g.summary;
        if s.detected_bpm > 0.0 && (s.bpm - s.detected_bpm).abs() > 0.001 {
            format!("detected: {:.2}", s.detected_bpm)
        } else if s.beat_phase_shift != 0 {
            format!("phase shifted +{}", s.beat_phase_shift)
        } else {
            "(no override)".to_string()
        }
    });
    let phase = grid_now
        .as_ref()
        .map_or(0, |g| g.summary.beat_phase_shift.rem_euclid(4));
    let locked = locks().has(id);
    let toggle_lock = {
        let store = platform.store.clone();
        move |_| {
            let mut l = locks.peek().clone();
            let now = l.toggle(id);
            let _ = store.set_json(crate::prefs::Locks::KEY, &l);
            let mut locks = locks;
            locks.set(l);
            props.on_flash.call(if now {
                "GRID AND ANALYSIS LOCKED".into()
            } else {
                "GRID UNLOCKED".into()
            });
        }
    };
    let draft_val = draft_bpm();
    let draft_num = draft_val
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| (20.0..=400.0).contains(v));
    let cur_bpm = draft_num.unwrap_or(bpm_eff);

    let height_style = if dh() > 0.0 {
        format!("height:{}px", dh())
    } else {
        "height:50%".to_string()
    };
    let art_url = client.url(&format!("/api/artwork/{}", track.id));
    let mut art_failed = use_signal(|| false);
    let side_cut = if dh() > 0.0 {
        format!("{}px", dh())
    } else {
        "50%".to_string()
    };
    let handle_style = if dh() > 0.0 {
        format!("bottom:calc({}px - 9px)", dh())
    } else {
        "bottom:calc(50% - 9px)".to_string()
    };

    rsx! {
        div { class: "drg", style: "{handle_style}", role: "separator",
            title: "Drag to resize the drawer. Double click to reset.",
            onpointerdown: move |e| {
                e.prevent_default();
                resizing.set(Some((e.client_coordinates().y, if dh() > 0.0 { dh() } else { -1.0 })));
            },
            ondoubleclick: move |_| dh.set(0.0),
            i {}
        }
        // The drawer covers the bottom of the sidebar, whose scrollbar some webviews paint above it: end the sidebar where the drawer starts.
        style { ".lib .side{{max-height:calc(100% - {side_cut})}}" }
        section { class: "drw", style: "{height_style}", "aria-label": "Track details",
            div { class: "dh",
                div { class: "art", style: "{art_style(id)}",
                    if !art_failed() && (track.art_id > 0 || !track.art_checked) {
                        img { src: "{art_url}", alt: "", onerror: move |_| art_failed.set(true) }
                    }
                }
                div { class: "tt",
                    div { class: "ttl", style: "font-size:20px", "{track.title}" }
                    div { class: "mut", "{track.artist}" }
                }
                div { class: "kv", span { "BPM" } b { "{bpm_eff:.2}" } }
                div { class: "kv", span { "KEY" } b { "{track.key}" } }
                div { class: "kv", span { "LENGTH" } b { "{mmss(track.duration)}" } }
                span { class: "sp" }
                span { class: "ldg",
                    span { class: "sh2", "LOAD ON" }
                    if props.decks.is_empty() { span { class: "mut", "NO PLAYERS" } }
                    for d in props.decks.iter() {
                        {
                            let n = d.n;
                            rsx! {
                                button { key: "{n}", class: "{d.cls}", title: "{d.title}", onclick: move |_| props.on_load.call(n), "{n}" }
                            }
                        }
                    }
                }
                button { class: "btn on", onclick: move |_| save_meta.call(()), "SAVE" }
                button { class: "btn", "aria-label": "Close track details", onclick: move |_| props.on_close.call(()), "CLOSE" }
            }

            div { class: "wv",
                div { class: "wt",
                    span { class: "sh2", "ZOOM  {bars()} BARS" }
                    button { class: "btn sm", title: "Zoom in (Ctrl-scroll up)", "aria-label": "Zoom in", disabled: bars() == grid::ZOOM_BARS[0], onclick: move |_| zoom_step.call(false), "+" }
                    button { class: "btn sm", title: "Zoom out (Ctrl-scroll down)", "aria-label": "Zoom out", disabled: bars() == grid::ZOOM_BARS[grid::ZOOM_BARS.len() - 1], onclick: move |_| zoom_step.call(true), "−" }
                    span { class: "sp" }
                    div { class: "bj",
                        span { class: "sh2", style: "align-self:center", "BEAT JUMP" }
                        for n in [-16, -4, -1, 1, 4, 16] {
                            button {
                                key: "{n}",
                                class: "btn sm",
                                onclick: move |_| {
                                    let beats = gdata.peek().as_ref().and_then(|g| g.as_ref()).map(|g| g.beats.clone());
                                    let b = beats.as_ref().map(|b| &b.0[..]).unwrap_or(&[]);
                                    // Read the position first: a `peek()` temporary would still be
                                    // borrowed when `seek` writes the signal, and that panics.
                                    let pos = *cursor.peek();
                                    seek.call(grid::jump_beats(b, pos, n, cur_bpm, duration));
                                },
                                if n > 0 { "+{n}" } else { "{n}" }
                            }
                        }
                    }
                }
                ZoomView { zoom: zoom(), lines: lines(), cues: cues.clone(), phrases: phrases_res.read().clone().unwrap_or_default(), cursor, pxps, duration, on_scrub: move |secs| seek.call(secs), on_zoom: move |out| zoom_step.call(out), on_add_cue: move |secs| add_cue_at.call(secs), on_move_cue: move |(n, secs)| move_cue.call((n, secs)) }
                Overview {
                    paths: wave_now.as_ref().map(|w| w.overview.clone()),
                    cues: cues.clone(),
                    cursor,
                    duration,
                    window: grid::window_secs_bars(bpm_eff, bars()),
                    on_seek: move |secs| seek.call(secs),
                }
                PhraseStrip { phrases: phrases_res.read().clone().unwrap_or_default(), span_ms: duration * 1000.0, class: "full" }

                div { class: "wt pr2",
                    span { class: "sh2", "PLAYBACK" }
                    button { class: "btn ib", title: "Previous cue", "aria-label": "Previous cue",
                        onclick: move |_| {
                            let pos = *cursor.peek();
                            let target = grid::step_cue(&cues_now(&cues_res), pos, -1);
                            if let Some(t) = target { seek.call(t); }
                        },
                        svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M6 5v14M19 5l-9 7 9 7z", fill: "currentColor" } }
                    }
                    button {
                        class: if playing { "btn ib act" } else { "btn ib" },
                        title: "Play or pause from the cursor",
                        "aria-label": "Play or pause",
                        onclick: {
                            let audio = audio.clone();
                            move |_| if playing { audio.pause() } else { audio.play() }
                        },
                        svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            if playing { path { d: "M8 5v14M16 5v14", stroke_width: "3.4" } } else { path { d: "M7 4.5v15l12-7.5z", fill: "currentColor" } } }
                    }
                    button { class: "btn ib", title: "Next cue", "aria-label": "Next cue",
                        onclick: move |_| {
                            let pos = *cursor.peek();
                            let target = grid::step_cue(&cues_now(&cues_res), pos, 1);
                            if let Some(t) = target { seek.call(t); }
                        },
                        svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M18 5v14M5 5l9 7-9 7z", fill: "currentColor" } }
                    }
                    Timecode { cursor, duration }
                    button {
                        class: if metro() { "btn ib act" } else { "btn ib" },
                        title: "Click on every beat of the grid",
                        "aria-label": "Metronome",
                        onclick: {
                            let store = platform.store.clone();
                            move |_| {
                                metro.toggle();
                                last_click.set(None);
                                let _ = store.set_json(METRONOME, &metro());
                            }
                        },
                        svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                            path { d: "M9 3h6l3.5 18h-13zM12 17l4-9M10 12h4" } }
                    }
                    button {
                        class: if wav() { "btn wavb act" } else { "btn wavb" },
                        title: "Play the ffmpeg decoded WAV, the same decoder as the analysis, to rule out a browser offset",
                        onclick: move |_| wav.toggle(),
                        "WAV"
                    }
                    span { class: "vol",
                        button { class: "btn ib", title: "Mute", "aria-label": "Mute",
                            onclick: {
                                let audio = audio.clone();
                                let store = platform.store.clone();
                                move |_| {
                                    let v = if volume() == 0 { prev_volume().max(1) } else { prev_volume.set(volume()); 0 };
                                    volume.set(v);
                                    audio.set_volume(v as f64 / 100.0);
                                    let _ = store.set_json(VOLUME, &v);
                                }
                            },
                            svg { width: "22", height: "22", view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round",
                                path { d: "M4 9.5v5h4l5 4v-13l-5 4z", fill: "currentColor" }
                                if volume() == 0 { path { d: "M16.5 9.5l5 5M21.5 9.5l-5 5" } }
                                else if volume() < 50 { path { d: "M16.5 9.5a3.5 3.5 0 010 5" } }
                                else { path { d: "M16.5 9.5a3.5 3.5 0 010 5M19 7a7 7 0 010 10" } } }
                        }
                        input {
                            class: "vr",
                            r#type: "range",
                            min: "0",
                            max: "100",
                            step: "1",
                            value: "{volume()}",
                            style: "--p:{volume()}%",
                            "aria-label": "Volume",
                            oninput: {
                                let audio = audio.clone();
                                let store = platform.store.clone();
                                move |e| {
                                    let v = e.value().parse::<u32>().unwrap_or(0).min(100);
                                    volume.set(v);
                                    audio.set_volume(v as f64 / 100.0);
                                    let _ = store.set_json(VOLUME, &v);
                                }
                            },
                        }
                        span { class: "mono mut", style: "font-size:12px;min-width:34px", "{volume()}" }
                    }
                    span { class: "sp" }
                    if loop_now.is_some() {
                        button { class: "btn sm", title: "Stop looping", onclick: move |_| active_loop.set(None), "EXIT LOOP" }
                    }
                    span { class: "pinfo", "{info}" }
                }

                div { class: "wt",
                    span { class: "sh2", "BPM" }
                    input {
                        class: "in bpm",
                        r#type: "number",
                        min: "20",
                        max: "400",
                        step: "0.01",
                        value: "{draft_val}",
                        "aria-label": "BPM",
                        oninput: move |e| draft_bpm.set(e.value()),
                    }
                    button { class: "btn ib", title: "Halve the BPM", "aria-label": "Halve the BPM",
                        disabled: locked, onclick: move |_| grid_edit.call(GridEdit::Bpm(cur_bpm / 2.0)), "/2" }
                    button { class: "btn ib", title: "Double the BPM", "aria-label": "Double the BPM",
                        disabled: locked, onclick: move |_| grid_edit.call(GridEdit::Bpm(cur_bpm * 2.0)), "x2" }
                    button { class: "btn", title: "Tap to the beat to set the BPM. The count resets after 2 seconds.",
                        onclick: move |_| {
                            let r = taps.write().tap(time::now_ms());
                            if let Some(b) = r { draft_bpm.set(format!("{b:.2}")); }
                        },
                        "TAP "
                        span { class: "mono mut", if taps.read().count() > 0 { "({taps.read().count()})" } }
                    }
                    button { class: "btn on", disabled: draft_num.is_none() || busy() || locked,
                        onclick: move |_| if let Some(b) = draft_num { grid_edit.call(GridEdit::Bpm(b)) },
                        "SAVE BPM" }
                    span { class: "mono mut", style: "font-size:12px", "{det_text}" }
                    span { class: "sp" }
                    span { class: "sh2", "PHASE" }
                    div { class: "seg",
                        for n in 0..4_i64 {
                            button {
                                key: "{n}",
                                class: if n == phase { "on" } else { "" },
                                title: "Place the first bar line on this beat",
                                disabled: locked,
                                onclick: move |_| grid_edit.call(GridEdit::Phase(n)),
                                if n == 0 { "0" } else { "+{n}" }
                            }
                        }
                    }
                    div { class: "bj",
                        button { class: "btn ib", title: "Clear the BPM and phase override", "aria-label": "Clear the BPM and phase override",
                            disabled: locked, onclick: move |_| grid_edit.call(GridEdit::Clear), "↶" }
                    }
                }
                div { class: "wt",
                    span { class: "sh2", "GRID" }
                    button {
                        class: if locked { "btn sm on" } else { "btn sm" },
                        title: if locked { "Locked: this client will not edit the grid or analyze this track again. Click to unlock." } else { "Lock the beat grid and the analysis against edits and re-analysis from this client" },
                        "aria-pressed": "{locked}",
                        "aria-label": if locked { "Unlock the beat grid and analysis" } else { "Lock the beat grid and analysis" },
                        onclick: toggle_lock,
                        if locked { "🔒 LOCKED" } else { "🔓 LOCK" }
                    }
                    span { class: "mono mut", style: "font-size:12px;min-width:92px", "SHIFT {shift_ms():+} MS" }
                    div { class: "bj",
                        button { class: "btn ib", title: "Shift the grid 10 ms earlier", "aria-label": "Shift the grid 10 ms earlier",
                            disabled: locked, onclick: move |_| grid_edit.call(GridEdit::Shift(-10)), "«" }
                        button { class: "btn ib", title: "Shift the grid 10 ms later", "aria-label": "Shift the grid 10 ms later",
                            disabled: locked, onclick: move |_| grid_edit.call(GridEdit::Shift(10)), "»" }
                        button { class: "btn ib", title: "Set the downbeat at the playhead", "aria-label": "Set the downbeat at the playhead",
                            disabled: locked, onclick: move |_| grid_edit.call(GridEdit::Downbeat((*cursor.peek() * 1000.0).round())), "⚑" }
                        button { class: "btn ib", title: "Analyze the track again", "aria-label": "Analyze the track again",
                            disabled: locked, onclick: move |_| grid_edit.call(GridEdit::Reanalyze), "⟳" }
                    }
                }
            }

            div { class: "g3",
                div { class: "card",
                    span { class: "sh2", "METADATA" }
                    div { class: "fld",
                        for (label , field , wide) in META_FIELDS {
                            label { key: "{label}", class: if wide { "w" } else { "" },
                                "{label}"
                                input {
                                    class: "in",
                                    r#type: "text",
                                    value: "{meta.read().get(field)}",
                                    oninput: move |e| meta.write().set(field, e.value()),
                                }
                            }
                        }
                    }
                }
                div { class: "card",
                    span { class: "sh2", "FILE AND LIBRARY" }
                    dl { class: "ro", "aria-label": "Track information",
                        for (k , v) in info_rows(&track, bpm_eff) {
                            dt { key: "{k}", "{k}" }
                            dd { class: if v == "—" { "dim" } else { "" }, "{v}" }
                        }
                    }
                }
                div { class: "card",
                    span { class: "sh2", "CUES" }
                    div {
                        if cues.is_empty() { span { class: "mut", "No cues." } }
                        for c in cues.iter() {
                            {
                                let t = c.time_ms as f64 / 1000.0;
                                let number = c.number;
                                let loop_badge = c.kind == CueKind::Loop;
                                let loop_end = (c.kind == CueKind::Loop && c.loop_ms > c.time_ms).then(|| c.loop_ms as f64 / 1000.0);
                                rsx! {
                                    div { key: "{number}", class: "cue", title: "Play from this cue", onclick: move |_| {
                                            play_from.call(t);
                                            active_loop.set(loop_end.map(|end| (t, end)));
                                        },
                                        i { style: "background:{cue_color(c)}", "{grid::cue_letter(c.number)}" }
                                        span { class: "ttl", "{cue_name(c)}" }
                                        if loop_badge { span { class: "fl am", "LOOP" } }
                                        span { class: "mono mut", "{grid::time_label(c.time_ms)}" }
                                        button { class: "btn ib", style: "width:32px;height:32px", title: "Delete this cue", "aria-label": "Delete this cue",
                                            onclick: move |e| { e.stop_propagation(); del_cue.call(number); }, "✕" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "bj",
                        button { class: "btn sm", title: "Add a hot cue at the playhead",
                            onclick: {
                                let cues = cues.clone();
                                move |_| match grid::free_hot_cue(&cues) {
                                    Some(n) => put_cue.call(CueInfo { number: n, kind: CueKind::Cue, time_ms: (*cursor.peek() * 1000.0).round() as i64, loop_ms: -1, color_id: cue_colors().for_cue(n), label: String::new() }),
                                    None => props.on_flash.call("ALL 8 HOT CUES ARE USED".into()),
                                }
                            },
                            "+ CUE" }
                        button { class: "btn sm", title: "Add a memory cue at the playhead",
                            onclick: {
                                let cues = cues.clone();
                                move |_| put_cue.call(CueInfo { number: grid::next_memory_cue(&cues), kind: CueKind::Cue, time_ms: (*cursor.peek() * 1000.0).round() as i64, loop_ms: -1, color_id: 0, label: String::new() })
                            },
                            "+ MEM" }
                    }
                }
                div { class: "card",
                    span { class: "sh2", "TRACK COLOR" }
                    div { class: "sw",
                        for (cid , name , hex) in palette::PALETTE {
                            button {
                                key: "{cid}",
                                class: if cid == 0 { if track.color_id == 0 { "nn on" } else { "nn" } } else if cid == track.color_id { "on" } else { "" },
                                style: if cid == 0 { "background:#161B22".to_string() } else { format!("background:{hex}") },
                                "aria-label": "{name}",
                                title: "{name}",
                                onclick: move |_| set_color.call(cid),
                            }
                        }
                    }
                    span { class: "sh2", "RECENT" }
                    div { class: "sw rc",
                        for cid in colors_hist() {
                            {
                                let (name, hex) = palette::color(cid).unwrap_or(("NONE", "#3A4350"));
                                rsx! {
                                    button { key: "{cid}", class: if cid == track.color_id { "on" } else { "" }, style: "background:{hex}",
                                        "aria-label": "{name}", title: "{name}", onclick: move |_| set_color.call(cid) }
                                }
                            }
                        }
                    }
                    span { class: "sh2", "TAGS" }
                    div { class: "bj",
                        for g in tags_now.iter() {
                            span { key: "{g.id}", class: "tag", style: "border-color:{palette::tag_color(g.color, g.id)}",
                                i { style: "background:{palette::tag_color(g.color, g.id)}" }
                                "{g.name}"
                            }
                        }
                        button { class: "tag", style: "border-style:dashed;color:#9AA4B2;background:none", onclick: move |_| tag_pick.toggle(),
                            if tag_pick() { "DONE" } else { "+ TAG" } }
                    }
                    if tag_pick() {
                        TagPicker { all: props.lib.read().tags.clone(), have: tags_now.clone(), on_set: move |ids| set_tags.call(ids) }
                    }
                    span { class: "sh2", "FILE" }
                    div { class: "pth", "{track.file_path}" }
                    if track.file_missing {
                        div { class: "bj",
                            span { class: "fl rd", style: "margin:0;align-self:center", "MISSING" }
                        }
                        div { class: "bj",
                            input { class: "in", r#type: "text", placeholder: "New path to the file", style: "flex:1;min-width:0",
                                value: "{relink()}", oninput: move |e| relink.set(e.value()) }
                            button { class: "btn sm", disabled: relink().trim().is_empty(),
                                onclick: move |_| relink_file.call(relink().trim().to_string()), "RELINK FILE" }
                        }
                    }
                }
            }
        }
        if let Some((y0, h0)) = resizing() {
            div {
                class: "rsz-shield",
                style: "cursor:row-resize",
                onpointermove: move |e| {
                    // The drawer is anchored to the bottom, so dragging up makes it taller.
                    let base = if h0 > 0.0 { h0 } else { 480.0 };
                    dh.set((base + y0 - e.client_coordinates().y).clamp(200.0, 900.0));
                },
                onpointerup: move |_| resizing.set(None),
                onpointerleave: move |_| resizing.set(None),
            }
        }
    }
}

fn track_bpm_fallback() -> f64 {
    120.0
}

fn cues_now(r: &Resource<Vec<CueInfo>>) -> Vec<CueInfo> {
    r.peek().clone().unwrap_or_default()
}

/// A grid edit the user asked for.
#[derive(Clone, Copy, PartialEq)]
enum GridEdit {
    /// Change the tempo and keep the first bar line where it is.
    Bpm(f64),
    Phase(i64),
    /// Drop the override and analyze again.
    Clear,
    /// Move the whole grid by this many ms.
    Shift(i64),
    /// Put a bar line at this time (ms).
    Downbeat(f64),
    Reanalyze,
}

impl GridEdit {
    /// Returns the shift in ms this edit adds to the session's shift counter.
    async fn apply(
        self,
        c: &vynull_api::Client,
        id: i64,
        beats: Option<&[BeatInfo]>,
    ) -> Result<i64, vynull_api::Error> {
        match self {
            GridEdit::Bpm(b) => {
                let b = (b.clamp(20.0, 400.0) * 100.0).round() / 100.0;
                c.adjust_beatgrid(&BeatgridAdjust {
                    track_id: id,
                    bpm: Some(b),
                    set_downbeat_ms: beats.and_then(grid::first_downbeat_ms),
                    ..Default::default()
                })
                .await?;
                Ok(0)
            }
            GridEdit::Phase(n) => {
                c.set_beats(
                    id,
                    &BeatsPatch {
                        beat_phase_shift: Some(n),
                        ..Default::default()
                    },
                )
                .await?;
                Ok(0)
            }
            GridEdit::Clear => {
                c.set_beats(
                    id,
                    &BeatsPatch {
                        clear: Some(true),
                        ..Default::default()
                    },
                )
                .await?;
                // The daemon keeps the old beat list after a clear; analyzing again rebuilds it.
                c.reanalyze(id).await?;
                Ok(0)
            }
            GridEdit::Shift(ms) => {
                c.adjust_beatgrid(&BeatgridAdjust {
                    track_id: id,
                    offset_ms: Some(ms as f64),
                    ..Default::default()
                })
                .await?;
                Ok(ms)
            }
            GridEdit::Downbeat(ms) => {
                c.adjust_beatgrid(&BeatgridAdjust {
                    track_id: id,
                    set_downbeat_ms: Some(ms),
                    ..Default::default()
                })
                .await?;
                Ok(0)
            }
            GridEdit::Reanalyze => {
                c.reanalyze(id).await?;
                Ok(0)
            }
        }
    }
}

const META_FIELDS: [(&str, MetaField, bool); 11] = [
    ("TITLE", MetaField::Title, true),
    ("ARTIST", MetaField::Artist, false),
    ("ALBUM", MetaField::Album, false),
    ("LABEL", MetaField::Label, false),
    ("REMIXER", MetaField::Remixer, false),
    ("GENRE", MetaField::Genre, false),
    ("YEAR", MetaField::Year, false),
    ("KEY", MetaField::Key, false),
    ("ORIGINAL ARTIST", MetaField::OriginalArtist, false),
    ("MIX NAME", MetaField::MixName, false),
    ("COMMENT", MetaField::Comment, true),
];

#[derive(Clone, Copy, PartialEq)]
enum MetaField {
    Title,
    Artist,
    Album,
    Label,
    Remixer,
    Genre,
    OriginalArtist,
    MixName,
    Year,
    Key,
    Comment,
}

#[derive(Clone, PartialEq, Default)]
struct Meta {
    title: String,
    artist: String,
    album: String,
    label: String,
    remixer: String,
    genre: String,
    original_artist: String,
    mix_name: String,
    year: String,
    key: String,
    comment: String,
}

impl Meta {
    fn from(t: &TrackInfo) -> Self {
        Meta {
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            label: t.label.clone(),
            remixer: t.remixer.clone(),
            genre: t.genre.clone(),
            original_artist: t.original_artist.clone(),
            mix_name: t.mix_name.clone(),
            year: if t.year > 0 {
                t.year.to_string()
            } else {
                String::new()
            },
            key: t.key.clone(),
            comment: t.comment.clone(),
        }
    }

    fn get(&self, f: MetaField) -> &str {
        match f {
            MetaField::Title => &self.title,
            MetaField::Artist => &self.artist,
            MetaField::Album => &self.album,
            MetaField::Label => &self.label,
            MetaField::Remixer => &self.remixer,
            MetaField::Genre => &self.genre,
            MetaField::OriginalArtist => &self.original_artist,
            MetaField::MixName => &self.mix_name,
            MetaField::Year => &self.year,
            MetaField::Key => &self.key,
            MetaField::Comment => &self.comment,
        }
    }

    fn set(&mut self, f: MetaField, v: String) {
        match f {
            MetaField::Title => self.title = v,
            MetaField::Artist => self.artist = v,
            MetaField::Album => self.album = v,
            MetaField::Label => self.label = v,
            MetaField::Remixer => self.remixer = v,
            MetaField::Genre => self.genre = v,
            MetaField::OriginalArtist => self.original_artist = v,
            MetaField::MixName => self.mix_name = v,
            MetaField::Year => self.year = v,
            MetaField::Key => self.key = v,
            MetaField::Comment => self.comment = v,
        }
    }

    /// Only the fields that differ from the track, as the API treats omitted fields as unchanged.
    fn patch_against(&self, t: &TrackInfo) -> TrackMetadataPatch {
        let d = |new: &String, old: &String| (new != old).then(|| new.clone());
        TrackMetadataPatch {
            title: d(&self.title, &t.title),
            artist: d(&self.artist, &t.artist),
            album: d(&self.album, &t.album),
            genre: d(&self.genre, &t.genre),
            label: d(&self.label, &t.label),
            remixer: d(&self.remixer, &t.remixer),
            original_artist: d(&self.original_artist, &t.original_artist),
            mix_name: d(&self.mix_name, &t.mix_name),
            year: (self.year != Meta::from(t).year).then(|| self.year.clone()),
            key: d(&self.key, &t.key),
            comment: d(&self.comment, &t.comment),
        }
    }
}

#[component]
fn Timecode(cursor: Signal<f64>, duration: f64) -> Element {
    rsx! {
        span { class: "tcode",
            "{grid::time_label((cursor() * 1000.0) as i64)} "
            span { class: "mut", style: "font-size:13px", "/ {mmss(duration as i64)}" }
        }
    }
}

#[component]
fn ZoomView(
    zoom: Option<Shared<Zoom>>,
    lines: Option<Shared<Grid>>,
    cues: Vec<CueInfo>,
    phrases: Vec<PhraseInfo>,
    cursor: Signal<f64>,
    pxps: f64,
    duration: f64,
    on_scrub: EventHandler<f64>,
    on_zoom: EventHandler<bool>,
    on_add_cue: EventHandler<f64>,
    on_move_cue: EventHandler<(i64, f64)>,
) -> Element {
    // The view's width in px, measured when a drag starts.
    let mut view_w = use_signal(|| 0.0_f64);
    // (cue number, pointer x at the start, cue time at the start, cue time now) while moving a cue.
    let mut cue_drag = use_signal(|| None::<(i64, f64, f64, f64)>);
    // (pointer x at the start, cursor at the start) while dragging.
    let mut drag = use_signal(|| None::<(f64, f64)>);
    let Some(z) = zoom else {
        return rsx! { div { class: "zw", div { class: "none", style: "padding:0;height:100%;justify-content:center", span { "LOADING WAVEFORM…" } } } };
    };
    let total = z.0.total_cols.max(1.0);
    // The strip is as wide as the track; the view slides it so the cursor sits at the center line.
    let width_pct = total / grid::VIEW_COLS * 100.0;
    // A cue being dragged is drawn where it would land.
    let shown_cues: Vec<CueInfo> = cues
        .iter()
        .map(|c| match cue_drag() {
            Some((n, _, _, now)) if n == c.number => CueInfo {
                time_ms: (now * 1000.0).round() as i64,
                ..c.clone()
            },
            _ => c.clone(),
        })
        .collect();
    let shift_pct = (cursor() * pxps - grid::VIEW_COLS / 2.0) / total * 100.0;
    rsx! {
        div {
            class: "zw",
            style: "cursor:grab",
            title: "Drag to move through the track. Shift-click adds a hot cue there. Ctrl-scroll zooms.",
            onwheel: move |e| {
                if e.modifiers().ctrl() {
                    e.prevent_default();
                    let dy = e.delta().strip_units().y;
                    if dy != 0.0 {
                        on_zoom.call(dy > 0.0);
                    }
                }
            },
            onpointerdown: move |e| {
                e.prevent_default();
                if e.modifiers().shift() {
                    let x = e.client_coordinates().x;
                    let center = *cursor.peek();
                    spawn(async move {
                        if let Some((left, w)) = crate::dom::left_and_width(".drw .zw").await {
                            if w > 0.0 {
                                let secs = center + ((x - left) / w - 0.5) * grid::window_secs_for(pxps);
                                on_add_cue.call(secs.clamp(0.0, duration.max(0.0)));
                            }
                        }
                    });
                    return;
                }
                drag.set(Some((e.client_coordinates().x, *cursor.peek())));
                spawn(async move {
                    if let Some((_, w)) = crate::dom::left_and_width(".drw .zw").await {
                        view_w.set(w);
                    }
                });
            },
            // A click without movement ends before the full-screen layer below exists.
            onpointerup: move |_| drag.set(None),
            div { class: "zin", style: "width:{width_pct}%;transform:translateX({-shift_pct}%)",
                ZoomLayers { zoom: z.clone(), lines: lines.clone(), phrases, pxps, total }
                CueFlags {
                    cues: shown_cues,
                    pxps,
                    total,
                    on_grab: {
                        let cues = cues.clone();
                        move |(number, x): (i64, f64)| {
                            let Some(c) = cues.iter().find(|c| c.number == number) else { return };
                            cue_drag.set(Some((number, x, c.time_ms as f64 / 1000.0, c.time_ms as f64 / 1000.0)));
                            spawn(async move {
                                if let Some((_, w)) = crate::dom::left_and_width(".drw .zw").await {
                                    view_w.set(w);
                                }
                            });
                        }
                    },
                }
            }
            div { class: "ph" }
        }
        if let Some((number, x0, t0, _)) = cue_drag() {
            div {
                class: "rsz-shield",
                style: "cursor:ew-resize",
                onpointermove: move |e| {
                    let width = view_w();
                    if width > 0.0 {
                        let dx = e.client_coordinates().x - x0;
                        let secs = (t0 + dx / width * grid::window_secs_for(pxps)).clamp(0.0, duration.max(0.0));
                        cue_drag.set(Some((number, x0, t0, secs)));
                    }
                },
                onpointerup: move |_| {
                    let cur: Option<(i64, f64, f64, f64)> = *cue_drag.peek();
                    let now = cur.map(|d| d.3);
                    cue_drag.set(None);
                    if let Some(secs) = now {
                        if (secs - t0).abs() > 0.004 {
                            on_move_cue.call((number, secs));
                        }
                    }
                },
                onpointerleave: move |_| cue_drag.set(None),
            }
        }
        if let Some((x0, start)) = drag() {
            div {
                class: "rsz-shield",
                style: "cursor:grabbing",
                onpointermove: move |e| {
                    let width = view_w();
                    if width > 0.0 {
                        // Dragging right pulls earlier audio under the center line.
                        let dx = e.client_coordinates().x - x0;
                        let secs = start - dx / width * grid::window_secs_for(pxps);
                        on_scrub.call(secs.clamp(0.0, duration.max(0.0)));
                    }
                },
                onpointerup: move |_| drag.set(None),
                onpointerleave: move |_| drag.set(None),
            }
        }
    }
}

/// The part of the zoom view that does not change while the playhead moves.
#[component]
fn ZoomLayers(
    zoom: Shared<Zoom>,
    lines: Option<Shared<Grid>>,
    phrases: Vec<PhraseInfo>,
    pxps: f64,
    total: f64,
) -> Element {
    let z = &zoom.0;
    rsx! {
        svg { view_box: "0 0 {total} 100", preserve_aspect_ratio: "none",
            if let Some(g) = &lines {
                path { d: "{g.0.beats}", fill: "none", class: "gbeat", stroke_width: "1", vector_effect: "non-scaling-stroke" }
                path { d: "{g.0.bars}", fill: "none", class: "gbar", stroke_width: "1.5", vector_effect: "non-scaling-stroke" }
            }
            if z.colored.is_empty() {
                path { d: "{z.lo}", class: "wl" }
                path { d: "{z.mi}", class: "wm" }
                path { d: "{z.hi}", class: "wh" }
            }
            for (fill , d) in z.colored.iter() {
                path { key: "{fill}", d: "{d}", fill: "{fill}" }
            }
        }
        // The strip is as wide as the canvas, so one pixel column is 1000 / pxps ms.
        PhraseStrip { phrases, span_ms: total / pxps.max(0.001) * 1000.0, class: "inzoom" }
        if let Some(g) = &lines {
            for (x , n) in g.0.bar_labels.iter() {
                if *n % 4 == 1 {
                    span { key: "{n}", class: "bn", style: "left:{x / total * 100.0:.3}%", "{n}" }
                }
            }
        }
    }
}

/// The cue flags over the zoom view. Apart from [`ZoomLayers`] so that dragging
/// a flag redraws only the flags.
#[component]
fn CueFlags(
    cues: Vec<CueInfo>,
    pxps: f64,
    total: f64,
    on_grab: EventHandler<(i64, f64)>,
) -> Element {
    rsx! {
        for c in cues.iter() {
            {
                let x = c.time_ms as f64 / 1000.0 * pxps / total * 100.0;
                let color = cue_color(c);
                let number = c.number;
                rsx! {
                    div { key: "{c.number}", class: "fg", style: "left:{x:.3}%;background:{color}",
                        i {
                            class: "hit",
                            title: "Drag to move this cue",
                            onpointerdown: move |e| {
                                e.prevent_default();
                                e.stop_propagation();
                                on_grab.call((number, e.client_coordinates().x));
                            },
                        }
                        span { style: "background:{color}", "{grid::cue_letter(c.number)}" }
                    }
                }
            }
        }
    }
}

#[component]
fn Overview(
    paths: Option<OverviewPaths>,
    cues: Vec<CueInfo>,
    cursor: Signal<f64>,
    duration: f64,
    window: f64,
    on_seek: EventHandler<f64>,
) -> Element {
    let dur = duration.max(0.001);
    let left = ((cursor() - window / 2.0) / dur * 100.0).clamp(-100.0, 100.0);
    let width = (window / dur * 100.0).min(100.0);
    let cue_pos: Vec<f64> = cues
        .iter()
        .map(|c| c.time_ms as f64 / 1000.0 / dur)
        .collect();
    rsx! {
        div {
            class: "ovw",
            onclick: move |e| {
                let x = e.client_coordinates().x;
                spawn(async move {
                    if let Some((left, width)) = crate::dom::left_and_width(".drw .ovw").await {
                        on_seek.call(((x - left) / width).clamp(0.0, 1.0) * dur);
                    }
                });
            },
            svg { view_box: "0 0 400 100", preserve_aspect_ratio: "none",
                if let Some(p) = &paths {
                    if p.colored.is_empty() {
                        path { d: "{p.lo}", class: "wl" }
                        path { d: "{p.mi}", class: "wm" }
                        path { d: "{p.hi}", class: "wh" }
                    }
                    for (fill , d) in p.colored.iter() {
                        path { key: "{fill}", d: "{d}", fill: "{fill}" }
                    }
                }
                path { d: "{waveform::cue_marks(&cue_pos)}", fill: "#FF6B6B" }
            }
            div { class: "vp", style: "left:{left}%;width:{width}%" }
        }
    }
}

#[component]
fn TagPicker(all: Rc<Vec<TagInfo>>, have: Vec<TagInfo>, on_set: EventHandler<Vec<i64>>) -> Element {
    let have_ids: Vec<i64> = have.iter().map(|t| t.id).collect();
    rsx! {
        div { class: "bj", style: "max-height:140px;overflow-y:auto",
            for g in all.iter() {
                {
                    let id = g.id;
                    let on = have_ids.contains(&id);
                    let color = palette::tag_color(g.color, id);
                    let style = if on {
                        format!("background:{color};border-color:{color};color:#05070A")
                    } else {
                        "background:var(--surface);border-color:var(--line2);color:var(--text)".to_string()
                    };
                    let ids = have_ids.clone();
                    rsx! {
                        button { key: "{id}", class: "tag", style: "{style}",
                            onclick: move |_| {
                                let mut next = ids.clone();
                                if let Some(p) = next.iter().position(|x| *x == id) { next.remove(p); } else { next.push(id); }
                                on_set.call(next);
                            },
                            "{g.name}" }
                    }
                }
            }
        }
    }
}
