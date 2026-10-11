//! Client-side preferences kept in the [`vynull_platform::Store`].

use serde::{Deserialize, Serialize};
use vynull_platform::{Store, StoreExt};

/// Where the daemon is, as typed on the connection screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub host: String,
    pub port: u16,
}

impl Saved {
    pub fn base(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

pub const CONNECTION: &str = "connection";

/// Waveform band colors (the mockup's `vynull-wave` setting).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveColors {
    #[serde(default)]
    pub mode: String,
    pub lo: String,
    pub mi: String,
    pub hi: String,
}

impl Default for WaveColors {
    fn default() -> Self {
        Self {
            // As the daemon's own web UI: follow the CDJ's waveform color setting until told otherwise.
            mode: AUTO.into(),
            lo: "#B07CFF".into(),
            mi: "#3FD6C0".into(),
            hi: "#F4F1E8".into(),
        }
    }
}

impl WaveColors {
    pub const KEY: &'static str = "wave";

    /// A missing or unreadable value gives the defaults.
    pub fn load(store: &dyn Store) -> Self {
        store.get_json(Self::KEY).ok().flatten().unwrap_or_default()
    }

    /// CSS custom properties read by the `.wl`, `.wm` and `.wh` waveform classes.
    /// The default cream high band vanishes on the day theme's light panels, so
    /// untouched defaults become a darker set there; chosen colors are kept.
    pub fn css_vars(&self, day: bool) -> String {
        let d = Self::default();
        if day && self.lo == d.lo && self.mi == d.mi && self.hi == d.hi {
            return "--wk:#7A3FD6;--wm:#0E9F8E;--wh:#4A4638;--wblend:normal".to_string();
        }
        // The CDJ three-band look adds the bands together (white where all are loud); on the
        // light theme that would wash out, so it is drawn plainly there.
        let cdj = Self::preset("3 BAND");
        let blend = if !day && self.lo == cdj.lo && self.mi == cdj.mi && self.hi == cdj.hi {
            "plus-lighter"
        } else {
            "normal"
        };
        format!(
            "--wk:{};--wm:{};--wh:{};--wblend:{blend}",
            self.lo, self.mi, self.hi
        )
    }
}

/// Presets for the waveform color modes, `(mode, [low, mid, high])`.
pub const PRESETS: [(&str, [&str; 3]); 6] = [
    // The CDJ-3000 three-band colors (bass blue, mid amber, treble white), dimmed for additive blending.
    ("3 BAND", ["#1A66D9", "#D9911A", "#52565A"]),
    ("RGB", ["#FF4D4D", "#4DFF88", "#4D8CFF"]),
    ("BLUE", ["#2F6BFF", "#5C9DFF", "#D6E8FF"]),
    ("WARM", ["#FF6B3C", "#FFC15A", "#FFF1D0"]),
    ("ICE", ["#3A7BFF", "#6FD0FF", "#EAF6FF"]),
    ("MONO", ["#6B7482", "#9AA4B2", "#F4F1E8"]),
];

/// Mode name meaning "follow the daemon's waveform color setting".
pub const AUTO: &str = "AUTO";

impl WaveColors {
    pub fn preset(name: &str) -> WaveColors {
        let c = PRESETS
            .iter()
            .find(|p| p.0 == name)
            .map_or(PRESETS[0].1, |p| p.1);
        WaveColors {
            mode: name.to_string(),
            lo: c[0].into(),
            mi: c[1].into(),
            hi: c[2].into(),
        }
    }

    /// The colors to draw with: the saved ones, or, in AUTO mode, the preset
    /// matching the daemon's `waveform_color` (`blue`, `rgb`, `3band`).
    pub fn effective(&self, daemon: &str) -> WaveColors {
        if self.mode != AUTO {
            return self.clone();
        }
        let name = match daemon {
            "rgb" => "RGB",
            "3band" => "3 BAND",
            _ => "BLUE",
        };
        WaveColors {
            mode: AUTO.into(),
            ..Self::preset(name)
        }
    }
}

impl WaveColors {
    /// Spectral style to draw with, if any: Pioneer RGB or blue (from the CDJ's
    /// PWV5 data), chosen by an explicit RGB or BLUE mode or, in AUTO, by the
    /// daemon's `waveform_color`. `None` means the three bands in the saved colors.
    pub fn spectral(&self, daemon: &str) -> Option<vynull_core::waveform::Spectral> {
        use vynull_core::waveform::Spectral;
        match self.mode.as_str() {
            "RGB" => Some(Spectral::Rgb),
            "BLUE" => Some(Spectral::Blue),
            AUTO => match daemon {
                "3band" => None,
                "blue" => Some(Spectral::Blue),
                _ => Some(Spectral::Rgb),
            },
            _ => None,
        }
    }

