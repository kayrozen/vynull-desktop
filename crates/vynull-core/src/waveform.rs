//! Deck overview waveform: three SVG paths (low, mid, high) drawn mirrored
//! around the centre of a 400 x 100 viewBox, like the mockup.

use std::collections::BTreeMap;

use vynull_api::{WaveEntryColorPreview, WaveEntrySpectral};

pub const VIEW_W: usize = 400;
pub const VIEW_H: f64 = 100.0;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OverviewPaths {
    /// Per-column colored bars (`(fill, path)`) for the spectral styles; empty for the three-band paths.
    pub colored: Vec<(String, String)>,
    pub lo: String,
    pub mi: String,
    pub hi: String,
}

/// Max of `entries[i*step .. (i+1)*step]` for each of `cols` columns.
fn column_max(
    entries: &[WaveEntryColorPreview],
    cols: usize,
    f: fn(&WaveEntryColorPreview) -> u8,
) -> Vec<f64> {
    (0..cols)
        .map(|i| {
            let a = i * entries.len() / cols;
            let b = (((i + 1) * entries.len()) / cols)
                .max(a + 1)
                .min(entries.len());
            entries[a..b]
                .iter()
                .map(|e| f(e) as f64 / 255.0)
                .fold(0.0, f64::max)
        })
        .collect()
}

/// A mirrored waveform outline: one column per value, `amplitude` is the half
/// height in a 100 unit tall view. Used by the overview and the zoom view.
pub fn mirrored_path(values: &[f64], amplitude: f64) -> String {
    let mid = VIEW_H / 2.0;
    let h = |v: f64| (v * amplitude).max(0.6);
    let up = values
        .iter()
        .enumerate()
        .map(|(i, &v)| format!("{i} {:.1}", mid - h(v)));
    let down = values
        .iter()
        .enumerate()
        .rev()
        .map(|(i, &v)| format!("{i} {:.1}", mid + h(v)));
    let pts: Vec<String> = up.chain(down).collect();
    if pts.is_empty() {
        return String::new();
    }
    format!("M{}Z", pts.join("L"))
}

/// Paths for a `color_preview` waveform. Empty input gives empty paths.
pub fn overview_paths(entries: &[WaveEntryColorPreview]) -> OverviewPaths {
    if entries.is_empty() {
        return OverviewPaths::default();
    }
    OverviewPaths {
        lo: mirrored_path(&column_max(entries, VIEW_W, |e| e.bass), 49.0),
        mi: mirrored_path(&column_max(entries, VIEW_W, |e| e.mid), 38.0),
        hi: mirrored_path(&column_max(entries, VIEW_W, |e| e.treble), 26.0),
        ..OverviewPaths::default()
    }
}

/// One thin full-height bar per cue, as a single path. `positions` are 0..=1.
pub fn cue_marks(positions: &[f64]) -> String {
    positions
        .iter()
        .map(|p| {
            format!(
                "M{} 0h3v100h-3z",
                (p.clamp(0.0, 1.0) * (VIEW_W as f64 - 3.0)).round()
            )
        })
        .collect()
}

