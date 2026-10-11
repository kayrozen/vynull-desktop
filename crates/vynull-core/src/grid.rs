//! Math behind the track drawer: the zoom waveform and its beat grid, beat
//! jumps, cue stepping, metronome scheduling and time labels.
//!
//! Positions are seconds from the start of the audio unless a name says `ms`.

use vynull_api::{BeatInfo, CueInfo, WaveEntry3Band};

use crate::waveform::mirrored_path;

/// Width of the zoom view in its own coordinate system. The view shows 16 bars.
pub const VIEW_COLS: f64 = 600.0;
/// Detail waveform entries per second.
pub const ENTRIES_PER_SEC: f64 = 150.0;

/// Zoom scale: pixels (view columns) per second so that 16 bars span the view.
pub fn px_per_sec(bpm: f64) -> f64 {
    let bpm = if bpm.is_finite() && bpm > 20.0 {
        bpm.min(400.0)
    } else {
        120.0
    };
    VIEW_COLS * bpm / (16.0 * 4.0 * 60.0)
}

/// Seconds the zoom view spans at this tempo.
pub fn window_secs(bpm: f64) -> f64 {
    VIEW_COLS / px_per_sec(bpm)
}

/// Seconds the zoom view spans at a given scale (pixels per second).
pub fn window_secs_for(pxps: f64) -> f64 {
    VIEW_COLS / pxps.max(0.001)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Zoom {
    pub lo: String,
    pub mi: String,
    pub hi: String,
    /// Width of the whole track in view columns.
    pub total_cols: f64,
}

/// The whole track drawn once at the zoom scale. The view then only slides it.
pub fn zoom_paths(entries: &[WaveEntry3Band], pxps: f64) -> Zoom {
    if entries.is_empty() || pxps <= 0.0 {
        return Zoom::default();
    }
    let step = ENTRIES_PER_SEC / pxps; // entries per column
    let cols = (entries.len() as f64 / step).ceil().max(1.0) as usize;
    let column = |f: fn(&WaveEntry3Band) -> u8| -> Vec<f64> {
        (0..cols)
            .map(|i| {
                let a = (i as f64 * step) as usize;
                let b = (((i + 1) as f64 * step) as usize)
                    .max(a + 1)
                    .min(entries.len());
                entries[a.min(entries.len() - 1)..b]
                    .iter()
                    .map(|e| f(e) as f64 / 255.0)
                    .fold(0.0, f64::max)
            })
            .collect()
    };
    Zoom {
        lo: mirrored_path(&column(|e| e.bass), 49.0),
        mi: mirrored_path(&column(|e| e.mid), 38.0),
        hi: mirrored_path(&column(|e| e.high), 26.0),
        total_cols: cols as f64,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grid {
    /// Thin lines on every beat, as one SVG path in view columns.
    pub beats: String,
    /// Brighter lines on the bar starts.
    pub bars: String,
    /// `(x in columns, bar number)` for each bar start, numbered from 1.
    pub bar_labels: Vec<(f64, usize)>,
}

pub fn grid_lines(beats: &[BeatInfo], pxps: f64) -> Grid {
    let mut g = Grid::default();
    let mut bar = 0;
    for b in beats {
        let x = b.time_ms / 1000.0 * pxps;
        let seg = format!("M{x:.1} 0L{x:.1} 100");
        if b.beat_in_bar == 1 {
            bar += 1;
            g.bars.push_str(&seg);
            g.bar_labels.push((x, bar));
        } else {
            g.beats.push_str(&seg);
        }
    }
    g
}

/// Time of the first bar start (the downbeat), in ms.
pub fn first_downbeat_ms(beats: &[BeatInfo]) -> Option<f64> {
    beats.iter().find(|b| b.beat_in_bar == 1).map(|b| b.time_ms)
}

/// Position `n` beats from `pos` (negative goes back), snapped to the grid.
/// Without a grid it steps by the tempo. Never leaves `0..=duration`.
pub fn jump_beats(beats: &[BeatInfo], pos: f64, n: i32, bpm: f64, duration: f64) -> f64 {
    let target = if beats.is_empty() {
        pos + n as f64 * 60.0 / bpm.max(1.0)
    } else {
        // Index of the last beat at or before `pos` (-1 before the first). A beat up to
        // 50 ms ahead counts as the current one, so a playhead parked at 0:00 with the first
        // beat at 16 ms still jumps from that beat.
        let at = beats.partition_point(|b| b.time_ms / 1000.0 <= pos + 0.05) as i64 - 1;
        let to = (at + n as i64).clamp(0, beats.len() as i64 - 1) as usize;
        beats[to].time_ms / 1000.0
    };
    target.clamp(0.0, duration.max(0.0))
}

/// The previous (`dir < 0`) or next cue time, ignoring cues within 50 ms of `pos`.
pub fn step_cue(cues: &[CueInfo], pos: f64, dir: i32) -> Option<f64> {
    let mut times: Vec<f64> = cues.iter().map(|c| c.time_ms as f64 / 1000.0).collect();
    times.sort_by(|a, b| a.total_cmp(b));
    times.dedup();
    if dir > 0 {
        times.into_iter().find(|&t| t > pos + 0.05)
    } else {
        times.into_iter().rev().find(|&t| t < pos - 0.05)
    }
}

/// Metronome clicks to schedule: beats in `(pos, pos + lookahead]` that have not
/// been scheduled yet, as `(delay seconds, accent)`. `last` is the index of the
/// last scheduled beat and is advanced. A seek (position behind the last click)
/// restarts from the first beat at or after `pos`.
pub fn next_clicks(
    beats: &[BeatInfo],
    pos: f64,
    lookahead: f64,
    last: &mut Option<usize>,
) -> Vec<(f64, bool)> {
    if let Some(i) = *last {
        if beats.get(i).is_some_and(|b| {
            b.time_ms / 1000.0 > pos + lookahead + 0.5 || b.time_ms / 1000.0 < pos - 1.0
        }) {
            *last = None; // the playhead jumped
        }
    }
    let from = last.map_or(0, |i| i + 1);
    let mut out = Vec::new();
    for (i, b) in beats.iter().enumerate().skip(from) {
        let t = b.time_ms / 1000.0;
        if t > pos + lookahead {
            break;
        }
        if t > pos {
            out.push((t - pos, b.beat_in_bar == 1));
            *last = Some(i);
        } else if last.is_none() {
            *last = Some(i); // already past: do not click for it, but skip it
        }
    }
    out
}

/// `01:12.3`
pub fn time_label(ms: i64) -> String {
    let t = ms.max(0) / 100;
    format!("{:02}:{:02}.{}", t / 600, t / 10 % 60, t % 10)
}

/// Letter for a hot cue (`1` is A), `M` for memory cues (numbered from 9).
pub fn cue_letter(number: i64) -> String {
    if (1..=8).contains(&number) {
        ((b'A' + (number - 1) as u8) as char).to_string()
    } else {
        "M".to_string()
    }
}

/// First free hot cue number (1..=8), if any.
pub fn free_hot_cue(cues: &[CueInfo]) -> Option<i64> {
    (1..=8).find(|n| cues.iter().all(|c| c.number != *n))
}

/// Next memory cue number: they count upward from 9.
pub fn next_memory_cue(cues: &[CueInfo]) -> i64 {
    cues.iter()
        .map(|c| c.number)
        .filter(|n| *n >= 9)
        .max()
        .map_or(9, |m| m + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vynull_api::CueKind;

    fn beats(n: usize, bpm: f64) -> Vec<BeatInfo> {
        (0..n)
            .map(|i| BeatInfo {
                time_ms: i as f64 * 60_000.0 / bpm,
                beat_in_bar: (i % 4) as i64 + 1,
            })
            .collect()
    }

    fn cue(number: i64, ms: i64) -> CueInfo {
        CueInfo {
            number,
            kind: CueKind::Cue,
            time_ms: ms,
            loop_ms: -1,
            ..Default::default()
        }
    }

    #[test]
    fn zoom_scale_shows_sixteen_bars() {
        assert!((window_secs(120.0) - 32.0).abs() < 1e-9);
        assert!((window_secs(60.0) - 64.0).abs() < 1e-9);
        assert_eq!(px_per_sec(0.0), px_per_sec(120.0)); // bad tempo falls back
    }

    #[test]
    fn window_follows_the_scale() {
        assert!((window_secs_for(px_per_sec(120.0)) - window_secs(120.0)).abs() < 1e-9);
        assert!(window_secs_for(0.0).is_finite());
    }

    #[test]
    fn zoom_paths_cover_the_track() {
        let entries = vec![
            WaveEntry3Band {
                bass: 200,
                mid: 100,
                high: 50
            };
            1500
        ]; // 10 s
        let z = zoom_paths(&entries, 18.75);
        assert_eq!(z.total_cols, 188.0); // 10 s * 18.75 px/s, rounded up
        assert!(z.lo.starts_with("M0 ") && z.lo.ends_with('Z'));
        assert_eq!(zoom_paths(&[], 18.75), Zoom::default());
    }

    #[test]
    fn grid_lines_number_the_bars() {
        let g = grid_lines(&beats(9, 120.0), 20.0);
        assert_eq!(g.bar_labels, vec![(0.0, 1), (40.0, 2), (80.0, 3)]);
        assert_eq!(g.bars.matches('M').count(), 3);
        assert_eq!(g.beats.matches('M').count(), 6);
        assert_eq!(first_downbeat_ms(&beats(9, 120.0)), Some(0.0));
        let mut shifted = beats(9, 120.0);
        for b in &mut shifted {
            b.beat_in_bar = (b.beat_in_bar % 4) + 1; // first beat is now beat 2
        }
        assert_eq!(first_downbeat_ms(&shifted), Some(1500.0));
    }

    #[test]
    fn beat_jumps_snap_to_the_grid_and_clamp() {
        let b = beats(20, 120.0); // a beat every 0.5 s
        assert_eq!(jump_beats(&b, 2.2, 1, 120.0, 9.5), 2.5);
        assert_eq!(jump_beats(&b, 2.2, 4, 120.0, 9.5), 4.0);
        assert_eq!(jump_beats(&b, 2.2, -1, 120.0, 9.5), 1.5);
        assert_eq!(jump_beats(&b, 2.0, -1, 120.0, 9.5), 1.5); // on a beat: that beat counts as current
                                                              // Parked just before the first beat: that beat is the current one.
        let late: Vec<BeatInfo> = beats(20, 120.0)
            .into_iter()
            .map(|b| BeatInfo {
                time_ms: b.time_ms + 16.3,
                ..b
            })
            .collect();
        assert!((jump_beats(&late, 0.0, 4, 120.0, 9.5) - 2.0163).abs() < 1e-9);
        assert!((jump_beats(&late, 0.0, 1, 120.0, 9.5) - 0.5163).abs() < 1e-9);
        assert_eq!(jump_beats(&b, 1.0, -16, 120.0, 9.5), 0.0);
        assert_eq!(jump_beats(&b, 9.0, 16, 120.0, 9.5), 9.5);
        assert_eq!(jump_beats(&[], 10.0, 4, 120.0, 100.0), 12.0);
        assert_eq!(jump_beats(&[], 1.0, -4, 120.0, 100.0), 0.0);
    }

    #[test]
    fn cue_stepping_skips_the_cue_under_the_playhead() {
        let cues = vec![cue(1, 8200), cue(2, 72_000), cue(9, 8200), cue(3, 182_400)];
        assert_eq!(step_cue(&cues, 0.0, 1), Some(8.2));
        assert_eq!(step_cue(&cues, 8.2, 1), Some(72.0));
        assert_eq!(step_cue(&cues, 72.0, -1), Some(8.2));
        assert_eq!(step_cue(&cues, 8.2, -1), None);
        assert_eq!(step_cue(&cues, 200.0, 1), None);
    }

    #[test]
    fn metronome_schedules_each_beat_once() {
        let b = beats(16, 120.0);
        let mut last = None;
        let c1 = next_clicks(&b, 0.1, 0.3, &mut last); // beat at 0.5 is outside 0.1..0.4
        assert!(c1.is_empty());
        let c2 = next_clicks(&b, 0.3, 0.3, &mut last); // 0.5 is inside 0.3..0.6
        assert_eq!(c2.len(), 1);
        assert!((c2[0].0 - 0.2).abs() < 1e-9 && !c2[0].1);
        assert!(
            next_clicks(&b, 0.35, 0.3, &mut last).is_empty(),
            "no double click"
        );
        let c3 = next_clicks(&b, 1.8, 0.3, &mut last); // jumped ahead: beat at 2.0 is the downbeat of bar 2
        assert_eq!(c3.len(), 1);
        assert!(c3[0].1, "beat 1 of a bar is accented");
        // Seeking back restarts scheduling.
        let c4 = next_clicks(&b, 0.3, 0.3, &mut last);
        assert_eq!(c4.len(), 1);
    }

    #[test]
    fn labels_and_cue_numbers() {
        assert_eq!(time_label(8200), "00:08.2");
        assert_eq!(time_label(182_400), "03:02.4");
        assert_eq!(time_label(-5), "00:00.0");
        assert_eq!(cue_letter(1), "A");
        assert_eq!(cue_letter(8), "H");
        assert_eq!(cue_letter(9), "M");
        let cues = vec![cue(1, 0), cue(2, 0), cue(4, 0), cue(10, 0)];
        assert_eq!(free_hot_cue(&cues), Some(3));
        assert_eq!(next_memory_cue(&cues), 11);
        assert_eq!(next_memory_cue(&[]), 9);
        let full: Vec<CueInfo> = (1..=8).map(|n| cue(n, 0)).collect();
        assert_eq!(free_hot_cue(&full), None);
    }
}
