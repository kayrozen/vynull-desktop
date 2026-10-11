//! The play queue. The API has no queue, so it lives in the client's store:
//! the tracks to play next, in order, and which of them were already loaded.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    pub ids: Vec<i64>,
    /// Queued tracks that were loaded on a deck (shown dimmed).
    #[serde(default)]
    pub played: Vec<i64>,
}

impl Queue {
    pub fn contains(&self, id: i64) -> bool {
        self.ids.contains(&id)
    }

    pub fn is_played(&self, id: i64) -> bool {
        self.played.contains(&id)
    }

    /// Append a track (a track can be queued once).
    pub fn add(&mut self, id: i64) {
        if !self.contains(id) {
            self.ids.push(id);
        }
    }

    pub fn remove(&mut self, id: i64) {
        self.ids.retain(|x| *x != id);
        self.played.retain(|x| *x != id);
    }

    pub fn toggle(&mut self, id: i64) {
        if self.contains(id) {
            self.remove(id);
        } else {
            self.add(id);
        }
    }

    /// Move one place earlier (`-1`) or later (`1`).
    pub fn step(&mut self, id: i64, delta: isize) {
        let Some(i) = self.ids.iter().position(|x| *x == id) else {
            return;
        };
        let j = i as isize + delta;
        if (0..self.ids.len() as isize).contains(&j) {
            self.ids.swap(i, j as usize);
        }
    }

    /// Note that a queued track was loaded. Tracks not in the queue are ignored.
    pub fn mark_played(&mut self, id: i64) {
        if self.contains(id) && !self.is_played(id) {
            self.played.push(id);
        }
    }

    pub fn next_unplayed(&self) -> Option<i64> {
        self.ids.iter().copied().find(|id| !self.is_played(*id))
    }

    pub fn clear(&mut self) {
        *self = Queue::default();
    }

    /// Drop ids that are no longer in the library.
    pub fn retain_known(&mut self, known: impl Fn(i64) -> bool) {
        self.ids.retain(|id| known(*id));
        self.played.retain(|id| known(*id));
    }
}

/// `1:05:09` for a total of seconds.
pub fn total_label(secs: i64) -> String {
    format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_toggle_remove_and_order() {
        let mut q = Queue::default();
        for id in [2, 5, 7] {
            q.add(id);
        }
        q.add(5); // already queued
        assert_eq!(q.ids, vec![2, 5, 7]);
        q.step(7, -1);
        assert_eq!(q.ids, vec![2, 7, 5]);
        q.step(2, -1); // already first
        q.step(5, 1); // already last
        assert_eq!(q.ids, vec![2, 7, 5]);
        q.toggle(7);
        assert_eq!(q.ids, vec![2, 5]);
        q.toggle(9);
        assert!(q.contains(9));
    }

    #[test]
    fn played_tracks_are_skipped_by_next() {
        let mut q = Queue::default();
        for id in [2, 5, 7] {
            q.add(id);
        }
        assert_eq!(q.next_unplayed(), Some(2));
        q.mark_played(2);
        q.mark_played(99); // not queued: ignored
        assert_eq!(q.next_unplayed(), Some(5));
        q.mark_played(5);
        q.mark_played(7);
        assert_eq!(q.next_unplayed(), None);
        q.remove(5);
        assert!(!q.is_played(5));
        q.clear();
        assert_eq!(q, Queue::default());
    }

    #[test]
    fn unknown_tracks_are_dropped_and_totals_format() {
        let mut q = Queue {
            ids: vec![1, 2, 3],
            played: vec![2, 3],
        };
        q.retain_known(|id| id != 2);
        assert_eq!((q.ids.clone(), q.played.clone()), (vec![1, 3], vec![3]));
        assert_eq!(total_label(3909), "1:05:09");
        assert_eq!(total_label(0), "0:00:00");
        let back: Queue = serde_json::from_str(&serde_json::to_string(&q).unwrap()).unwrap();
        assert_eq!(back, q);
        assert_eq!(
            serde_json::from_str::<Queue>(r#"{"ids":[4]}"#)
                .unwrap()
                .played,
            Vec::<i64>::new()
        );
    }
}