/// A fixed, made-up waveform for previews (colors, overlay shape): a quiet
/// intro, a build, a drop, a break and an outro, with a kick on every beat.
pub fn demo_entries(n: usize) -> Vec<WaveEntryColorPreview> {
    let mut seed = 7u32;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed as f64 / u32::MAX as f64
    };
    (0..n)
        .map(|i| {
            let p = i as f64 / n.max(1) as f64;
            let e = if p < 0.1 {
                0.3
            } else if p < 0.3 {
                0.5 + p
            } else if p < 0.55 {
                1.0
            } else if p < 0.65 {
                0.35
            } else {
                0.95
            };
            let beat = (i % 9) as f64 / 9.0;
            let kick = (-beat * 4.5).exp();
            let hat = (-(((beat + 0.5) % 1.0) * 7.0)).exp();
            let v = |x: f64| (x.clamp(0.0, 1.0) * 255.0) as u8;
            WaveEntryColorPreview {
                bass: v(e * (0.22 + 0.78 * kick) * (0.9 + 0.2 * rnd())),
                mid: v(e * (0.25 + 0.2 * kick) * (0.75 + 0.5 * rnd())),
                treble: v(e * (0.1 + 0.55 * hat) * (0.65 + 0.7 * rnd())),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(b: u8, m: u8, t: u8) -> WaveEntryColorPreview {
        WaveEntryColorPreview {
            bass: b,
            mid: m,
            treble: t,
        }
    }

    #[test]
    fn paths_have_one_point_pair_per_column() {
        let entries: Vec<_> = (0..1000).map(|i| e((i % 256) as u8, 100, 50)).collect();
        let p = overview_paths(&entries);
        // 400 up points + 400 down points, one 'M' and the 'Z'.
        assert_eq!(p.lo.matches('L').count(), 2 * VIEW_W - 1);
        assert!(p.lo.starts_with("M0 ") && p.lo.ends_with('Z'));
    }

    #[test]
    fn silent_and_loud_columns() {
        let p = overview_paths(&[e(0, 0, 0), e(255, 255, 255)]);
        // A silent column keeps a 0.6 sliver; a full one reaches the amplitude limit.
        assert!(p.lo.contains("0 49.4"));
        assert!(p.lo.contains("399 1.0") || p.lo.contains("399 "));
    }

    #[test]
    fn fewer_entries_than_columns_and_empty() {
        assert!(!overview_paths(&[e(10, 10, 10)]).lo.is_empty());
        assert_eq!(overview_paths(&[]), OverviewPaths::default());
    }

    #[test]
    fn the_demo_waveform_is_stable_and_has_dynamics() {
        let a = demo_entries(1200);
        assert_eq!(a.len(), 1200);
        assert_eq!(a, demo_entries(1200), "same every time");
        let loud = a[500..600].iter().map(|e| e.bass as u32).sum::<u32>();
        let quiet = a[0..100].iter().map(|e| e.bass as u32).sum::<u32>();
        assert!(loud > quiet * 2, "the drop is louder than the intro");
        assert!(!overview_paths(&a).lo.is_empty());
    }

    #[test]
    fn cues_clamp_inside_the_view() {
        assert_eq!(cue_marks(&[0.0, 2.0]), "M0 0h3v100h-3zM397 0h3v100h-3z");
    }
}

/// How the spectral (Pioneer PWV5) detail data is colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spectral {
    /// The CDJ's RGB waveform: each column's color is its three 3-bit band weights.
    Rgb,
    /// The CDJ's blue waveform: blue, whiter where the treble dominates.
    Blue,
}

/// Fill color of a column from its averaged band weights (0 to 7 each).
/// Quantised to the 3 bits the format has, so columns can share one path.
fn spectral_fill(style: Spectral, r: f64, g: f64, b: f64) -> String {
    match style {
        Spectral::Rgb => {
            let q = |v: f64| (v.round().clamp(0.0, 7.0) as u32 * 38).min(255);
            format!("rgb({},{},{})", q(r), q(g), q(b))
        }
        Spectral::Blue => {
            let sum = r + g + b;
            let tf = if sum > 0.0 { b / sum } else { 0.0 };
            let k = ((tf * 16.0).round() / 16.0) * 0.85;
            let mix = |lo: f64, hi: f64| (lo + (hi - lo) * k).round() as u32;
            format!(
                "rgb({},{},{})",
                mix(40.0, 255.0),
                mix(120.0, 255.0),
                mix(235.0, 255.0)
            )
        }
    }
}

/// Spectral entries as colored bars in a view `cols` wide and 100 tall, grouped
/// by color so a track makes at most a few hundred paths. Column `i` covers
/// `entries[i*step .. (i+1)*step]`: its height is the tallest entry in it
/// (`h` is 0 to 31) and its color the average of the band weights. With `half`
/// the bars grow from the bottom (the CDJ's half overview), otherwise from the middle.
pub fn spectral_groups(
    entries: &[WaveEntrySpectral],
    step: f64,
    cols: usize,
    style: Spectral,
    half: bool,
) -> Vec<(String, String)> {
    let mut groups: BTreeMap<String, String> = BTreeMap::new();
    if entries.is_empty() || step <= 0.0 {
        return Vec::new();
    }
    for i in 0..cols {
        let a = (i as f64 * step) as usize;
        if a >= entries.len() {
            break;
        }
        let b = (((i + 1) as f64 * step) as usize)
            .max(a + 1)
            .min(entries.len());
        let (mut mh, mut r, mut g, mut bl) = (0u8, 0.0, 0.0, 0.0);
        for e in &entries[a..b] {
            mh = mh.max(e.h);
            r += e.r as f64;
            g += e.g as f64;
            bl += e.b as f64;
        }
        if mh == 0 {
            continue;
        }
        let n = (b - a) as f64;
        let bar = (mh as f64 / 31.0 * VIEW_H).max(1.0);
        let y = if half {
            VIEW_H - bar
        } else {
            (VIEW_H - bar) / 2.0
        };
        let fill = spectral_fill(style, r / n, g / n, bl / n);
        groups
            .entry(fill)
            .or_default()
            .push_str(&format!("M{i} {y:.1}h1v{bar:.1}h-1z"));
    }
    groups.into_iter().collect()
}

/// Whole-track overview of spectral data in the deck overview's 400 columns.
pub fn spectral_overview(
    entries: &[WaveEntrySpectral],
    style: Spectral,
    half: bool,
) -> OverviewPaths {
    OverviewPaths {
        colored: spectral_groups(
            entries,
            entries.len() as f64 / VIEW_W as f64,
            VIEW_W,
            style,
            half,
        ),
        ..OverviewPaths::default()
    }
}

#[cfg(test)]
mod spectral_tests {
    use super::*;

    fn e(r: u8, g: u8, b: u8, h: u8) -> WaveEntrySpectral {
        WaveEntrySpectral { r, g, b, h }
    }

    #[test]
    fn rgb_uses_the_three_bit_weights() {
        let g = spectral_groups(
            &[e(7, 0, 0, 31), e(0, 7, 0, 31)],
            1.0,
            2,
            Spectral::Rgb,
            false,
        );
        let fills: Vec<&str> = g.iter().map(|x| x.0.as_str()).collect();
        assert!(fills.contains(&"rgb(255,0,0)"), "{fills:?}");
        assert!(fills.contains(&"rgb(0,255,0)"), "{fills:?}");
    }

    #[test]
    fn columns_with_the_same_color_share_a_path() {
        let g = spectral_groups(
            &[e(7, 7, 7, 10), e(7, 7, 7, 20)],
            1.0,
            2,
            Spectral::Rgb,
            false,
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].1.matches('M').count(), 2);
    }

    #[test]
    fn silence_draws_nothing_and_height_follows_h() {
        assert!(spectral_groups(&[e(7, 7, 7, 0)], 1.0, 1, Spectral::Rgb, false).is_empty());
        let full = spectral_groups(&[e(7, 7, 7, 31)], 1.0, 1, Spectral::Rgb, false);
        assert!(full[0].1.starts_with("M0 0.0h1v100.0"), "{}", full[0].1);
        let half = spectral_groups(&[e(7, 7, 7, 31 / 2)], 1.0, 1, Spectral::Rgb, true);
        assert!(
            half[0].1.contains("v46.8") || half[0].1.contains("v48.4"),
            "{}",
            half[0].1
        );
        let centred = spectral_groups(&[e(7, 7, 7, 15)], 1.0, 1, Spectral::Rgb, false);
        assert!(centred[0].1.starts_with("M0 25."), "{}", centred[0].1);
        let bottom = spectral_groups(&[e(7, 7, 7, 15)], 1.0, 1, Spectral::Rgb, true);
        assert!(bottom[0].1.starts_with("M0 51."), "{}", bottom[0].1);
    }

    #[test]
    fn a_column_takes_the_tallest_entry_and_the_mean_color() {
        let g = spectral_groups(
            &[e(7, 0, 0, 5), e(3, 0, 0, 31)],
            2.0,
            1,
            Spectral::Rgb,
            false,
        );
        assert_eq!(g.len(), 1);
        assert!(g[0].1.contains("v100.0"));
        assert_eq!(g[0].0, "rgb(190,0,0)"); // mean 5 -> 5 * 38
    }

    #[test]
    fn blue_gets_whiter_with_treble() {
        let low = spectral_groups(&[e(7, 0, 0, 20)], 1.0, 1, Spectral::Blue, false);
        let high = spectral_groups(&[e(0, 0, 7, 20)], 1.0, 1, Spectral::Blue, false);
        assert_eq!(low[0].0, "rgb(40,120,235)");
        assert_ne!(high[0].0, low[0].0);
    }
}
