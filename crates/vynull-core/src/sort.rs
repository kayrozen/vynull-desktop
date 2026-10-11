//! Sorting the library list by its column headers: one primary key and one
//! secondary key that breaks ties. Clicking a header makes it the primary key
//! and demotes the old primary to secondary, which works without a keyboard.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use vynull_api::TrackInfo;

use crate::columns::Col;
use crate::key;
use crate::search::fold;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortKey {
    pub col: Col,
    pub desc: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    /// Primary first; at most two.
    pub keys: Vec<SortKey>,
}

/// Columns that hold a value to sort by.
pub fn sortable(c: Col) -> bool {
    !matches!(c, Col::Art | Col::Overview | Col::Load)
}

impl Sort {
    fn position(&self, c: Col) -> Option<usize> {
        self.keys.iter().position(|k| k.col == c)
    }

    /// The direction of `c` and its rank (1 primary, 2 secondary), if it sorts.
    pub fn rank(&self, c: Col) -> Option<(usize, bool)> {
        self.position(c).map(|i| (i + 1, self.keys[i].desc))
    }

    /// Plain click: a new column becomes primary (the old primary becomes the
    /// secondary); the primary flips to descending and then switches off; the
    /// secondary is promoted.
    pub fn click(&mut self, c: Col) {
        if !sortable(c) {
            return;
        }
        match self.position(c) {
            Some(0) => {
                if self.keys[0].desc {
                    self.keys.remove(0);
                } else {
                    self.keys[0].desc = true;
                }
            }
            Some(i) => {
                let k = self.keys.remove(i);
                self.keys.insert(0, k);
            }
            None => {
                self.keys.insert(
                    0,
                    SortKey {
                        col: c,
                        desc: false,
                    },
                );
                self.keys.truncate(2);
            }
        }
    }

    /// Shift click: make `c` the secondary key, or flip it when it already is.
    pub fn click_secondary(&mut self, c: Col) {
        if !sortable(c) {
            return;
        }
        match self.position(c) {
            None if self.keys.is_empty() => self.keys.push(SortKey {
                col: c,
                desc: false,
            }),
            None => {
                self.keys.truncate(1);
                self.keys.push(SortKey {
                    col: c,
                    desc: false,
                });
            }
            Some(1) => {
                if self.keys[1].desc {
                    self.keys.remove(1);
                } else {
                    self.keys[1].desc = true;
                }
            }
            Some(_) => self.click(c),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Order `idx` (indexes into `tracks`). Stable, so unsorted ties keep
    /// their incoming order. Empty values go last in either direction.
    pub fn apply(&self, tracks: &[TrackInfo], idx: &mut [usize]) {
        if self.keys.is_empty() {
            return;
        }
        idx.sort_by(|&a, &b| {
            for k in &self.keys {
                let (ta, tb) = (&tracks[a], &tracks[b]);
                let o = match (value(k.col, ta), value(k.col, tb)) {
                    (Value::Empty, Value::Empty) => Ordering::Equal,
                    (Value::Empty, _) => return Ordering::Greater,
                    (_, Value::Empty) => return Ordering::Less,
                    (x, y) => x.cmp(&y),
                };
                let o = if k.desc { o.reverse() } else { o };
                if o != Ordering::Equal {
                    return o;
                }
            }
            Ordering::Equal
        });
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    Empty,
    Num(i64),
    Pair(i64, i64),
    Text(String),
    Texts(String, String),
}

fn text(s: &str) -> Value {
    if s.trim().is_empty() {
        Value::Empty
    } else {
        Value::Text(fold(s))
    }
}

fn value(c: Col, t: &TrackInfo) -> Value {
    match c {
        Col::Color => {
            if t.color_id == 0 {
                Value::Empty
            } else {
                Value::Num(t.color_id)
            }
        }
        Col::Track => {
            if t.title.trim().is_empty() {
                Value::Empty
            } else {
                Value::Texts(fold(&t.title), fold(&t.artist))
            }
        }
        Col::Comment => text(&t.comment),
        Col::Genre => text(&t.genre),
        Col::Bpm => {
            if t.bpm > 0.0 {
                Value::Num((t.bpm * 100.0).round() as i64)
            } else {
                Value::Empty
            }
        }
        Col::Key => match key::parse(&t.key) {
            Some((n, minor)) => Value::Pair(n as i64, if minor { 0 } else { 1 }),
            None => Value::Empty,
        },
        Col::Time => {
            if t.duration > 0 {
                Value::Num(t.duration)
            } else {
                Value::Empty
            }
        }
        Col::Rating => {
            if t.rating > 0 {
                Value::Num(t.rating)
            } else {
                Value::Empty
            }
        }
        Col::Art | Col::Overview | Col::Load => Value::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: i64, title: &str, genre: &str, bpm: f64, key: &str) -> TrackInfo {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "genre": genre, "bpm": bpm, "key": key
        }))
        .unwrap()
    }

