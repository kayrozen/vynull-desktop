//! The stream overlay settings: what the editor offers, and the CSS for the
//! preview card. The daemon sanitises whatever it is sent, but the editor keeps
//! to the same ranges so a click never asks for something invalid.

use vynull_api::OverlayConfig;

pub const POSITIONS: [(&str, &str); 6] = [
    ("top-left", "TOP LEFT"),
    ("top-center", "TOP CENTER"),
    ("top-right", "TOP RIGHT"),
    ("bottom-left", "BOTTOM LEFT"),
    ("bottom-center", "BOTTOM CENTER"),
    ("bottom-right", "BOTTOM RIGHT"),
];

/// `(name, hex)`.
pub const ACCENTS: [(&str, &str); 6] = [
    ("Orange", "#FF7714"),
    ("Amber", "#FFC15A"),
    ("Teal", "#3FD6C0"),
    ("Green", "#5FD38D"),
    ("Purple", "#B07CFF"),
    ("Pink", "#FF8AD0"),
];

pub const WIDTH: (i64, i64, i64) = (280, 1000, 20); // min, max, step
pub const HISTORY: (i64, i64, i64) = (1, 10, 1);
pub const BAND: (i64, i64, i64) = (40, 300, 20);

/// `value` moved by `steps` of `range`, kept inside it.
pub fn step(value: i64, steps: i64, range: (i64, i64, i64)) -> i64 {
    (value + steps * range.2).clamp(range.0, range.1)
}

/// Whether two hex colors are the same, ignoring case.
pub fn same_color(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Inline style of the preview card: width, edge and corner, in units of the
/// 1920 px stage (`--u` is one stage pixel).
///
/// Every property is set on every call (`auto` for the unused edges): the
/// renderer updates a `style` string property by property and does not remove
/// the ones a new string leaves out, so a card moved from the bottom left to the
/// top right would keep its old `bottom` and `left` and stretch.
pub fn card_style(c: &OverlayConfig) -> String {
    let u = |px: i64| format!("calc({px}*var(--u))");
    let (vert, hor) = c.position.split_once('-').unwrap_or(("bottom", "left"));
    let top = vert == "top";
    // A full-width band takes the edge, so the card sits above or below it.
    let edge = u(40
        + if c.waveform_band {
            c.waveform_height
        } else {
            0
        });
    let (left, right, transform) = match hor {
        "right" => ("auto".to_string(), u(40), "none"),
        "center" => ("50%".to_string(), "auto".to_string(), "translateX(-50%)"),
        _ => (u(40), "auto".to_string(), "none"),
    };
    format!(
        "width:{};top:{};bottom:{};left:{left};right:{right};transform:{transform};",
        u(c.width),
        if top { edge.clone() } else { "auto".into() },
        if top { "auto".into() } else { edge },
    )
}

/// Inline style of the full-width waveform band (both edges set, as above).
pub fn band_style(c: &OverlayConfig) -> String {
    let top = c.position.starts_with("top");
    format!(
        "top:{};bottom:{};height:calc({}*var(--u))",
        if top { "0" } else { "auto" },
        if top { "auto" } else { "0" },
        c.waveform_height
    )
}

/// SVG path of the overlay waveform in a 400 x 100 view, from values in 0..=1.
/// `full` mirrors it around the middle; otherwise the bars grow up from the bottom.
pub fn wave_path(values: &[f64], full: bool) -> String {
    if values.is_empty() {
        return String::new();
    }
    let h = |v: f64| (v.clamp(0.0, 1.0) * if full { 48.0 } else { 96.0 }).max(1.0);
    let up = values
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, if full { 50.0 - h(v) } else { 100.0 - h(v) }));
    let down = values
        .iter()
        .enumerate()
        .rev()
        .map(|(i, &v)| (i, if full { 50.0 + h(v) } else { 100.0 }));
    let pts: Vec<String> = up.chain(down).map(|(i, y)| format!("{i} {y:.1}")).collect();
    format!("M{}Z", pts.join("L"))
}

/// The overlay page the daemon serves.
pub fn url(base: &str) -> String {
    format!("{}/overlay", base.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> OverlayConfig {
        OverlayConfig {
            position: "bottom-left".into(),
            width: 440,
            waveform_height: 120,
            ..Default::default()
        }
    }

    #[test]
    fn steppers_stay_in_range() {
        assert_eq!(step(440, 1, WIDTH), 460);
        assert_eq!(step(440, -1, WIDTH), 420);
        assert_eq!(step(280, -1, WIDTH), 280);
        assert_eq!(step(1000, 1, WIDTH), 1000);
        assert_eq!(step(1, -1, HISTORY), 1);
        assert_eq!(step(10, 1, HISTORY), 10);
        assert_eq!(step(40, -1, BAND), 40);
        assert_eq!(step(290, 1, BAND), 300);
    }

    #[test]
    fn card_follows_the_corner_and_the_band() {
        assert_eq!(
            card_style(&cfg()),
            "width:calc(440*var(--u));top:auto;bottom:calc(40*var(--u));left:calc(40*var(--u));right:auto;transform:none;"
        );
        let mut c = cfg();
        c.position = "top-right".into();
        assert_eq!(
            card_style(&c),
            "width:calc(440*var(--u));top:calc(40*var(--u));bottom:auto;left:auto;right:calc(40*var(--u));transform:none;"
        );
        c.position = "bottom-center".into();
        assert!(card_style(&c).contains("left:50%;right:auto;transform:translateX(-50%);"));
        c.waveform_band = true;
        assert!(
            card_style(&c).contains("bottom:calc(160*var(--u))"),
            "40 + the 120 px band"
        );
        assert_eq!(
            band_style(&c),
            "top:auto;bottom:0;height:calc(120*var(--u))"
        );
        c.position = "top-left".into();
        assert!(band_style(&c).starts_with("top:0;bottom:auto;"));
    }

    #[test]
    fn every_corner_sets_the_same_properties() {
        // The renderer cannot remove a property a later style omits, so each style must name all of them.
        let props = |s: String| -> Vec<String> {
            s.split(';')
                .filter(|p| !p.is_empty())
                .map(|p| p.split(':').next().unwrap().to_string())
                .collect()
        };
        let mut c = cfg();
        let first = props(card_style(&c));
        for (pos, _) in POSITIONS {
            c.position = pos.to_string();
            assert_eq!(props(card_style(&c)), first, "{pos}");
        }
        let b0 = props(band_style(&c));
        c.position = "bottom-left".into();
        assert_eq!(props(band_style(&c)), b0);
    }

    #[test]
    fn wave_shapes() {
        let full = wave_path(&[0.0, 1.0], true);
        assert!(full.starts_with("M0 49.0L1 2.0L1 98.0L0 51.0Z"), "{full}");
        let half = wave_path(&[0.0, 1.0], false);
        assert!(half.starts_with("M0 99.0L1 4.0L1 100.0L0 100.0Z"), "{half}");
        assert_eq!(wave_path(&[], true), "");
    }

    #[test]
    fn odd_values_do_not_break_the_style() {
        let mut c = cfg();
        c.position = "somewhere".into();
        assert!(card_style(&c).contains("bottom:"));
        assert!(same_color("#ff7714", "#FF7714"));
        assert!(!same_color("#ff7714", "#FFC15A"));
        assert_eq!(url("http://h:9443/"), "http://h:9443/overlay");
    }
}
