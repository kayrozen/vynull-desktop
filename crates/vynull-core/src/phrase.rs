//! Song phrases (`/api/tracks/{id}/phrases`) and the vocal markers on them.
//! The daemon scores each phrase for singing; the client picks how strict to be.

/// How readily a phrase is marked as sung. Higher catches more, with more false positives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Vocal {
    Off,
    Low,
    #[default]
    Med,
    High,
}

impl Vocal {
    pub const ALL: [Vocal; 4] = [Vocal::Off, Vocal::Low, Vocal::Med, Vocal::High];

    pub fn label(self) -> &'static str {
        match self {
            Vocal::Off => "OFF",
            Vocal::Low => "LOW",
            Vocal::Med => "MED",
            Vocal::High => "HIGH",
        }
    }

    /// The next level, wrapping from HIGH to OFF.
    pub fn next(self) -> Vocal {
        match self {
            Vocal::Off => Vocal::Low,
            Vocal::Low => Vocal::Med,
            Vocal::Med => Vocal::High,
            Vocal::High => Vocal::Off,
        }
    }

    /// A phrase is marked when its score is above this.
    pub fn threshold(self) -> f64 {
        match self {
            Vocal::Off => f64::INFINITY,
            Vocal::Low => 0.15,
            Vocal::Med => 0.12,
            Vocal::High => 0.085,
        }
    }

    pub fn marks(self, score: f64) -> bool {
        score > self.threshold()
    }

    /// Stored form: the position in [`Vocal::ALL`].
    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(2) as u8
    }

    pub fn from_index(i: u8) -> Vocal {
        Self::ALL.get(i as usize).copied().unwrap_or_default()
    }
}

/// Fill color of a phrase segment by its kind.
pub fn kind_color(kind: i64) -> &'static str {
    match kind {
        1 => "#5CDADA",
        2 => "#FFD84D",
        3 => "#5AA8FF",
        5 => "#FF6A00",
        6 => "#8A8A92",
        _ => "#3A4350",
    }
}

/// `m:ss`, for the tooltip.
pub fn clock(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_cycle_and_mark_by_score() {
        assert_eq!(Vocal::default(), Vocal::Med);
        assert_eq!(Vocal::High.next(), Vocal::Off);
        assert!(!Vocal::Off.marks(10.0));
        assert!(Vocal::High.marks(0.1));
        assert!(!Vocal::Med.marks(0.1));
        assert!(
            !Vocal::Low.marks(0.15),
            "the threshold itself is not above it"
        );
        assert!(Vocal::Low.marks(0.16));
    }

    #[test]
    fn index_round_trips_and_bad_values_fall_back() {
        for v in Vocal::ALL {
            assert_eq!(Vocal::from_index(v.index()), v);
        }
        assert_eq!(Vocal::from_index(9), Vocal::Med);
    }

    #[test]
    fn clock_formats() {
        assert_eq!(clock(125_000), "2:05");
        assert_eq!(clock(-5), "0:00");
    }
}
