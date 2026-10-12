//! Client-side library search, as in the mockup: accent-insensitive, every
//! word must match somewhere (AND), plus typed suggestions and a ranked list
//! of recent searches.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use vynull_api::TrackInfo;

/// Lowercase and strip accents (`Café` and `cafe` fold to the same text).
pub fn fold(s: &str) -> String {
    s.to_lowercase()
        .nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect()
}

/// BPM as the list shows it, and as search sees it.
pub fn bpm_label(bpm: f64) -> String {
    format!("{bpm:.1}")
}

/// Fields a suggestion can come from, in the order suggestions rank them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    Tag,
    Artist,
    Track,
    Genre,
    Album,
    Label,
    Remixer,
    Key,
    Comment,
}

impl Field {
    pub fn label(self) -> &'static str {
        match self {
            Field::Tag => "TAG",
            Field::Artist => "ARTIST",
            Field::Track => "TRACK",
            Field::Genre => "GENRE",
            Field::Album => "ALBUM",
            Field::Label => "LABEL",
            Field::Remixer => "REMIXER",
            Field::Key => "KEY",
            Field::Comment => "COMMENT",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub field: Field,
    /// The value as stored (with its accents), for display and for the query.
    pub value: String,
    /// How many tracks have this value.
    pub count: usize,
}

impl Suggestion {
    /// Split `value` around the first match of `query` for highlighting.
    /// Returns the whole value in `before` when it does not match.
    pub fn highlight(&self, query: &str) -> (String, String, String) {
        let q = fold(query);
        let q = q.trim();
        let folded = fold(&self.value);
        let Some(byte) = (!q.is_empty()).then(|| folded.find(q)).flatten() else {
            return (self.value.clone(), String::new(), String::new());
        };
        let start = folded[..byte].chars().count();
        let end = start + q.chars().count();
        let chars: Vec<char> = self.value.chars().collect();
        // Folding can change the number of characters (a Hangul syllable becomes two or three
        // jamo, an accent is dropped), so the offsets in the folded text are turned back into
        // positions in the original by folding it one character at a time.
        let mut at = Vec::with_capacity(chars.len() + 1);
        let mut n = 0usize;
        for c in &chars {
            at.push(n);
            n += fold(&c.to_string()).chars().count();
        }
        at.push(n);
        let from = at
            .iter()
            .rposition(|&x| x <= start)
            .unwrap_or(0)
            .min(chars.len());
        let to = at
            .iter()
            .position(|&x| x >= end)
            .unwrap_or(chars.len())
            .clamp(from, chars.len());
        (
            chars[..from].iter().collect(),
            chars[from..to].iter().collect(),
            chars[to..].iter().collect(),
        )
    }
}

#[derive(Debug, Clone)]
struct Row {
    title: String,
    artist: String,
    album: String,
    label: String,
    remixer: String,
    genre: String,
    key: String,
    comment: String,
    tags: Vec<String>,
    /// Everything searchable, folded, joined with ` | `.
    hay: String,
}

/// Folded copies of the searchable fields, built once per library load so a
/// keystroke does not refold 4,000 tracks. Row `i` belongs to `tracks[i]`.
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    rows: Vec<Row>,
}

fn make_row(t: &TrackInfo) -> Row {
    let tags: Vec<String> = t.tags.iter().map(|g| fold(g)).collect();
    let hay = fold(
        &[
            t.title.as_str(),
            &t.artist,
            &t.album,
            &t.label,
            &t.remixer,
            &t.original_artist,
            &t.mix_name,
            &t.genre,
            &t.key,
            &bpm_label(t.bpm),
            &t.comment,
            &t.tags.join(" "),
        ]
        .join(" | "),
    );
    Row {
        title: fold(&t.title),
        artist: fold(&t.artist),
        album: fold(&t.album),
        label: fold(&t.label),
        remixer: fold(&t.remixer),
        genre: fold(&t.genre),
        key: fold(&t.key),
        comment: fold(&t.comment),
        tags,
        hay,
    }
}

impl SearchIndex {
    pub fn new(tracks: &[TrackInfo]) -> Self {
        Self {
            rows: tracks.iter().map(make_row).collect(),
        }
    }

