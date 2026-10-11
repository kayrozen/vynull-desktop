//! Playhead position of a deck between SSE events.
//!
//! The daemon reports `beat_in_track` (beats since the track start) and
//! `beat_age_ms` (how long ago that beat fired). Between events the client adds
//! the time since the event arrived. This assumes a steady tempo, so on a track
//! with a variable grid it drifts by up to a fraction of a beat; it is for the
//! deck overview, not for beat-accurate work. The first beat is counted as
//! position 0, which is unverified against a real deck.

/// Seconds from the track start. `since_event_secs` is the time since the SSE
/// event arrived; it only counts while the deck is playing.
pub fn position_secs(
    beat_in_track: i64,
    beat_age_ms: i64,
    bpm: f64,
    playing: bool,
    since_event_secs: f64,
) -> f64 {
    if bpm <= 0.0 || beat_in_track <= 0 {
        return 0.0;
    }
    let beats = (beat_in_track - 1) as f64;
    let mut t = beats * 60.0 / bpm + beat_age_ms.max(0) as f64 / 1000.0;
    if playing {
        t += since_event_secs.max(0.0);
    }
    t
}

/// Playhead as a 0..=100 percentage of the track.
pub fn percent(position_secs: f64, duration_ms: i64) -> f64 {
    if duration_ms <= 0 {
        return 0.0;
    }
    (position_secs * 1000.0 / duration_ms as f64 * 100.0).clamp(0.0, 100.0)
}

/// `-03:44` style remaining time.
pub fn remaining_label(position_secs: f64, duration_ms: i64) -> String {
    if duration_ms <= 0 {
        return "--:--".into();
    }
    let left = (duration_ms as f64 / 1000.0 - position_secs)
        .max(0.0)
        .round() as i64;
    format!("-{:02}:{:02}", left / 60, left % 60)
}

/// `3 / 4` beat label and the bar number, from the counters in `PlayerInfo`.
pub fn beat_labels(beat_in_bar: i64, beat_in_track: i64) -> (String, String) {
    if beat_in_bar <= 0 || beat_in_track <= 0 {
        return ("- / 4".into(), "-".into());
    }
    (
        format!("{beat_in_bar} / 4"),
        ((beat_in_track - 1) / 4 + 1).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_adds_age_and_elapsed_only_when_playing() {
        // 5th beat at 120 BPM is 2.0 s in, fired 250 ms ago.
        assert!((position_secs(5, 250, 120.0, true, 0.1) - 2.35).abs() < 1e-9);
        assert!((position_secs(5, 250, 120.0, false, 9.0) - 2.25).abs() < 1e-9);
        assert_eq!(position_secs(0, 0, 120.0, true, 1.0), 0.0);
        assert_eq!(position_secs(5, 0, 0.0, true, 1.0), 0.0);
    }

    #[test]
    fn percent_and_remaining() {
        assert_eq!(percent(30.0, 120_000), 25.0);
        assert_eq!(percent(500.0, 120_000), 100.0);
        assert_eq!(percent(1.0, 0), 0.0);
        assert_eq!(remaining_label(30.0, 120_000), "-01:30");
        assert_eq!(remaining_label(500.0, 120_000), "-00:00");
        assert_eq!(remaining_label(0.0, 0), "--:--");
    }

    #[test]
    fn beat_and_bar() {
        assert_eq!(beat_labels(3, 163), ("3 / 4".into(), "41".into()));
        assert_eq!(beat_labels(1, 1), ("1 / 4".into(), "1".into()));
        assert_eq!(beat_labels(0, 0), ("- / 4".into(), "-".into()));
    }
}
