//! The phrase strip: colored segments for the intro, ups, downs, choruses and
//! outro of a track, with a microphone on the ones that sound sung.

use dioxus::prelude::*;
use vynull_api::PhraseInfo;
use vynull_core::phrase;

use crate::shell::LookCtx;

/// `span_ms` is the time that the strip's full width stands for. The
/// full-track strip passes the track length; the drawer's zoom strip passes
/// the width of the whole zoom canvas in milliseconds, so segments line up with
/// the waveform above.
#[component]
pub fn PhraseStrip(phrases: Vec<PhraseInfo>, span_ms: f64, class: String) -> Element {
    let vocal = use_context::<LookCtx>().vocal;
    if phrases.is_empty() || span_ms <= 0.0 {
        return rsx! {};
    }
    let level = vocal();
    rsx! {
        div { class: "phs {class}", "aria-label": "Song phrases",
            for (i , p) in phrases.iter().enumerate() {
                {
                    let left = p.start_ms as f64 / span_ms * 100.0;
                    let width = (p.end_ms - p.start_ms).max(0) as f64 / span_ms * 100.0;
                    let sung = level.marks(p.vocal_score);
                    let title = format!("{} · vocal {:.2} · {} – {}", p.name, p.vocal_score, phrase::clock(p.start_ms), phrase::clock(p.end_ms));
                    let color = phrase::kind_color(p.kind);
                    rsx! {
                        div {
                            key: "{i}",
                            class: if sung { "ph1 vc" } else { "ph1" },
                            style: "left:{left:.3}%;width:{width:.3}%;background:{color}",
                            title: "{title}",
                            span { class: "pnm", "{p.name}" }
                            if sung {
                                svg { class: "mic", view_box: "0 0 24 24", width: "10", height: "10", fill: "currentColor", "aria-label": "vocal",
                                    path { d: "M12 14a3 3 0 0 0 3-3V5a3 3 0 0 0-6 0v6a3 3 0 0 0 3 3zm5-3a5 5 0 0 1-10 0H5a7 7 0 0 0 6 6.9V21h2v-3.1A7 7 0 0 0 19 11z" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
