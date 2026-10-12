//! The waveform stack on the Players screen: one scrolling waveform per deck
//! that has a track, all centred on the same vertical line, so beats that line
//! up on the decks line up on screen. Rows appear when a deck loads a track and
//! go when it empties; nothing is added by hand.

use std::rc::Rc;

use dioxus::prelude::*;
use vynull_api::{BeatInfo, CueInfo, PlayerInfo, WaveEntry3Band, WaveEntrySpectral};
use vynull_core::grid::{self, Grid, Zoom};
use vynull_core::playhead;
use vynull_core::waveform::Spectral;
use vynull_platform::{time, StoreExt};

use crate::drawer::Shared;
use crate::live::Feed;
use crate::shell::{LookCtx, Platform};

const BARS_KEY: &str = "stack_bars";
/// How many bars the stack shows across, unless the user changed it. Four bars is 16 beats.
const DEFAULT_BARS: u32 = 4;

/// What one row needs from the daemon, fetched once per track.
#[derive(Clone)]
struct Raw {
    spectral: Shared<Vec<WaveEntrySpectral>>,
    bands: Shared<Vec<WaveEntry3Band>>,
    beats: Shared<Vec<BeatInfo>>,
    cues: Vec<CueInfo>,
    /// The track's own tempo, which the strip is drawn at (the live tempo only rescales it).
    bpm: f64,
}

impl PartialEq for Raw {
    fn eq(&self, o: &Self) -> bool {
        self.spectral == o.spectral
            && self.bands == o.bands
            && self.beats == o.beats
            && self.bpm == o.bpm
            && self.cues == o.cues
    }
}

/// Decks that have a track loaded, in deck order.
pub fn loaded(players: &[PlayerInfo]) -> Vec<&PlayerInfo> {
    let mut v: Vec<&PlayerInfo> = players
        .iter()
        .filter(|p| p.track_id > 0 || !p.track_title.is_empty())
        .collect();
    v.sort_by_key(|p| p.device_number);
    v
}

#[component]
pub fn WaveStack(players: Vec<PlayerInfo>, feed: Feed) -> Element {
    let platform = use_context::<Platform>();
    let mut bars = use_signal({
        let store = platform.store.clone();
        move || {
            store
                .get_json::<u32>(BARS_KEY)
                .ok()
                .flatten()
                .filter(|b| grid::ZOOM_BARS.contains(b))
                .unwrap_or(DEFAULT_BARS)
        }
    });
    let step = use_callback({
        let store = platform.store.clone();
        move |out: bool| {
            let next = grid::step_bars(*bars.peek(), out);
            if next != *bars.peek() {
                bars.set(next);
                let _ = store.set_json(BARS_KEY, &next);
            }
        }
    });

    let decks = loaded(&players);
    if decks.is_empty() {
        return rsx! {};
    }
    rsx! {
        section {
            class: "card stk",
            "aria-label": "Waveform stack",
            onwheel: move |e| {
                if e.modifiers().ctrl() {
                    e.prevent_default();
                    let dy = e.delta().strip_units().y;
                    if dy != 0.0 {
                        step.call(dy > 0.0);
                    }
                }
            },
            div { class: "hd",
                span { class: "sh2", "WAVEFORM STACK" }
                span { class: "mut mono", style: "font-size:12px", "{bars()} BARS" }
                span { class: "sp" }
                button { class: "btn sm", title: "Zoom in (Ctrl-scroll up)", "aria-label": "Zoom the stack in", disabled: bars() == grid::ZOOM_BARS[0], onclick: move |_| step.call(false), "+" }
                button { class: "btn sm", title: "Zoom out (Ctrl-scroll down)", "aria-label": "Zoom the stack out", disabled: bars() == grid::ZOOM_BARS[grid::ZOOM_BARS.len() - 1], onclick: move |_| step.call(true), "−" }
            }
            div { class: "skrows",
                for p in decks {
                    StackRow {
                        key: "{p.device_number}-{p.track_id}-{p.external}-{p.analysis_url}",
                        player: p.clone(),
                        feed,
                        bars: bars(),
                    }
                }
                div { class: "skcl", "aria-hidden": "true" }
            }
        }
    }
}

