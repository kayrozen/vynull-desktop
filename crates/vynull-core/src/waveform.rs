//! Deck overview waveform: three SVG paths (low, mid, high) drawn mirrored
//! around the centre of a 400 x 100 viewBox, like the mockup.

use vynull_api::WaveEntryColorPreview;

pub const VIEW_W: usize = 400;
pub const VIEW_H: f64 = 100.0;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct OverviewPaths {
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
