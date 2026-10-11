//! Platform-free client logic: library search and suggestions, recent-search
//! ranking, key compatibility, playhead math, tap tempo, waveform overview
//! paths, and host checks. Beat-grid and transport math for the track drawer is in `grid`.

pub mod columns;
pub mod diag;
pub mod grid;
pub mod key;
pub mod net;
pub mod overlay;
pub mod palette;
pub mod phrase;
pub mod playhead;
pub mod queue;
pub mod route;
pub mod search;
pub mod settings;
pub mod smart;
pub mod sort;
pub mod tap;
pub mod waveform;
