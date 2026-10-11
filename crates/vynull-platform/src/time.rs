//! Timers and a millisecond clock that work on both targets.

use std::time::Duration;

/// Wait for `d` without blocking the UI thread.
#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep(d: Duration) {
    tokio::time::sleep(d).await;
}

/// Wait for `d` without blocking the UI thread.
#[cfg(target_arch = "wasm32")]
pub async fn sleep(d: Duration) {
    gloo_timers::future::sleep(d).await;
}

/// Monotonic milliseconds since an arbitrary start, for measuring elapsed time.
#[cfg(not(target_arch = "wasm32"))]
pub fn now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Monotonic milliseconds since page load, for measuring elapsed time.
#[cfg(target_arch = "wasm32")]
pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// Origin (`http://host:port`) the page was served from, so a web client served
/// by the daemon at `/app` can talk to it without asking. `None` natively.
#[cfg(not(target_arch = "wasm32"))]
pub fn same_origin_base() -> Option<String> {
    None
}

/// Origin (`http://host:port`) the page was served from, so a web client served
/// by the daemon at `/app` can talk to it without asking. `None` natively.
#[cfg(target_arch = "wasm32")]
pub fn same_origin_base() -> Option<String> {
    let origin = web_sys::window()?.location().origin().ok()?;
    origin.starts_with("http").then_some(origin)
}

/// Today's date in the user's time zone, `2026-10-11`.
#[cfg(not(target_arch = "wasm32"))]
pub fn local_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Today's date in the user's time zone, `2026-10-11`.
#[cfg(target_arch = "wasm32")]
pub fn local_date() -> String {
    let d = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date()
    )
}

/// `21:48:02` in the user's time zone, for a Unix time in milliseconds.
#[cfg(not(target_arch = "wasm32"))]
pub fn clock_label(unix_ms: i64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_millis_opt(unix_ms)
        .single()
        .map_or_else(
            || "--:--:--".to_string(),
            |d| d.format("%H:%M:%S").to_string(),
        )
}

/// `21:48:02` in the user's time zone, for a Unix time in milliseconds.
#[cfg(target_arch = "wasm32")]
pub fn clock_label(unix_ms: i64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(unix_ms as f64));
    format!(
        "{:02}:{:02}:{:02}",
        d.get_hours(),
        d.get_minutes(),
        d.get_seconds()
    )
}
