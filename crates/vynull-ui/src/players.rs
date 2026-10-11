//! Players screen: mixer on-air state, one card per deck, recent plays and the
//! stream-overlay preview. Mirrors the mockup, except the mixer strip: the API
//! has no fader, crossfader or master level, so only on-air state is shown.

use dioxus::prelude::*;
use vynull_api::{HistoryItem, MixerInfo, PlayerInfo};
use vynull_core::{key, playhead, waveform};

use crate::live::Feed;
use crate::phrases::PhraseStrip;
use crate::shell::{ClientCtx, LookCtx, Tab};
use vynull_api::WaveEntrySpectral;
use vynull_platform::time;

#[component]
pub fn Players(feed: Feed, base: String, goto: EventHandler<Tab>) -> Element {
    let client = use_context::<ClientCtx>().0;
    let polled = (feed.polled)();
    let live = (feed.live)();

    let mixers = polled.peers.iter().filter(|p| p.mixer.is_some()).count();
    let others = polled.peers.len() - mixers;

    // Recent plays: refetch when a deck loads a different track.
    let loaded = use_memo(move || {
        (feed.live)()
            .players
            .iter()
            .map(|p| (p.device_number, p.track_id))
            .collect::<Vec<_>>()
    });
    let c = client.clone();
    let history = use_resource(move || {
        let _ = loaded();
        let c = c.clone();
        async move { c.history().await.unwrap_or_default() }
    });

    let master_key = live
        .players
        .iter()
        .find(|p| p.is_master)
        .map(|p| p.key.clone())
        .unwrap_or_default();
    let mixer = polled.peers.iter().find_map(|p| p.mixer.clone());

    rsx! {
        div { class: "hd",
            span { class: "kick", "PLAYERS" }
            h1 { "Live link" }
            span { class: "mut", "{others} players and {mixers} mixer on the Pro DJ Link network" }
        }

        section { class: "card", "aria-label": "Mixer",
            div { class: "hd",
                span { class: "sh2", "MIXER" }
                if mixer.is_some() {
                    span { class: "chip mono", i { class: "dot" } "ONLINE" }
                } else {
                    span { class: "chip mono", i { class: "dot off" } "NO MIXER" }
                }
            }
            if let Some(m) = mixer {
                MixerStrip { mixer: m }
            } else {
                span { class: "mut", "No DJM mixer is on the link." }
            }
        }

        section { class: "pcs",
            if live.players.is_empty() {
                article { class: "card",
                    div { class: "empty", span { "NO PLAYERS ON THE LINK" } }
                }
            }
            for p in live.players.iter() {
                Deck {
                    key: "{p.device_number}",
                    player: p.clone(),
                    master_key: master_key.clone(),
                    feed,
                    goto,
                }
            }
        }

        section { class: "two",
            div { class: "card",
                div { class: "sh2", "RECENT PLAYS" }
                match &*history.read() {
                    Some(items) if !items.is_empty() => {
                        // Newest first, whatever order the daemon uses.
                        let mut recent: Vec<&HistoryItem> = items.iter().collect();
                        recent.sort_by(|a, b| b.started_at.cmp(&a.started_at));
                        rsx! {
                            for h in recent.into_iter().take(5) {
                                HistoryRow { item: h.clone() }
                            }
                        }
                    }
                    Some(_) => rsx! { span { class: "mut", "Nothing played yet." } },
                    None => rsx! { span { class: "mut", "Loading…" } },
                }
            }
            div { class: "card",
                div { class: "hd",
                    span { class: "sh2", "STREAM OVERLAY" }
                    span { class: "sp" }
                    a { class: "chip mono", href: "{base}/overlay", target: "_blank", "/overlay" }
                }
                OverlayPreview { feed, base: base.clone() }
                span { class: "mut",
                    "Add this page as a browser source in the streaming software. The overlay shows the loudest deck."
                }
            }
        }
    }
}

#[component]
fn MixerStrip(mixer: MixerInfo) -> Element {
    let known = mixer.channel_state_known;
    rsx! {
        div { class: "mx",
            for (i, on) in mixer.channel_on_air.iter().enumerate() {
                div { key: "{i}", class: "m1",
                    div { class: "t",
                        span { "CH {i + 1}" }
                        b { if !known { "?" } else if *on { "ON AIR" } else { "OFF" } }
                    }
                    div { class: "tr",
                        i { style: if known && *on { "width:100%;background:#5FD38D" } else { "width:0;background:#5FD38D" } }
                    }
                }
            }
            div { class: "m1",
                div { class: "t", span { "MASTER" } b { "P{mixer.master_device} · {mixer.master_bpm:.1}" } }
            }
        }
    }
}

