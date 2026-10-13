//! The app's accent color: six presets, a custom color from a wheel, and the shades derived from it
//! (readable text on top of it, and a darker variant for the light theme).

/// `(name, hex)`. The first is the default.
pub const PRESETS: [(&str, &str); 6] = [
    ("Amber", "#FFC15A"),
    ("Orange", "#FF7714"),
    ("Teal", "#3FD6C0"),
    ("Green", "#5FD38D"),
    ("Blue", "#6FB1FF"),
    ("Pink", "#FF8AD0"),
];

pub const DEFAULT: &str = PRESETS[0].1;

/// `#rrggbb` (or `rrggbb`, any case) as bytes.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn to_hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// `#RRGGBB` for a valid color, `None` otherwise.
pub fn normalize(s: &str) -> Option<String> {
    parse_hex(s).map(to_hex)
}

fn lin(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance, 0 (black) to 1 (white).
pub fn luminance(rgb: [u8; 3]) -> f64 {
    0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])
}

/// The text color that reads best on `bg`: the app's near-black, or white.
pub fn on_color(bg: [u8; 3]) -> &'static str {
    // Contrast of near-black (#05070A, luminance ~0.002) against white decides at about 0.18.
    if luminance(bg) > 0.18 {
        "#05070A"
    } else {
        "#FFFFFF"
    }
}

/// `(hue 0..360, saturation 0..1, value 0..1)`.
pub fn to_hsv(rgb: [u8; 3]) -> (f64, f64, f64) {
    let [r, g, b] = rgb.map(|c| c as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

pub fn from_hsv(h: f64, s: f64, v: f64) -> [u8; 3] {
    let h = h.rem_euclid(360.0);
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|c| ((c + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// The accent as text on a light background: darkened until it has enough contrast with white
/// (about 4.5:1), keeping its hue.
pub fn for_light_text(rgb: [u8; 3]) -> [u8; 3] {
    let (h, s, mut v) = to_hsv(rgb);
    let mut out = rgb;
    // Contrast against #F4F5F7 (luminance ~0.91): (0.91 + .05) / (l + .05) >= 4.5 -> l <= 0.163.
    while luminance(out) > 0.163 && v > 0.05 {
        v -= 0.02;
        out = from_hsv(h, s, v);
    }
    out
}

/// The accent text of the stylesheet's own themes, for the default amber.
const DEFAULT_TEXT_NIGHT: &str = "#FFC15A";
const DEFAULT_TEXT_DAY: &str = "#8A5200";

/// CSS variables for the app container. Always the same three, also for the default: the UI
/// updates a `style` string property by property and does not remove one that is left out, so
/// going back to amber must say amber.
pub fn css_vars(hex: &str, day: bool) -> String {
    let rgb = parse_hex(hex).unwrap_or([255, 193, 90]);
    let is_default = to_hex(rgb).eq_ignore_ascii_case(DEFAULT);
    let text = match (is_default, day) {
        (true, false) => DEFAULT_TEXT_NIGHT.to_string(),
        (true, true) => DEFAULT_TEXT_DAY.to_string(),
        (false, false) => to_hex(rgb),
        (false, true) => to_hex(for_light_text(rgb)),
    };
    format!(
        "--ac:{};--ac-on:{};--accent-text:{text};",
        to_hex(rgb),
        on_color(rgb)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_rejects_junk() {
        assert_eq!(parse_hex("#ff7714"), Some([255, 119, 20]));
        assert_eq!(parse_hex("FF7714"), Some([255, 119, 20]));
        assert_eq!(to_hex([255, 119, 20]), "#FF7714");
        assert_eq!(normalize(" #3fd6c0 ").as_deref(), Some("#3FD6C0"));
        for bad in ["", "#fff", "#gggggg", "#12345", "#1234567", "é12345"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn hsv_round_trips() {
        for (_, hex) in PRESETS {
            let rgb = parse_hex(hex).unwrap();
            let (h, s, v) = to_hsv(rgb);
            assert_eq!(from_hsv(h, s, v), rgb, "{hex}");
        }
        assert_eq!(from_hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(from_hsv(120.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(from_hsv(240.0, 1.0, 1.0), [0, 0, 255]);
        assert_eq!(from_hsv(0.0, 0.0, 0.5), [128, 128, 128]);
        assert_eq!(from_hsv(360.0, 1.0, 1.0), [255, 0, 0]);
    }

    #[test]
    fn text_on_the_accent_is_readable() {
        for (_, hex) in PRESETS {
            assert_eq!(on_color(parse_hex(hex).unwrap()), "#05070A", "{hex}");
        }
        assert_eq!(on_color([20, 20, 120]), "#FFFFFF");
        assert_eq!(on_color([0, 0, 0]), "#FFFFFF");
    }

    #[test]
    fn the_light_theme_gets_a_darker_text_shade() {
        for (_, hex) in PRESETS {
            let d = for_light_text(parse_hex(hex).unwrap());
            assert!(luminance(d) <= 0.17, "{hex} -> {d:?}");
        }
        // Already dark colors are left alone.
        assert_eq!(for_light_text([20, 20, 120]), [20, 20, 120]);
    }

    #[test]
    fn the_default_is_stated_not_left_out() {
        assert_eq!(
            css_vars("", false),
            "--ac:#FFC15A;--ac-on:#05070A;--accent-text:#FFC15A;"
        );
        assert_eq!(
            css_vars("#ffc15a", true),
            "--ac:#FFC15A;--ac-on:#05070A;--accent-text:#8A5200;"
        );
        assert_eq!(css_vars("nonsense", false), css_vars("", false));
        assert!(css_vars("#FF7714", false).contains("--ac:#FF7714;"));
        assert!(css_vars("#FF7714", true).contains("--accent-text:#"));
        assert_ne!(css_vars("#FF7714", true), css_vars("#FF7714", false));
    }
}
