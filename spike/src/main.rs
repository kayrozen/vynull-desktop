//! Phase 0 UI spike: a 4,000-row virtual list and one moving waveform, with a
//! frame-rate meter. Query params: `?api=http://host:9443&scroll=1&wave=1&n=4000`.
//! Results are shown on screen and mirrored to `window.__spike` for scripts.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use vynull_api::{Client, TrackInfo, WaveEntry3Band, Waveform};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

const ROW_H: f64 = 28.0;
const VIEW_H: f64 = 600.0;
const OVERSCAN: usize = 6;
/// Waveform zoom: pixels per second of audio. 150 entries/s, so 2 entries per pixel.
const PX_PER_SEC: f64 = 75.0;
const WAVE_H: f64 = 120.0;

const CSS: &str = r#"
body{margin:0;background:#15161a;color:#d8dae0;font:13px system-ui,sans-serif}
#bar{display:flex;gap:16px;align-items:center;padding:8px 12px;background:#0e0f12;position:sticky;top:0;z-index:2}
#bar b{color:#ff7714}
button{background:#25272e;color:inherit;border:1px solid #3a3d46;padding:3px 10px;border-radius:4px}
button.on{border-color:#ff7714}
#list{height:600px;overflow-y:auto;position:relative;margin:12px;border:1px solid #2a2c33}
.row{height:28px;display:grid;grid-template-columns:60px 2fr 1.4fr 1.4fr 1fr 70px 50px 60px;align-items:center;padding:0 8px;box-sizing:border-box;border-bottom:1px solid #1d1f25;white-space:nowrap;overflow:hidden}
.row span{overflow:hidden;text-overflow:ellipsis;padding-right:8px}
#wave{width:1000px;height:120px;overflow:hidden;position:relative;margin:12px;background:#0b0c0f}
#wave canvas{position:absolute;top:0;left:500px;will-change:transform}
#head{position:absolute;left:500px;top:0;bottom:0;width:2px;background:#ff7714;z-index:1}
"#;

fn tint(color_id: i64) -> &'static str {
    match color_id {
        1 => "rgba(236,72,153,.18)",
        2 => "rgba(239,68,68,.18)",
        3 => "rgba(249,115,22,.18)",
        4 => "rgba(234,179,8,.18)",
        5 => "rgba(34,197,94,.18)",
        6 => "rgba(6,182,212,.18)",
        7 => "rgba(59,130,246,.18)",
        8 => "rgba(168,85,247,.18)",
        _ => "transparent",
    }
}

fn param(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{name}=")).map(str::to_string))
}

/// The library, padded by repetition up to `n` rows with distinct ids.
fn pad(tracks: Vec<TrackInfo>, n: usize) -> Vec<TrackInfo> {
    if tracks.is_empty() {
        return tracks;
    }
    (0..n)
        .map(|i| {
            let mut t = tracks[i % tracks.len()].clone();
            t.id = i as i64 + 1;
            t
        })
        .collect()
}

fn fmt_time(secs: i64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let api = param("api").unwrap_or_else(|| "http://127.0.0.1:9443".into());
    let n: usize = param("n").and_then(|v| v.parse().ok()).unwrap_or(4000);
    let mut auto_scroll = use_signal(|| param("scroll").as_deref() == Some("1"));
    let mut wave_on = use_signal(|| param("wave").as_deref() == Some("1"));
    let scroll_top = use_signal(|| 0.0_f64);
    let mut stats = use_signal(String::new);
    let mut pos_secs = use_signal(|| 0.0_f64);

    let api2 = api.clone();
    let data = use_resource(move || {
        let api = api2.clone();
        async move {
            let c = Client::new(api);
            let tracks = c.tracks().await.map_err(|e| e.to_string())?;
            // The longest track gives the most waveform to move.
            let pick = tracks.iter().max_by_key(|t| t.duration).map(|t| t.id);
            let wave = match pick {
                Some(id) => c.detail_waveform::<WaveEntry3Band>(id, "3band").await.ok(),
                None => None,
            };
            Ok::<_, String>((Rc::new(pad(tracks, n)), wave.map(Rc::new)))
        }
    });

    // One requestAnimationFrame loop drives the clock, the auto-scroller and the meter.
    use_hook(move || {
        let t0 = now();
        let frames: Rc<RefCell<Vec<f64>>> = Rc::default();
        let last_report = Rc::new(RefCell::new(t0));
        let cb: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::default();
        let cb2 = cb.clone();
        *cb.borrow_mut() = Some(Closure::new(move |ts: f64| {
            let mut f = frames.borrow_mut();
            f.push(ts);
            if wave_on() {
                pos_secs.set((ts - t0) / 1000.0);
            }
            if auto_scroll() {
                // 300 px/s down, wrapping: a steady scroll without a mouse.
                let doc = web_sys::window().unwrap().document().unwrap();
                if let Some(el) = doc.get_element_by_id("list") {
                    let span = (n as f64 * ROW_H - VIEW_H).max(1.0);
                    el.set_scroll_top((((ts - t0) / 1000.0 * 300.0) % span) as i32);
                }
            }
            if ts - *last_report.borrow() >= 2000.0 {
                *last_report.borrow_mut() = ts;
                let mut dts: Vec<f64> = f.windows(2).map(|w| w[1] - w[0]).collect();
                let fps = (f.len() as f64 - 1.0) / ((f[f.len() - 1] - f[0]) / 1000.0);
                dts.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let p95 = dts[(dts.len() as f64 * 0.95) as usize % dts.len().max(1)];
                let worst = dts.last().copied().unwrap_or(0.0);
                f.clear();
                stats.set(format!("{fps:.1} fps · p95 {p95:.1} ms · worst {worst:.1} ms"));
                publish(fps, p95, worst);
            }
            drop(f);
            request(cb2.borrow().as_ref().unwrap());
        }));
        request(cb.borrow().as_ref().unwrap());
        // Keep the closure alive for the page's lifetime.
        std::mem::forget(cb);
    });

    rsx! {
        document::Style { {CSS} }
        div { id: "bar",
            b { "Vynull spike" }
            span { "{n} rows" }
            button { class: if auto_scroll() { "on" }, onclick: move |_| auto_scroll.toggle(), "auto-scroll" }
            button { class: if wave_on() { "on" }, onclick: move |_| wave_on.toggle(), "waveform" }
            span { id: "stats", "{stats}" }
        }
        match &*data.read() {
            None => rsx! { p { style: "margin:12px", "loading {api}…" } },
            Some(Err(e)) => rsx! { p { style: "margin:12px;color:#f87171", "failed: {e}" } },
            Some(Ok((tracks, wave))) => rsx! {
                WaveView { wave: wave.clone(), pos_secs }
                List { tracks: tracks.clone(), scroll_top }
            },
        }
    }
}

#[component]
fn WaveView(wave: Option<Rc<Waveform<WaveEntry3Band>>>, pos_secs: Signal<f64>) -> Element {
    let entries = wave.as_ref().map_or(0, |w| w.entries.len());
    let width = (entries as f64 / 2.0).ceil().max(1.0);
    let dur = entries as f64 / 150.0;

    // Draw the whole track once; afterwards it only moves.
    let w2 = wave.clone();
    use_effect(move || {
        let Some(w) = &w2 else { return };
        let doc = web_sys::window().unwrap().document().unwrap();
        let Some(canvas) = doc.get_element_by_id("wavecanvas") else { return };
        let canvas: web_sys::HtmlCanvasElement = canvas.unchecked_into();
        let ctx: web_sys::CanvasRenderingContext2d =
            canvas.get_context("2d").unwrap().unwrap().unchecked_into();
        let mid = WAVE_H / 2.0;
        for (x, pair) in w.entries.chunks(2).enumerate() {
            let avg = |f: fn(&WaveEntry3Band) -> u8| {
                pair.iter().map(|e| f(e) as f64).sum::<f64>() / pair.len() as f64 / 255.0
            };
            for (v, color) in [
                (avg(|e| e.bass), "#2563eb"),
                (avg(|e| e.mid), "#f59e0b"),
                (avg(|e| e.high), "#f1f5f9"),
            ] {
                let h = v * mid;
                ctx.set_fill_style_str(color);
                ctx.fill_rect(x as f64, mid - h, 1.0, h * 2.0);
            }
        }
    });

    let t = if dur > 0.0 { pos_secs() % dur } else { 0.0 };
    let shift = -t * PX_PER_SEC;
    rsx! {
        div { id: "wave",
            div { id: "head" }
            canvas {
                id: "wavecanvas",
                width: "{width}",
                height: "{WAVE_H}",
                style: "transform:translateX({shift}px)",
            }
        }
    }
}

#[component]
fn List(tracks: Rc<Vec<TrackInfo>>, scroll_top: Signal<f64>) -> Element {
    let total = tracks.len();
    let first = ((scroll_top() / ROW_H) as usize).saturating_sub(OVERSCAN);
    let last = (first + (VIEW_H / ROW_H) as usize + 2 * OVERSCAN).min(total);
    rsx! {
        div {
            id: "list",
            onscroll: move |e| scroll_top.set(e.data().scroll_top() as f64),
            div { style: "height:{total as f64 * ROW_H}px;position:relative",
                div { style: "position:absolute;top:{first as f64 * ROW_H}px;left:0;right:0",
                    for t in &tracks[first..last] {
                        div { key: "{t.id}", class: "row", style: "background:{tint(t.color_id)}",
                            span { "{t.id}" }
                            span { "{t.title}" }
                            span { "{t.artist}" }
                            span { "{t.album}" }
                            span { "{t.genre}" }
                            span { "{t.bpm:.1}" }
                            span { "{t.key}" }
                            span { "{fmt_time(t.duration)}" }
                        }
                    }
                }
            }
        }
    }
}

fn now() -> f64 {
    web_sys::window().unwrap().performance().unwrap().now()
}

fn request(cb: &Closure<dyn FnMut(f64)>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(cb.as_ref().unchecked_ref())
        .unwrap();
}

/// Mirror the latest numbers to `window.__spike` for scripted runs.
fn publish(fps: f64, p95: f64, worst: f64) {
    let o = js_sys::Object::new();
    for (k, v) in [("fps", fps), ("p95_ms", p95), ("worst_ms", worst)] {
        let _ = js_sys::Reflect::set(&o, &k.into(), &v.into());
    }
    let _ = js_sys::Reflect::set(&web_sys::window().unwrap(), &"__spike".into(), &o);
}