    /// The `color` the daemon's PNG renderer wants (`rgb`, `blue`, `3band`) for the library rows.
    pub fn png_color(&self, daemon: &str) -> &'static str {
        match self.mode.as_str() {
            "RGB" => "rgb",
            "BLUE" => "blue",
            AUTO => match daemon {
                "blue" => "blue",
                "3band" => "3band",
                _ => "rgb",
            },
            _ => "3band",
        }
    }
}

/// Default colors of hot cues A to H for new cues made here (`color_id`, 0 = none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CueColors(pub Vec<i64>);

impl Default for CueColors {
    fn default() -> Self {
        Self(vec![0; 8])
    }
}

impl CueColors {
    pub const KEY: &'static str = "cuecolors";

    pub fn load(store: &dyn Store) -> Self {
        let mut v: CueColors = store.get_json(Self::KEY).ok().flatten().unwrap_or_default();
        v.0.resize(8, 0);
        v
    }

    /// The color for hot cue `number` (1 = A), 0 when none is set.
    pub fn for_cue(&self, number: i64) -> i64 {
        usize::try_from(number - 1)
            .ok()
            .and_then(|i| self.0.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// The next palette color for slot `number`, wrapping back to none.
    pub fn cycle(&mut self, number: i64) {
        if let Ok(i) = usize::try_from(number - 1) {
            if let Some(c) = self.0.get_mut(i) {
                *c = (*c + 1) % 9;
            }
        }
    }
}

pub const VOCAL_KEY: &str = "vocal";

/// Tracks whose beat grid and analysis are locked in this client.
///
/// The daemon has no lock of its own, so this only stops *this client* from
/// editing the grid, clearing it or analysing the track again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locks(pub std::collections::BTreeSet<i64>);

impl Locks {
    pub const KEY: &'static str = "locked";

    pub fn load(store: &dyn Store) -> Self {
        store.get_json(Self::KEY).ok().flatten().unwrap_or_default()
    }

    pub fn has(&self, id: i64) -> bool {
        self.0.contains(&id)
    }

    /// Lock the track, or unlock it when it is locked. Returns whether it is locked now.
    pub fn toggle(&mut self, id: i64) -> bool {
        if !self.0.remove(&id) {
            self.0.insert(id);
            return true;
        }
        false
    }
}

/// Theme and row density, kept in this browser (or this machine) only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    #[serde(default)]
    pub day: bool,
    #[serde(default)]
    pub compact: bool,
}

impl Look {
    pub const KEY: &'static str = "look";

    pub fn load(store: &dyn Store) -> Self {
        store.get_json(Self::KEY).ok().flatten().unwrap_or_default()
    }

    /// Class names on the app container.
    pub fn classes(&self) -> String {
        format!(
            "app{}{}",
            if self.day { " day" } else { "" },
            if self.compact { " compact" } else { "" }
        )
    }

    /// Row height of the library list in px; matches `--row-h` in the stylesheet.
    pub fn row_px(&self) -> f64 {
        if self.compact {
            48.0
        } else {
            64.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_daemon_waveform_color() {
        let auto = WaveColors::default();
        assert_eq!(auto.mode, AUTO);
        assert!(auto.spectral("rgb").is_some());
        assert!(auto.spectral("blue").is_some());
        assert!(
            auto.spectral("3band").is_none(),
            "3band keeps the band drawing"
        );
        let c = auto.effective("3band");
        assert_eq!(
            (c.lo.as_str(), c.mi.as_str(), c.hi.as_str()),
            ("#1A66D9", "#D9911A", "#52565A")
        );
        assert!(c.css_vars(false).contains("--wblend:plus-lighter"));
        assert!(
            c.css_vars(true).contains("--wblend:normal"),
            "no additive blending on the light theme"
        );
    }

    #[test]
    fn locks_toggle_per_track() {
        let mut l = Locks::default();
        assert!(l.toggle(5));
        assert!(l.has(5) && !l.has(6));
        assert!(!l.toggle(5));
        assert!(!l.has(5));
    }

    #[test]
    fn chosen_colors_are_drawn_plainly() {
        let own = WaveColors {
            mode: "3 BAND".into(),
            lo: "#112233".into(),
            mi: "#445566".into(),
            hi: "#778899".into(),
        };
        assert!(own.spectral("rgb").is_none());
        assert!(own.css_vars(false).contains("--wblend:normal"));
    }
}
