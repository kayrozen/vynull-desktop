//! Tap tempo: the median interval of the last taps, like the mockup. The count
//! resets after a 2 second pause, and only the last 12 taps count.

const RESET_MS: f64 = 2000.0;
const KEEP: usize = 12;

#[derive(Debug, Clone, Default)]
pub struct TapTempo {
    taps: Vec<f64>,
}

impl TapTempo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a tap at `now_ms` (any monotonic clock). Returns the BPM once
    /// there are two taps.
    pub fn tap(&mut self, now_ms: f64) -> Option<f64> {
        if self.taps.last().is_some_and(|&t| now_ms - t > RESET_MS) {
            self.taps.clear();
        }
        self.taps.push(now_ms);
        if self.taps.len() > KEEP {
            self.taps.remove(0);
        }
        let mut gaps: Vec<f64> = self.taps.windows(2).map(|w| w[1] - w[0]).collect();
        if gaps.is_empty() {
            return None;
        }
        gaps.sort_by(|a, b| a.total_cmp(b));
        let median = gaps[gaps.len() / 2];
        (median > 0.0).then(|| 60_000.0 / median)
    }

    /// Taps counted so far, for the `(n)` label.
    pub fn count(&self) -> usize {
        self.taps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_taps_give_the_tempo() {
        let mut t = TapTempo::new();
        assert_eq!(t.tap(0.0), None);
        assert!((t.tap(500.0).unwrap() - 120.0).abs() < 1e-9);
        assert!((t.tap(1000.0).unwrap() - 120.0).abs() < 1e-9);
        assert_eq!(t.count(), 3);
    }

    #[test]
    fn median_ignores_one_stumble() {
        let mut t = TapTempo::new();
        for ms in [0.0, 500.0, 1000.0, 1900.0, 2400.0] {
            t.tap(ms);
        }
        // Gaps 500, 500, 900, 500: the median is 500.
        assert!((t.tap(2900.0).unwrap() - 120.0).abs() < 1e-9);
    }

    #[test]
    fn a_pause_resets_and_only_twelve_taps_count() {
        let mut t = TapTempo::new();
        t.tap(0.0);
        t.tap(500.0);
        assert_eq!(t.tap(10_000.0), None);
        assert_eq!(t.count(), 1);
        for i in 0..20 {
            t.tap(10_000.0 + 400.0 * (i + 1) as f64);
        }
        assert_eq!(t.count(), 12);
    }
}