    /// The index with row `i` redone for the edited track `t`. Only that row is folded again;
    /// the others are copied, which is far cheaper than folding every field of every track.
    pub fn with_row(&self, i: usize, t: &TrackInfo) -> Self {
        let mut rows = self.rows.clone();
        if let Some(slot) = rows.get_mut(i) {
            *slot = make_row(t);
        }
        Self { rows }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Indices of the tracks that match every word of `query`, in library order.
    pub fn filter(&self, query: &str) -> Vec<usize> {
        let tokens = tokens(query);
        (0..self.rows.len())
            .filter(|&i| tokens.iter().all(|t| self.rows[i].hay.contains(t.as_str())))
            .collect()
    }

    /// For a track that matched through a field the list does not show, say
    /// where: `COMMENT: Female vocal at bar 33`. `None` when the title, artist,
    /// genre, key or BPM explain the match, or there is no query.
    pub fn match_in(&self, tracks: &[TrackInfo], i: usize, query: &str) -> Option<String> {
        let (row, t) = (self.rows.get(i)?, tracks.get(i)?);
        let seen = fold(
            &[
                t.title.as_str(),
                &t.artist,
                &t.genre,
                &t.key,
                &bpm_label(t.bpm),
            ]
            .join(" "),
        );
        let tag_text = row.tags.join(", ");
        let places: [(&str, &str, String); 5] = [
            ("COMMENT", &row.comment, t.comment.clone()),
            ("TAG", &tag_text, t.tags.join(", ")),
            ("ALBUM", &row.album, t.album.clone()),
            ("LABEL", &row.label, t.label.clone()),
            ("REMIXER", &row.remixer, t.remixer.clone()),
        ];
        for tok in tokens(query) {
            if seen.contains(tok.as_str()) {
                continue;
            }
            for (name, folded, original) in &places {
                if !original.is_empty() && folded.contains(tok.as_str()) {
                    return Some(format!("{name}: {original}"));
                }
            }
        }
        None
    }

    /// Up to 8 values that contain the whole query, grouped by field in
    /// [`Field`] order and by how many tracks share them.
    pub fn suggest(&self, tracks: &[TrackInfo], query: &str) -> Vec<Suggestion> {
        let q = fold(query);
        let q = q.trim();
        if q.is_empty() {
            return Vec::new();
        }
        let mut order: Vec<Suggestion> = Vec::new();
        let mut at: HashMap<(Field, String), usize> = HashMap::new();
        let mut add = |field: Field, folded: &str, original: &str| {
            if original.is_empty() || !folded.contains(q) {
                return;
            }
            let slot = *at.entry((field, original.to_string())).or_insert_with(|| {
                order.push(Suggestion {
                    field,
                    value: original.to_string(),
                    count: 0,
                });
                order.len() - 1
            });
            order[slot].count += 1;
        };
        for (row, t) in self.rows.iter().zip(tracks) {
            for (folded, original) in row.tags.iter().zip(&t.tags) {
                add(Field::Tag, folded, original);
            }
            add(Field::Artist, &row.artist, &t.artist);
            add(Field::Track, &row.title, &t.title);
            add(Field::Genre, &row.genre, &t.genre);
            add(Field::Album, &row.album, &t.album);
            add(Field::Label, &row.label, &t.label);
            add(Field::Remixer, &row.remixer, &t.remixer);
            add(Field::Key, &row.key, &t.key);
            add(Field::Comment, &row.comment, &t.comment);
        }
        order.sort_by(|a, b| a.field.cmp(&b.field).then(b.count.cmp(&a.count)));
        order.truncate(8);
        order
    }
}

fn tokens(query: &str) -> Vec<String> {
    fold(query).split_whitespace().map(str::to_string).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEntry {
    pub value: String,
    pub count: u32,
    pub tick: u64,
}

/// Searches the user picked or submitted, ranked by how often and how lately.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recent {
    entries: Vec<RecentEntry>,
    tick: u64,
}

const KEEP: usize = 30;

impl Recent {
    pub fn add(&mut self, value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        self.tick += 1;
        match self.entries.iter_mut().find(|e| e.value == value) {
            Some(e) => {
                e.count += 1;
                e.tick = self.tick;
            }
            None => self.entries.push(RecentEntry {
                value: value.into(),
                count: 1,
                tick: self.tick,
            }),
        }
        if self.entries.len() > KEEP {
            // Drop the least used, oldest entry.
            self.entries
                .sort_by(|a, b| b.count.cmp(&a.count).then(b.tick.cmp(&a.tick)));
            self.entries.truncate(KEEP);
        }
    }

