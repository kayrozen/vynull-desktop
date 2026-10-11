//! Small DOM measurements through `eval`, so the same code runs in a browser
//! and in a desktop webview. (`MountedData::get_client_rect` did not give a
//! usable rectangle for the elements measured here.)

use dioxus::prelude::*;

/// `(left, width)` in px of the first element matching `selector`.
pub async fn left_and_width(selector: &str) -> Option<(f64, f64)> {
    let sel = serde_json::to_string(selector).ok()?;
    let mut ev = document::eval(&format!(
        "const e = document.querySelector({sel}); if (e) {{ const r = e.getBoundingClientRect(); dioxus.send([r.left, r.width]); }} else {{ dioxus.send([0, 0]); }}"
    ));
    let v = ev.recv::<Vec<f64>>().await.ok()?;
    (v.len() == 2 && v[1] > 0.0).then(|| (v[0], v[1]))
}