/// What the overview needs besides the live counters; fetched once per track.
#[derive(Clone, PartialEq)]
struct Overview {
    paths: waveform::OverviewPaths,
    cue_ms: Vec<i64>,
}

fn art_style(n: i64) -> String {
    let h = (n * 47 + 7) % 360;
    format!(
        "background:linear-gradient(135deg,hsl({h},42%,34%),hsl({},42%,16%))",
        (h + 50) % 360
    )
}

#[component]
fn Deck(player: PlayerInfo, master_key: String, feed: Feed, goto: EventHandler<Tab>) -> Element {
    let client = use_context::<ClientCtx>().0;
    let p = player;
    let empty = p.track_id <= 0 && p.track_title.is_empty();

    let (id, external) = (p.track_id, p.external);
    let c = client.clone();
    let LookCtx {
        colors,
        daemon_wave,
        daemon_overview,
        ..
    } = use_context::<LookCtx>();
    let spectral_style = colors().spectral(&daemon_wave());
    let half = daemon_overview() == "half";
    // A track from the deck's own USB or SD has no library id: its waveform and cues come from
    // `analysis_url`, which answers 404 until the daemon has read them over NFS, so ask again.
    let analysis_url = p.analysis_url.clone();
    let overview = use_resource(use_reactive!(|(
        id,
        external,
        spectral_style,
        half,
        analysis_url,
    )| {
        let c = c.clone();
        async move {
            if external {
                if analysis_url.is_empty() {
                    return None;
                }
                for _ in 0..40 {
                    if let Ok(a) = c.ext_analysis(&analysis_url).await {
                        if a.waveform.is_empty() {
                            return None;
                        }
                        // Only the CDJ's own RGB data exists here, so the three-band style falls back to blue.
                        let style = spectral_style.unwrap_or(waveform::Spectral::Blue);
                        return Some(Overview {
                            paths: waveform::spectral_overview(&a.waveform, style, half),
                            cue_ms: a.cues.iter().map(|q| q.time_ms).collect(),
                        });
                    }
                    time::sleep(std::time::Duration::from_secs(3)).await;
                }
                return None;
            }
            if id <= 0 {
                return None;
            }
            let paths = match spectral_style {
                Some(style) => {
                    let w = c
                        .detail_waveform::<WaveEntrySpectral>(id, "spectral")
                        .await
                        .ok()?;
                    waveform::spectral_overview(&w.entries, style, half)
                }
                None => waveform::overview_paths(&c.color_preview(id).await.ok()?.entries, half),
            };
            let cues = c.track_cues(id).await.unwrap_or_default();
            Some(Overview {
                paths,
                cue_ms: cues.iter().map(|q| q.time_ms).collect(),
            })
        }
    }));

    let c = client.clone();
    let phrases = use_resource(use_reactive!(|(id, external)| {
        let c = c.clone();
        async move {
            if id <= 0 || external {
                return Vec::new();
            }
            c.phrases(id).await.unwrap_or_default()
        }
    }));

    let since = ((feed.now)() - (feed.live)().at_ms) / 1000.0;
    let pos = playhead::position_secs(p.beat_in_track, p.beat_age_ms, p.bpm, p.is_playing, since);
    let pct = playhead::percent(pos, p.duration_ms);
    let (beat, bar) = playhead::beat_labels(p.beat_in_bar, p.beat_in_track);
    let remain = playhead::remaining_label(pos, p.duration_ms);

    let state = if p.play_state_name.is_empty() {
        "—".to_string()
    } else {
        p.play_state_name.to_uppercase()
    };
    let state_cls = if p.is_playing {
        "bdg pl"
    } else if state.contains("CUE") {
        "bdg cu"
    } else {
        "bdg"
    };
    let src = if p.source.is_empty() {
        "NONE".to_string()
    } else {
        p.source.to_uppercase()
    };
    let key_cls = if !master_key.is_empty() && key::compatible(&p.key, &master_key) {
        "key ok"
    } else {
        "key"
    };
    let art = if p.artwork_url.is_empty() {
        None
    } else {
        Some(client.url(&p.artwork_url))
    };

    rsx! {
        article { class: "card",
            div { class: "pch",
                span { class: "pn", "P{p.device_number}" }
                span { class: "mono mut", "{p.name}" }
                span { class: "{state_cls}", "{state}" }
                span { class: if p.is_master { "bdg ms" } else { "bdg off" }, title: if p.is_master { "This deck is the tempo master" } else { "Not the tempo master" }, "MASTER" }
                span { class: if p.is_sync { "bdg sy" } else { "bdg off" }, title: if p.is_sync { "Sync is on" } else { "Sync is off" }, "SYNC" }
                if p.on_air { span { class: "bdg pl", "ON AIR" } }
                span { class: "sp" }
                if p.external { span { class: "bdg ex", "EXT METADATA" } }
                span { class: "bdg", "{src}" }
            }
            if empty {
                div { class: "empty",
                    span { "NO TRACK LOADED" }
                    button { class: "btn on", onclick: move |_| goto.call(Tab::Library), "LOAD TRACK" }
                }
            } else {
                div { class: "pb",
                    div { class: "art", style: "{art_style(p.device_number)}",
                        if let Some(url) = art { img { src: "{url}", alt: "" } }
                    }
                    div { class: "pt",
                        div { class: "ttl", "{p.track_title}" }
                        div { class: "mut", "{p.artist}" }
                    }
                    div {
                        div { class: "big", "{p.bpm:.1}" }
                        div { class: "mono mut", style: "text-align:right", "{p.pitch_pct:+.1} %" }
                    }
                }
                div { class: "kv4",
                    div { class: "kv", span { "KEY" } b { span { class: "{key_cls}", style: "display:inline-block;font-size:16px;letter-spacing:0;color:inherit", "{p.key}" } } }
                    div { class: "kv", span { "BEAT" } b { "{beat}" } }
                    div { class: "kv", span { "BAR" } b { "{bar}" } }
                    div { class: "kv", span { "REMAIN" } b { "{remain}" } }
                }
                div { class: "ov",
                    match &*overview.read() {
                        Some(Some(o)) => {
                            let cues: Vec<f64> = o.cue_ms.iter().map(|&t| t as f64 / p.duration_ms.max(1) as f64).collect();
                            rsx! {
                                svg { view_box: "0 0 400 100", preserve_aspect_ratio: "none",
                                    if o.paths.colored.is_empty() {
                                        path { d: "{o.paths.lo}", class: "wl" }
                                        path { d: "{o.paths.mi}", class: "wm" }
                                        path { d: "{o.paths.hi}", class: "wh" }
                                    }
                                    for (fill , d) in o.paths.colored.iter() {
                                        path { key: "{fill}", d: "{d}", fill: "{fill}" }
                                    }
                                    path { d: "{waveform::cue_marks(&cues)}", fill: "#FF6B6B" }
                                }
                            }
                        }
                        _ => rsx! {
                            div { class: "empty", style: "min-height:0;height:100%;border:0",
                                if external { "WAITING FOR THE DECK'S WAVEFORM…" } else { "NO WAVEFORM" }
                            }
                        },
                    }
                    div { class: "past", style: "width:{pct}%" }
                    div { class: "phd", style: "left:{pct}%" }
                }
                PhraseStrip { phrases: phrases.read().clone().unwrap_or_default(), span_ms: p.duration_ms as f64, class: "full" }
                div { class: "acts",
                    button { class: "btn", onclick: move |_| goto.call(Tab::Library), "LOAD TRACK" }
                    button { class: "btn", onclick: move |_| goto.call(Tab::Library), "TRACK INFO" }
                }
            }
        }
    }
}

