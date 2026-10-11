//! Track colors (the API's `color_id`: 0 none, 1 to 8) and the row tint they give.

pub const PALETTE: [(i64, &str, &str); 9] = [
    (0, "NONE", "#3A4350"),
    (1, "PINK", "#FF8AD0"),
    (2, "RED", "#FF6B6B"),
    (3, "ORANGE", "#FF9A3C"),
    (4, "YELLOW", "#E8E05A"),
    (5, "GREEN", "#5FD38D"),
    (6, "AQUA", "#3FD6C0"),
    (7, "BLUE", "#6FB1FF"),
    (8, "PURPLE", "#B07CFF"),
];

/// `(name, hex)` for a color id, or `None` for 0 and unknown ids.
pub fn color(id: i64) -> Option<(&'static str, &'static str)> {
    PALETTE
        .iter()
        .find(|p| p.0 == id && id != 0)
        .map(|p| (p.1, p.2))
}

fn channel(hex: &str, k: usize) -> f64 {
    u8::from_str_radix(&hex[1 + k * 2..3 + k * 2], 16).unwrap_or(0) as f64
}

/// A tag's dot color: its palette color (1 to 8), or a stable hue from its id when it has none.
pub fn tag_color(color: i64, id: i64) -> String {
    match self::color(color) {
        Some((_, hex)) => hex.to_string(),
        None => format!("hsl({},70%,66%)", (id * 47) % 360),
    }
}

/// The same color at partial opacity, for borders. (`hsl(..)88` is not valid CSS:
/// the hex alpha suffix only works on `#rrggbb`, and an invalid value leaves the old style.)
pub fn tag_color_dim(color: i64, id: i64) -> String {
    match self::color(color) {
        Some((_, hex)) => format!("{hex}88"),
        None => format!("hsla({},70%,66%,.53)", (id * 47) % 360),
    }
}

/// Blend `hex` into `base` by `a` (0..=1).
pub fn mix(base: &str, hex: &str, a: f64) -> String {
    let c = |k| (channel(base, k) * (1.0 - a) + channel(hex, k) * a).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(0), c(1), c(2))
}

/// CSS custom properties for a row tinted by its track color. With no color
/// they are the plain row colors, not empty: a renderer that updates styles
/// property by property would otherwise keep the old tint after a recolor.
pub fn tint_vars(color_id: i64) -> String {
    let Some((_, hex)) = color(color_id) else {
        return "--rb:var(--bg);--rh:var(--row-hover);--rs:var(--hover);".to_string();
    };
    // Mixed in CSS against the theme's own colors, so tints follow night and day.
    format!(
        "--rb:color-mix(in srgb,var(--bg) 93%,{hex} 7%);--rh:color-mix(in srgb,var(--row-hover) 89%,{hex} 11%);--rs:color-mix(in srgb,var(--hover) 86%,{hex} 14%);"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_tints() {
        assert_eq!(color(2), Some(("RED", "#FF6B6B")));
        assert_eq!(color(0), None);
        assert_eq!(color(99), None);
        assert_eq!(
            tint_vars(0),
            "--rb:var(--bg);--rh:var(--row-hover);--rs:var(--hover);"
        );
        assert_eq!(tint_vars(99), tint_vars(0));
        assert!(tint_vars(2).contains("#FF6B6B 7%"));
        assert_eq!(tag_color(5, 1), "#5FD38D");
        assert!(tag_color(0, 3).starts_with("hsl("));
        assert_eq!(tag_color_dim(5, 1), "#5FD38D88");
        assert!(tag_color_dim(0, 3).starts_with("hsla("));
        assert_eq!(mix("#000000", "#ffffff", 0.5), "#808080");
        assert_eq!(mix("#0E1116", "#FF6B6B", 0.0), "#0e1116");
    }
}
