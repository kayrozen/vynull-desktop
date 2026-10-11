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
            mode: String::new(),
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
            return "--wk:#7A3FD6;--wm:#0E9F8E;--wh:#4A4638".to_string();
        }
        format!("--wk:{};--wm:{};--wh:{}", self.lo, self.mi, self.hi)
    }
}

/// Presets for the waveform color modes, `(mode, [low, mid, high])`.
pub const PRESETS: [(&str, [&str; 3]); 6] = [
    ("3 BAND", ["#B07CFF", "#3FD6C0", "#F4F1E8"]),
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
