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
    pub fn css_vars(&self) -> String {
        format!("--wk:{};--wm:{};--wh:{}", self.lo, self.mi, self.hi)
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