    fn ids(tracks: &[TrackInfo], s: &Sort) -> Vec<i64> {
        let mut idx: Vec<usize> = (0..tracks.len()).collect();
        s.apply(tracks, &mut idx);
        idx.into_iter().map(|i| tracks[i].id).collect()
    }

    #[test]
    fn clicks_promote_flip_and_clear() {
        let mut s = Sort::default();
        s.click(Col::Genre);
        s.click(Col::Bpm);
        assert_eq!(s.rank(Col::Bpm), Some((1, false)));
        assert_eq!(s.rank(Col::Genre), Some((2, false)));
        s.click(Col::Key); // a third column pushes the old secondary out
        assert_eq!(s.rank(Col::Genre), None);
        assert_eq!(s.rank(Col::Bpm), Some((2, false)));
        s.click(Col::Bpm); // secondary promoted
        assert_eq!(s.rank(Col::Bpm), Some((1, false)));
        s.click(Col::Bpm);
        assert_eq!(s.rank(Col::Bpm), Some((1, true)));
        s.click(Col::Bpm); // off: the secondary moves up
        assert_eq!(s.rank(Col::Bpm), None);
        assert_eq!(s.rank(Col::Key), Some((1, false)));
        s.click(Col::Art);
        assert_eq!(s.keys.len(), 1, "artwork does not sort");
    }

    #[test]
    fn shift_click_sets_and_flips_the_secondary() {
        let mut s = Sort::default();
        s.click(Col::Genre);
        s.click_secondary(Col::Bpm);
        assert_eq!(s.rank(Col::Bpm), Some((2, false)));
        s.click_secondary(Col::Bpm);
        assert_eq!(s.rank(Col::Bpm), Some((2, true)));
        s.click_secondary(Col::Key);
        assert_eq!(s.rank(Col::Bpm), None);
        assert_eq!(s.rank(Col::Genre), Some((1, false)));
        s.click_secondary(Col::Key);
        s.click_secondary(Col::Key);
        assert_eq!(s.keys.len(), 1);
    }

    #[test]
    fn secondary_breaks_ties_and_empty_goes_last() {
        let tracks = vec![
            t(1, "b", "House", 124.0, "8A"),
            t(2, "a", "House", 120.0, "10A"),
            t(3, "c", "Techno", 130.0, "2B"),
            t(4, "d", "", 100.0, ""),
            t(5, "é", "house", 120.0, "9B"),
        ];
        let mut s = Sort::default();
        s.click(Col::Bpm);
        s.click(Col::Genre); // genre first, bpm second
        assert_eq!(ids(&tracks, &s), vec![2, 5, 1, 3, 4]);
        s.click(Col::Genre); // descending: empty still last
        assert_eq!(ids(&tracks, &s), vec![3, 2, 5, 1, 4]);
    }

    #[test]
    fn keys_sort_around_the_wheel_and_titles_ignore_accents() {
        let tracks = vec![
            t(1, "Zed", "", 0.0, "10A"),
            t(2, "Élan", "", 0.0, "2B"),
            t(3, "Ace", "", 0.0, "2A"),
        ];
        let mut s = Sort::default();
        s.click(Col::Key);
        assert_eq!(ids(&tracks, &s), vec![3, 2, 1]);
        let mut s = Sort::default();
        s.click(Col::Track);
        assert_eq!(ids(&tracks, &s), vec![3, 2, 1]);
    }
}
