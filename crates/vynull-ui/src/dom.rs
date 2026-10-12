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

/// `(list top, list bottom, content top, scrollTop)` in px for a scrolling list and its content element.
pub async fn list_geometry(list: &str, content: &str) -> Option<(f64, f64, f64, f64)> {
    let (l, c) = (
        serde_json::to_string(list).ok()?,
        serde_json::to_string(content).ok()?,
    );
    let mut ev = document::eval(&format!(
        "const l = document.querySelector({l}), c = document.querySelector({c}); if (l && c) {{ const a = l.getBoundingClientRect(), b = c.getBoundingClientRect(); dioxus.send([a.top, a.bottom, b.top, l.scrollTop]); }} else {{ dioxus.send([]); }}"
    ));
    let v = ev.recv::<Vec<f64>>().await.ok()?;
    (v.len() == 4).then(|| (v[0], v[1], v[2], v[3]))
}

/// Scroll a list by `dy` px.
pub async fn scroll_list_by(list: &str, dy: i32) {
    if let Ok(l) = serde_json::to_string(list) {
        let _ = document::eval(&format!(
            "const l = document.querySelector({l}); if (l) l.scrollTop += {dy};"
        ))
        .await;
    }
}

/// Put a scrolling list back at its top. Setting a signal that mirrors the scroll position is not
/// enough: the element keeps its own `scrollTop`, so the rows drawn for the top would sit outside
/// the part that is shown, and the list looks empty until the next scroll event.
pub async fn scroll_list_to_top(list: &str) {
    if let Ok(l) = serde_json::to_string(list) {
        let _ = document::eval(&format!(
            "const l = document.querySelector({l}); if (l) l.scrollTop = 0;"
        ))
        .await;
    }
}
