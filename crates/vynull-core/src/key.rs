//! Camelot-wheel key compatibility, as the deck cards use it to tint a key
//! that mixes well with the tempo master.

/// Parse `"9A"`, `"12B"` into (number 1..=12, minor?). Letter A is minor.
pub(crate) fn parse(key: &str) -> Option<(u8, bool)> {
    let key = key.trim();
    // Split off the last character, not the last byte: a key can end in "♯" or "♭".
    let letter = key.chars().next_back()?;
    let num = &key[..key.len() - letter.len_utf8()];
    let n: u8 = num.parse().ok()?;
    if !(1..=12).contains(&n) {
        return None;
    }
    match letter {
        'A' | 'a' => Some((n, true)),
        'B' | 'b' => Some((n, false)),
        _ => None,
    }
}

/// Same key, one step around the wheel (same letter), or the relative major/minor.
/// Anything that is not a Camelot key is never compatible.
pub fn compatible(a: &str, b: &str) -> bool {
    let (Some((na, ma)), Some((nb, mb))) = (parse(a), parse(b)) else {
        return false;
    };
    if na == nb {
        return true; // same key, or the relative key (A/B)
    }
    let step = (na as i8 - nb as i8).rem_euclid(12);
    ma == mb && (step == 1 || step == 11)
}

/// How a track's key relates to the reference (master) key, as the library
/// tints it: green mixes well, yellow is two steps up the wheel, orange is the
/// energy-boost jump of seven, red is the jump of four (it clashes: shown so
/// it can be avoided). Everything else further away is left untinted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    None,
    Ok,
    Yellow,
    Orange,
    Red,
}

impl Relation {
    /// CSS class for the key chip.
    pub fn class(self) -> &'static str {
        match self {
            Relation::None => "key",
            Relation::Ok => "key ok",
            Relation::Yellow => "key y",
            Relation::Orange => "key o",
            Relation::Red => "key r",
        }
    }
}

pub fn relation(key: &str, reference: &str) -> Relation {
    let (Some((n, minor)), Some((nb, minor_b))) = (parse(key), parse(reference)) else {
        return Relation::None;
    };
    let dd = (n as i8 - nb as i8).rem_euclid(12);
    let rd = dd.min(12 - dd);
    if (minor == minor_b && rd <= 1) || (minor != minor_b && rd == 0) {
        Relation::Ok
    } else if minor == minor_b && dd == 2 {
        Relation::Yellow
    } else if minor == minor_b && dd == 7 {
        Relation::Orange
    } else if minor == minor_b && dd == 4 {
        Relation::Red
    } else {
        Relation::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_neighbours() {
        assert!(compatible("9A", "9A"));
        assert!(compatible("9A", "8A"));
        assert!(compatible("9A", "10A"));
        assert!(compatible("9A", "9B"));
        assert!(!compatible("9A", "4A"));
        assert!(!compatible("9A", "10B"));
    }

    #[test]
    fn relation_matches_the_mockup_against_9a() {
        let r = |k| relation(k, "9A");
        assert_eq!(r("9A"), Relation::Ok);
        assert_eq!(r("8A"), Relation::Ok);
        assert_eq!(r("10A"), Relation::Ok);
        assert_eq!(r("9B"), Relation::Ok);
        assert_eq!(r("11A"), Relation::Yellow);
        assert_eq!(r("4A"), Relation::Orange);
        assert_eq!(r("1A"), Relation::Red);
        assert_eq!(r("1B"), Relation::None);
        assert_eq!(r("3B"), Relation::None);
        assert_eq!(r("10B"), Relation::None);
        assert_eq!(r("nope"), Relation::None);
        assert_eq!(relation("9A", ""), Relation::None);
    }

    #[test]
    fn relation_agrees_with_compatible() {
        for a in ["1A", "5B", "9A", "12B"] {
            for b in ["1A", "2A", "5A", "5B", "9B", "12A", "11B"] {
                assert_eq!(
                    compatible(a, b),
                    relation(a, b) == Relation::Ok,
                    "{a} vs {b}"
                );
            }
        }
    }

    #[test]
    fn wraps_around_twelve() {
        assert!(compatible("12A", "1A"));
        assert!(compatible("1B", "12B"));
        assert!(!compatible("12A", "2A"));
    }

    #[test]
    fn non_camelot_text_is_never_compatible() {
        assert!(!compatible("", "9A"));
        assert!(!compatible("Am", "9A"));
        assert!(!compatible("13A", "13A"));
        assert!(!compatible("0B", "0B"));
    }

    #[test]
    fn keys_ending_in_a_multibyte_character_are_not_camelot_and_do_not_panic() {
        for k in ["F♯", "9♭", "♯", "é", "9é", "", " ", "Ab♭", "日本語"] {
            assert_eq!(relation(k, "9A"), Relation::None, "{k:?}");
            assert_eq!(relation("9A", k), Relation::None, "{k:?}");
            assert!(!compatible(k, "9A"), "{k:?}");
        }
        assert_eq!(relation("9A", "9A"), Relation::Ok, "plain keys still work");
        assert_eq!(relation(" 9a ", "9A"), Relation::Ok);
    }
}
