//! Overlay screen: edit the daemon's now-playing overlay and see it on a 16:9
//! stage built from the live decks. Every change is sent to the daemon, which
//! sanitises it and answers with the config it kept; the stage shows that answer.

use std::time::Duration;

use dioxus::prelude::*;
use vynull_api::OverlayConfig;
use vynull_core::overlay;
use vynull_platform::time;

use crate::dom;
use crate::shell::ClientCtx;

/// The daemon's own `/overlay` page, shown at the 1920 x 1080 it is made for and scaled to fit.
/// Its look is the daemon's, not ours, so what is shown is what a streaming program shows.
#[component]
pub fn OverlayFrame(base: String, id: String) -> Element {
    let mut scale = use_signal(|| 0.0_f64);
    let sel = format!("#{id}");
    use_future(move || {
        let sel = sel.clone();
        async move {
            loop {
                if let Some((_, w)) = dom::left_and_width(&sel).await {
                    let next = (w / 1920.0).max(0.0);
                    if (next - *scale.peek()).abs() > 0.0005 {
                        scale.set(next);
                    }
                }
                time::sleep(Duration::from_millis(400)).await;
            }
        }
    });
    let src = overlay::url(&base);
    rsx! {
        div { class: "ovf", id: "{id}",
            iframe {
                class: "ovf-in",
                src: "{src}",
                title: "Stream overlay, as the daemon draws it",
                tabindex: "-1",
                style: "transform:scale({scale})",
            }
        }
    }
}

#[component]
pub fn OverlayScreen(base: String) -> Element {
    let client = use_context::<ClientCtx>().0;
    let mut cfg = use_signal(|| None::<OverlayConfig>);
    let mut error = use_signal(String::new);
    let mut copied = use_signal(|| false);
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

    let full = c.waveform_style == "full";

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
                        div { class: "stage",
                            span { class: "stg-l", "1920 x 1080  TRANSPARENT BACKGROUND" }
                            OverlayFrame { base: base.clone(), id: "ovstage" }
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
