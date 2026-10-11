//! Overlay screen: edit the daemon's now-playing overlay and see it on a 16:9
//! stage built from the live decks. Every change is sent to the daemon, which
//! sanitises it and answers with the config it kept; the stage shows that answer.

use std::time::Duration;

use dioxus::prelude::*;
use vynull_api::{HistoryItem, OverlayConfig, PlayerInfo};
use vynull_core::{overlay, waveform};
use vynull_platform::time;

use crate::live::Feed;
use crate::shell::ClientCtx;

#[component]
pub fn OverlayScreen(feed: Feed, base: String) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut cfg = use_signal(|| None::<OverlayConfig>);
    let mut error = use_signal(String::new);
    let mut copied = use_signal(|| false);
    let mut history = use_signal(Vec::<HistoryItem>::new);
    // Counts saves, so that a slow answer to an old click cannot undo a newer one.
    let mut latest = use_signal(|| 0u64);
    // The header field keeps what is being typed: this screen redraws several times a second
    // (live decks), and a field bound straight to the saved value would be reset each time.
    let mut label_draft = use_signal(|| None::<String>);

    let c = client.clone();
    use_future(move || {
        let c = c.clone();
        async move {
            match c.overlay_config().await {
                Ok(v) => cfg.set(Some(v)),
                Err(e) => error.set(format!("Could not read the overlay settings: {e}")),
            }
        }
    });
    let c = client.clone();
    use_future(move || {
        let c = c.clone();
        async move {
            loop {
                if let Ok(h) = c.history().await {
                    history.set(h);
                }
                time::sleep(Duration::from_secs(5)).await;
            }
        }
    });

    // Send the whole config; show what the daemon kept.
    let save = {
        let c = client.clone();
        use_callback(move |next: OverlayConfig| {
            // The stage follows the click at once; the daemon's answer then replaces it.
            cfg.set(Some(next.clone()));
            let mine = latest() + 1;
            latest.set(mine);
            let c = c.clone();
            spawn(async move {
                let res = c.set_overlay_config(&next).await;
                if latest() != mine {
                    return; // a newer change is on its way: its answer is the one to show
                }
                match res {
                    Ok(kept) => {
                        error.set(String::new());
                        cfg.set(Some(kept));
                    }
                    Err(e) => error.set(format!("The daemon did not accept the change: {e}")),
                }
            });
        })
    };

    let url = overlay::url(&base);
    let copy = {
        let url = url.clone();
        move |_| {
            if let Ok(js) = serde_json::to_string(&url) {
                document::eval(&format!(
                    "navigator.clipboard && navigator.clipboard.writeText({js})"
                ));
            }
            copied.set(true);
            spawn(async move {
                time::sleep(Duration::from_secs(2)).await;
                copied.set(false);
            });
        }
    };

    let Some(c) = cfg() else {
        return rsx! {
            div { class: "pgo", main { class: "pg",
                if error().is_empty() { span { class: "mut", "Loading…" } } else { div { class: "errbox", "{error()}" } }
            } }
        };
    };

    let live = (feed.live)();
    let np = live.nowplaying.clone();
    let full = c.waveform_style == "full";
    let values: Vec<f64> = waveform::demo_entries(400)
        .iter()
        .map(|e| e.bass as f64 / 255.0)
        .collect();
    let wave = overlay::wave_path(&values, full);
    let label = c.label.trim().to_string();
    let accent = if c.accent.is_empty() {
        "#FFC15A".to_string()
    } else {
        c.accent.clone()
    };
    let stage_style = format!("--ac:{accent}");
    // Newest first, as many as the card lists.
    let hist_view: Vec<(usize, String, String)> = {
        let h = history.read();
        let mut v: Vec<&HistoryItem> = h.iter().collect();
        v.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        v.into_iter()
            .take(c.history_count.max(1) as usize)
            .enumerate()
            .map(|(i, h)| {
                (
                    i + 1,
                    h.title.clone(),
                    h.started_at.get(11..16).unwrap_or("").to_string(),
                )
            })
            .collect()
    };
    let deck_rows: Vec<(i64, String, String)> = live.players.iter().map(deck_row).collect();

    // Reads the current config when called, so it holds no borrow and can sit in every handler.
    let set = move |f: fn(&mut OverlayConfig)| {
        let mut n = cfg().unwrap_or_default();
        f(&mut n);
        save.call(n);
    };
    let toggle_row = |name: &'static str,
                      sub: Option<&'static str>,
                      on: bool,
                      f: fn(&mut OverlayConfig)| {
        rsx! {
            div { class: "sr",
                div { class: "sl", b { "{name}" } if let Some(s) = sub { span { "{s}" } } }
                button { class: if on { "tg on" } else { "tg" }, "aria-label": "{name}", onclick: move |_| set(f), i {} }
            }
        }
    };

    rsx! {
        div { class: "pgo",
            main { class: "pg",
                div { class: "hd",
                    span { class: "kick", "OVERLAY" }
                    h1 { "Stream overlay" }
                    span { class: "mut", "Now-playing card for OBS. Changes apply live." }
                    span { class: "sp" }
                    div { class: "url",
                        input { class: "in", r#type: "text", readonly: true, value: "{url}", "aria-label": "Overlay address" }
                        button { class: if copied() { "btn on" } else { "btn" }, onclick: copy, if copied() { "COPIED" } else { "COPY" } }
                    }
                }
                if !error().is_empty() { div { class: "errbox", "{error()}" } }
                div { class: "lay",
                    div { class: "stw",
                        div { class: "stage", style: "{stage_style}",
                            span { class: "stg-l", "1920 x 1080  TRANSPARENT BACKGROUND" }
                            if c.waveform_band {
                                svg { class: "band", style: "{overlay::band_style(&c)}", view_box: "0 0 400 100", preserve_aspect_ratio: "none", "aria-hidden": "true",
                                    defs { clipPath { id: "cb1", rect { x: "0", y: "0", width: "150", height: "100" } } }
                                    path { d: "{wave}", fill: "#fff", opacity: ".22" }
                                    path { d: "{wave}", fill: "var(--ac)", clip_path: "url(#cb1)" }
                                }
                            }
                            if c.show_nowplaying {
                                div { class: "np", style: "{overlay::card_style(&c)}",
                                    div { class: "row",
                                        div { class: "deck",
                                            if c.style == "cover" {
                                                div { class: "cov", style: "background:linear-gradient(135deg,{accent},#2A313C 70%)" }
                                            } else {
                                                div { class: "vinyl",
                                                    div { class: "lbl", style: "background:linear-gradient(135deg,{accent},#2A313C 70%)" }
                                                    div { class: "hole" }
                                                }
                                            }
                                        }
                                        div { class: "meta",
                                            if !label.is_empty() {
                                                div { class: "npl",
                                                    span { class: "eq", i { style: "height:60%" } i { style: "height:100%" } i { style: "height:40%" } i { style: "height:80%" } }
                                                    span { "{label}" }
                                                }
                                            }
                                            div { class: "ttl", if np.playing { "{np.title}" } else { "Nothing playing" } }
                                            div { class: "art", if np.playing { "{np.artist}" } else { "The card is empty until a deck is on air." } }
                                            if c.show_meta && np.playing {
                                                div { class: "sub", b { "{np.bpm:.2}" } " BPM   " b { "{np.key}" } " KEY" }
                                            }
                                        }
                                    }
                                    if c.show_waveform && !c.waveform_band {
                                        svg { class: "wv", view_box: "0 0 400 100", preserve_aspect_ratio: "none", "aria-hidden": "true",
                                            defs { clipPath { id: "cb2", rect { x: "0", y: "0", width: "150", height: "100" } } }
                                            path { d: "{wave}", fill: "#fff", opacity: ".22" }
                                            path { d: "{wave}", fill: "var(--ac)", clip_path: "url(#cb2)" }
                                        }
                                    }
                                    if c.show_decks {
                                        div { class: "sec",
                                            span { class: "h", "DECKS" }
                                            for (n , t , s) in deck_rows.iter() { div { key: "{n}", class: "l", b { "{n}" } span { "{t}" } em { "{s}" } } }
                                        }
                                    }
                                    if c.show_history {
                                        div { class: "sec",
                                            span { class: "h", "RECENTLY PLAYED" }
                                            if hist_view.is_empty() { div { class: "l", span { "Nothing played yet." } } }
                                            for (n , t , s) in hist_view.iter() { div { key: "{n}", class: "l", b { "{n}" } span { "{t}" } em { "{s}" } } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    aside { class: "card cfg",
                        div { class: "grp", "CARD" }
                        div { class: "sr", div { class: "pos",
                            for (v , name) in overlay::POSITIONS {
                                button { key: "{v}", class: if c.position == v { "on" } else { "" },
                                    onclick: move |_| { let mut n = cfg().unwrap_or_default(); n.position = v.to_string(); save.call(n); }, "{name}" }
                            }
                        } }
                        div { class: "sr",
                            div { class: "sl", b { "Style" } }
                            div { class: "seg",
                                button { class: if c.style != "cover" { "on" } else { "" }, onclick: move |_| set(|n| n.style = "vinyl".into()), "VINYL" }
                                button { class: if c.style == "cover" { "on" } else { "" }, onclick: move |_| set(|n| n.style = "cover".into()), "COVER" }
                            }
                        }
                        div { class: "sr",
                            div { class: "sl", b { "Accent color" } span { class: "mono", "{c.accent}" } }
                            div { class: "sw",
                                for (name , hex) in overlay::ACCENTS {
                                    button { key: "{hex}", class: if overlay::same_color(&c.accent, hex) { "on" } else { "" }, style: "background:{hex}", "aria-label": "{name}", title: "{name}",
                                        onclick: move |_| { let mut n = cfg().unwrap_or_default(); n.accent = hex.to_string(); save.call(n); } }
                                }
                            }
                        }
                        div { class: "sr",
                            div { class: "sl", b { "Card width" } }
                            div { class: "stp",
                                button { "aria-label": "Narrower", onclick: move |_| set(|n| n.width = overlay::step(n.width, -1, overlay::WIDTH)), "-" }
                                span { "{c.width} px" }
                                button { "aria-label": "Wider", onclick: move |_| set(|n| n.width = overlay::step(n.width, 1, overlay::WIDTH)), "+" }
                            }
                        }
                        div { class: "sr",
                            div { class: "sl", b { "Header text" } span { "Empty hides the header." } }
                            input { class: "in", style: "flex:1 1 160px", r#type: "text", maxlength: "40", placeholder: "Now Playing", "aria-label": "Header text",
                                value: "{label_draft().unwrap_or_else(|| c.label.clone())}",
                                oninput: move |e| label_draft.set(Some(e.value())),
                                onchange: move |e| {
                                    let mut n = cfg().unwrap_or_default();
                                    n.label = e.value();
                                    label_draft.set(None);
                                    save.call(n);
                                } }
                        }
                        div { class: "grp", "SECTIONS" }
                        {toggle_row("Now-playing card", None, c.show_nowplaying, |n| n.show_nowplaying = !n.show_nowplaying)}
                        {toggle_row("BPM and key line", None, c.show_meta, |n| n.show_meta = !n.show_meta)}
                        {toggle_row("Card waveform", None, c.show_waveform, |n| n.show_waveform = !n.show_waveform)}
                        {toggle_row("Deck by deck", Some("Which deck plays what."), c.show_decks, |n| n.show_decks = !n.show_decks)}
                        {toggle_row("Recently played", None, c.show_history, |n| n.show_history = !n.show_history)}
                        div { class: "sr",
                            div { class: "sl", b { "Tracks listed" } }
                            div { class: "stp",
                                button { "aria-label": "Fewer", onclick: move |_| set(|n| n.history_count = overlay::step(n.history_count, -1, overlay::HISTORY)), "-" }
                                span { "{c.history_count}" }
                                button { "aria-label": "More", onclick: move |_| set(|n| n.history_count = overlay::step(n.history_count, 1, overlay::HISTORY)), "+" }
                            }
                        }
                        div { class: "grp", "WAVEFORM" }
                        div { class: "sr",
                            div { class: "sl", b { "Shape" } span { "Applies to every waveform." } }
                            div { class: "seg",
                                button { class: if !full { "on" } else { "" }, onclick: move |_| set(|n| n.waveform_style = "half".into()), "HALF" }
                                button { class: if full { "on" } else { "" }, onclick: move |_| set(|n| n.waveform_style = "full".into()), "FULL" }
                            }
                        }
                        {toggle_row("Full-width band", Some("Replaces the card waveform."), c.waveform_band, |n| n.waveform_band = !n.waveform_band)}
                        div { class: "sr",
                            div { class: "sl", b { "Band height" } }
                            div { class: "stp",
                                button { "aria-label": "Shorter", onclick: move |_| set(|n| n.waveform_height = overlay::step(n.waveform_height, -1, overlay::BAND)), "-" }
                                span { "{c.waveform_height} px" }
                                button { "aria-label": "Taller", onclick: move |_| set(|n| n.waveform_height = overlay::step(n.waveform_height, 1, overlay::BAND)), "+" }
                            }
                        }
                        div { class: "nt", "OBS: ADD A BROWSER SOURCE AT THE ADDRESS ABOVE. KEEP THE BACKGROUND TRANSPARENT." }
                    }
                }
            }
        }
    }
}

fn deck_row(p: &PlayerInfo) -> (i64, String, String) {
    let title = if p.track_title.is_empty() {
        "-".to_string()
    } else {
        p.track_title.clone()
    };
    let state = if p.play_state_name.is_empty() {
        "EMPTY".to_string()
    } else {
        p.play_state_name.to_uppercase()
    };
    (p.device_number, title, state)
}