    /// Most used first, ties by most recent.
    pub fn top(&self, n: usize) -> Vec<&RecentEntry> {
        let mut v: Vec<&RecentEntry> = self.entries.iter().collect();
        v.sort_by(|a, b| b.count.cmp(&a.count).then(b.tick.cmp(&a.tick)));
        v.truncate(n);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: i64, title: &str, artist: &str) -> TrackInfo {
        TrackInfo {
            id,
            title: title.into(),
            artist: artist.into(),
            bpm: 122.0,
            key: "9A".into(),
            ..Default::default()
        }
    }

    fn lib() -> Vec<TrackInfo> {
        let mut a = t(1, "Café Crème", "Élève");
        a.comment = "Female vocal at bar 33".into();
        a.tags = vec!["Vocal".into(), "Warm-up".into()];
        let mut b = t(2, "Deep Night", "Sample Artist");
        b.album = "Night Moves".into();
        b.label = "Sample Label".into();
        b.tags = vec!["Dark".into(), "Warm-up".into()];
        b.bpm = 124.0;
        b.key = "8A".into();
        let mut c = t(3, "Sunrise", "Sample Artist");
        c.genre = "Deep House".into();
        vec![a, b, c]
    }

    #[test]
    fn fold_drops_case_and_accents() {
        assert_eq!(fold("Café CRÈME Ñandú"), "cafe creme nandu");
        assert_eq!(fold("Straße"), "straße");
    }

    #[test]
    fn search_is_accent_insensitive_and_ands_words() {
        let tracks = lib();
        let ix = SearchIndex::new(&tracks);
        assert_eq!(ix.filter("cafe creme"), vec![0]);
        assert_eq!(ix.filter("ÉLÈVE"), vec![0]);
        assert_eq!(ix.filter("sample artist"), vec![1, 2]);
        assert_eq!(ix.filter("sample deep"), vec![1, 2]); // words match in different fields
        assert_eq!(ix.filter("sample deep night"), vec![1]);
        assert_eq!(ix.filter("warm-up"), vec![0, 1]);
        assert_eq!(ix.filter("124.0"), vec![1]);
        assert_eq!(ix.filter("8a"), vec![1]);
        assert_eq!(ix.filter("nothing like this"), Vec::<usize>::new());
        assert_eq!(ix.filter("   "), vec![0, 1, 2]);
        assert_eq!(ix.filter(""), vec![0, 1, 2]);
    }

    #[test]
    fn match_in_names_the_hidden_field() {
        let tracks = lib();
        let ix = SearchIndex::new(&tracks);
        assert_eq!(
            ix.match_in(&tracks, 0, "female").as_deref(),
            Some("COMMENT: Female vocal at bar 33")
        );
        assert_eq!(
            ix.match_in(&tracks, 1, "dark").as_deref(),
            Some("TAG: Dark, Warm-up")
        );
        assert_eq!(
            ix.match_in(&tracks, 1, "moves").as_deref(),
            Some("ALBUM: Night Moves")
        );
        assert_eq!(
            ix.match_in(&tracks, 1, "sample label").as_deref(),
            Some("LABEL: Sample Label")
        );
        // Title, artist, genre, key and BPM are visible, so nothing to explain.
        assert_eq!(ix.match_in(&tracks, 1, "deep"), None);
        assert_eq!(ix.match_in(&tracks, 1, ""), None);
    }

    #[test]
    fn suggestions_group_by_field_then_count() {
        let tracks = lib();
        let ix = SearchIndex::new(&tracks);
        let s = ix.suggest(&tracks, "warm");
        assert_eq!(s.len(), 1);
        assert_eq!(
            (s[0].field, s[0].value.as_str(), s[0].count),
            (Field::Tag, "Warm-up", 2)
        );

        let s = ix.suggest(&tracks, "a");
        // Tags come first; Artist "Sample Artist" (2 tracks) before Élève (no "a").
        assert_eq!(s[0].field, Field::Tag);
        assert!(s
            .windows(2)
            .all(|w| w[0].field < w[1].field
                || (w[0].field == w[1].field && w[0].count >= w[1].count)));
        assert!(s
            .iter()
            .any(|x| x.field == Field::Artist && x.value == "Sample Artist" && x.count == 2));
        assert!(s.len() <= 8);
        assert!(ix.suggest(&tracks, "  ").is_empty());
    }