#[component]
fn StackRow(player: PlayerInfo, feed: Feed, bars: u32) -> Element {
    let client = use_context::<crate::shell::ClientCtx>().0;
    let LookCtx {
        colors,
        daemon_wave,
        daemon_overview,
        ..
    } = use_context::<LookCtx>();
    let p = player;
    let spectral = colors().spectral(&daemon_wave());
    let half = daemon_overview() == "half";
    let (id, external, url) = (p.track_id, p.external, p.analysis_url.clone());
    let live_bpm = p.bpm;

    let raw = use_resource(use_reactive!(|(id, external, url, spectral)| {
        let c = client.clone();
        async move {
            let empty = || Shared(Rc::new(Vec::new()));
            if external {
                if url.is_empty() {
                    return None;
                }
                // The daemon answers 404 until it has read the deck's media.
                for _ in 0..40 {
                    if let Ok(a) = c.ext_analysis(&url).await {
                        let beats = a
                            .beats
                            .iter()
                            .map(|&t| BeatInfo {
                                time_ms: t,
                                beat_in_bar: 2,
                            })
                            .collect();
                        return Some(Raw {
                            spectral: Shared(Rc::new(a.waveform)),
                            bands: empty(),
                            beats: Shared(Rc::new(beats)),
                            cues: a.cues,
                            bpm: a.bpm,
                        });
                    }
                    time::sleep(std::time::Duration::from_secs(3)).await;
                }
                return None;
            }
            if id <= 0 {
                return None;
            }
            let a = c.analysis(id).await.ok()?;
            let (sp, bd) = if spectral.is_some() {
                (
                    c.detail_waveform::<WaveEntrySpectral>(id, "spectral")
                        .await
                        .ok()?
                        .entries,
                    Vec::new(),
                )
            } else {
                (
                    Vec::new(),
                    c.detail_waveform::<WaveEntry3Band>(id, "3band")
                        .await
                        .ok()?
                        .entries,
                )
            };
            Some(Raw {
                spectral: Shared(Rc::new(sp)),
                bands: Shared(Rc::new(bd)),
                beats: Shared(Rc::new(a.beats)),
                cues: a.cues,
                bpm: a.bpm,
            })
        }
    }));

    let base_bpm = raw
        .read()
        .as_ref()
        .and_then(|r| r.as_ref())
        .map(|r| r.bpm)
        .filter(|b| *b > 20.0)
        .unwrap_or(live_bpm);
    // The strip is drawn once at the track's own tempo; the live tempo only stretches it.
    let pxps0 = grid::px_per_sec_bars(base_bpm, bars);
    let built = use_memo(use_reactive!(|(spectral, half, bars, base_bpm)| {
        let r = raw.read().clone().flatten()?;
        let pxps = grid::px_per_sec_bars(base_bpm, bars);
        let zoom = match spectral {
            Some(style) => grid::zoom_spectral(&r.spectral.0, pxps, style, half),
            // Only the CDJ's own data exists for a deck's USB, so the band style needs the library.
            None if !r.bands.0.is_empty() => grid::zoom_paths(&r.bands.0, pxps, half),
            None => grid::zoom_spectral(&r.spectral.0, pxps, Spectral::Blue, half),
        };
        let lines = grid::grid_lines(&r.beats.0, pxps);
        Some((
            Shared(Rc::new(zoom)),
            Shared(Rc::new(lines)),
            r.cues.clone(),
        ))
    }));

    let since = ((feed.now)() - (feed.live)().at_ms) / 1000.0;
    let pos = playhead::position_secs(p.beat_in_track, p.beat_age_ms, p.bpm, p.is_playing, since);
    let pxps_live = grid::px_per_sec_bars(live_bpm.max(20.0), bars);

    let key_label = if p.key.is_empty() {
        "—".to_string()
    } else {
        p.key.clone()
    };
    rsx! {
        div { class: if p.on_air { "sk on" } else { "sk" },
            div { class: "skl",
                span { class: "pn", "P{p.device_number}" }
                span { class: "mono", "{p.bpm:.1}" }
                span { class: "mono mut", "{key_label}" }
                if p.is_master { span { class: "skm", title: "Tempo master", "M" } }
            }
            div { class: "skw",
                match built() {
                    Some((zoom, lines, cues)) => {
                        let total = zoom.0.total_cols.max(1.0);
                        let secs = total / pxps0.max(0.001);
                        let width_pct = secs * pxps_live / grid::VIEW_COLS * 100.0;
                        let shift_pct = (pos * pxps_live - grid::VIEW_COLS / 2.0) / (secs * pxps_live) * 100.0;
                        rsx! {
                            div { class: "skin", style: "width:{width_pct:.3}%;transform:translateX({-shift_pct:.4}%)",
                                StackLayers { zoom: zoom.clone(), lines: lines.clone(), cues: cues.clone(), total, pxps0 }
                            }
                        }
                    }
                    None => rsx! { div { class: "skno", if external { "WAITING FOR THE DECK'S WAVEFORM…" } else { "LOADING…" } } },
                }
            }
        }
    }
}

/// The part of a row that does not change while the playhead moves.
#[component]
fn StackLayers(
    zoom: Shared<Zoom>,
    lines: Shared<Grid>,
    cues: Vec<CueInfo>,
    total: f64,
    pxps0: f64,
) -> Element {
    let z = &zoom.0;
    let g = &lines.0;
    rsx! {
        svg { view_box: "0 0 {total} 100", preserve_aspect_ratio: "none",
            path { d: "{g.beats}", fill: "none", class: "gbeat", stroke_width: "1", vector_effect: "non-scaling-stroke" }
            path { d: "{g.bars}", fill: "none", class: "gbar", stroke_width: "1.5", vector_effect: "non-scaling-stroke" }
            if z.colored.is_empty() {
                path { d: "{z.lo}", class: "wl" }
                path { d: "{z.mi}", class: "wm" }
                path { d: "{z.hi}", class: "wh" }
            }
            for (fill , d) in z.colored.iter() {
                path { key: "{fill}", d: "{d}", fill: "{fill}" }
            }
        }
        for c in cues.iter() {
            div { key: "{c.number}", class: "skcue", style: "left:{c.time_ms as f64 / 1000.0 * pxps0 / total * 100.0:.3}%" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(n: i64, id: i64, title: &str) -> PlayerInfo {
        PlayerInfo {
            device_number: n,
            track_id: id,
            track_title: title.into(),
            ..PlayerInfo::default()
        }
    }

    #[test]
    fn only_decks_with_a_track_get_a_row_in_deck_order() {
        let players = [
            deck(3, 0, "from a usb"),
            deck(1, 7, ""),
            deck(2, 0, ""),
            deck(4, 0, ""),
        ];
        let rows: Vec<i64> = loaded(&players).iter().map(|p| p.device_number).collect();
        assert_eq!(
            rows,
            vec![1, 3, 4]
                .into_iter()
                .filter(|n| *n != 4)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn an_empty_link_has_no_stack() {
        assert!(loaded(&[]).is_empty());
        assert!(loaded(&[deck(1, 0, "")]).is_empty());
    }
}