#[component]
fn HistoryRow(item: HistoryItem) -> Element {
    // RFC 3339 "2026-10-11T21:48:03-04:00": show HH:MM.
    let at = item.started_at.get(11..16).unwrap_or("");
    rsx! {
        div { class: "row",
            span { class: "pn", style: "font-size:16px", "P{item.device_number}" }
            span { class: "ttl", "{item.title}" }
            span { class: "mono mut", "{item.bpm:.1}" }
            span { class: "mono mut", "{at}" }
        }
    }
}

#[component]
fn OverlayPreview(feed: Feed, base: String) -> Element {
    let np = (feed.live)().nowplaying;
    let client = use_context::<ClientCtx>().0;
    let art = if np.artwork_url.is_empty() {
        None
    } else {
        Some(client.url(&np.artwork_url))
    };
    let _ = base;
    rsx! {
        div { class: "ovc",
            div { class: "art", style: "{art_style(np.device_number)}",
                if let Some(url) = art { img { src: "{url}", alt: "" } }
            }
            div { style: "min-width:0",
                if np.playing {
                    div { class: "ttl", "{np.title}" }
                    div { class: "mut", "{np.artist}" }
                    div { class: "mono", style: "margin-top:8px;color:#FFC15A", "{np.bpm:.1} BPM  {np.key}" }
                } else {
                    div { class: "ttl", "Nothing playing" }
                    div { class: "mut", "The overlay is empty until a deck is on air." }
                }
            }
        }
    }
}