    #[test]
    fn highlight_splits_around_the_match() {
        let s = Suggestion {
            field: Field::Track,
            value: "Café Crème".into(),
            count: 1,
        };
        assert_eq!(
            s.highlight("CREME"),
            ("Café ".into(), "Crème".into(), String::new())
        );
        assert_eq!(
            s.highlight("af"),
            ("C".into(), "af".into(), "é Crème".into())
        );
        assert_eq!(
            s.highlight("zzz"),
            ("Café Crème".into(), String::new(), String::new())
        );
    }

    #[test]
    fn highlight_survives_text_that_folds_to_a_different_length() {
        let sug = |v: &str| Suggestion {
            field: Field::Track,
            value: v.into(),
            count: 1,
        };
        // Hangul syllables fold to two or three jamo each.
        assert_eq!(
            sug("안녕").highlight("녕"),
            ("안".into(), "녕".into(), String::new())
        );
        assert_eq!(
            sug("안녕").highlight("안"),
            (String::new(), "안".into(), "녕".into())
        );
        // Already decomposed (e + combining acute): the accent folds away, the letter stays whole.
        let decomposed = "Cafe\u{301} Creme";
        let (a, m, b) = sug(decomposed).highlight("cafe");
        assert_eq!(
            format!("{a}{m}{b}"),
            decomposed,
            "nothing is lost or duplicated"
        );
        assert!(m.starts_with("Cafe"), "{m:?}");
        // Whatever the text, it never panics and always reassembles.
        for v in [
            "ﬁne",
            "İstanbul",
            "ǅ",
            "한국어 노래",
            "e\u{301}\u{301}",
            "ß",
        ] {
            for q in ["n", "한", "ss", "i", "e", "노래", "zz"] {
                let (a, m, b) = sug(v).highlight(q);
                assert_eq!(format!("{a}{m}{b}"), v, "{v:?} / {q:?}");
            }
        }
    }

    #[test]
    fn redoing_one_row_gives_the_same_index_as_building_it_again() {
        let mk = |id: i64, title: &str, comment: &str| -> TrackInfo {
            serde_json::from_value(serde_json::json!({"id": id, "title": title, "comment": comment, "bpm": 120.0, "key": "8A"})).unwrap()
        };
        let mut tracks = vec![
            mk(1, "Alpha", "x"),
            mk(2, "Bravo", "y"),
            mk(3, "Charlie", "z"),
        ];
        let before = SearchIndex::new(&tracks);
        tracks[1] = mk(2, "Bravo", "edited comment");
        let fast = before.with_row(1, &tracks[1]);
        let full = SearchIndex::new(&tracks);
        for q in [
            "edited",
            "bravo",
            "alpha",
            "y",
            "comment edited",
            "charlie",
            "zzz",
            "",
        ] {
            assert_eq!(fast.filter(q), full.filter(q), "{q:?}");
        }
        assert_eq!(fast.len(), 3);
        assert_eq!(
            before.filter("edited"),
            Vec::<usize>::new(),
            "the old index is not touched"
        );
        // an index past the end is ignored rather than a panic
        assert_eq!(before.with_row(9, &tracks[0]).len(), 3);
    }

    #[test]
    fn recent_ranks_by_count_then_recency() {
        let mut r = Recent::default();
        for v in [
            "Warm-up",
            "Warm-up",
            "Warm-up",
            "Sample Artist",
            "Sample Artist",
            "9A",
        ] {
            r.add(v);
        }
        let top: Vec<&str> = r.top(3).iter().map(|e| e.value.as_str()).collect();
        assert_eq!(top, vec!["Warm-up", "Sample Artist", "9A"]);
        r.add("9A");
        r.add("9A"); // 3 uses, newer than Warm-up's third
        let top: Vec<&str> = r.top(2).iter().map(|e| e.value.as_str()).collect();
        assert_eq!(top, vec!["9A", "Warm-up"]);
        r.add("  ");
        assert_eq!(r.top(10).len(), 3);
    }

    #[test]
    fn recent_stays_bounded_and_round_trips() {
        let mut r = Recent::default();
        for i in 0..100 {
            r.add(&format!("q{i}"));
        }
        assert_eq!(r.top(100).len(), 30);
        let back: Recent = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }
}
