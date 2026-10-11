//! Library list columns: order, visibility and widths. The UI turns this into
//! CSS custom properties on the list, so reordering never rebuilds rows.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Col {
    Color,
    Art,
    Track,
    Comment,
    Overview,
    Genre,
    Bpm,
    Key,
    Time,
    Rating,
    Load,
}

impl Col {
    pub const ALL: [Col; 11] = [
        Col::Color,
        Col::Art,
        Col::Track,
        Col::Comment,
        Col::Overview,
        Col::Genre,
        Col::Bpm,
        Col::Key,
        Col::Time,
        Col::Rating,
        Col::Load,
    ];

    /// Name used in the CSS custom properties (`--o-bpm`, `--v-bpm`, `--w-bpm`).
    pub fn css(self) -> &'static str {
        match self {
            Col::Color => "color",
            Col::Art => "art",
            Col::Track => "track",
            Col::Comment => "comment",
            Col::Overview => "overview",
            Col::Genre => "genre",
            Col::Bpm => "bpm",
            Col::Key => "key",
            Col::Time => "time",
            Col::Rating => "rating",
            Col::Load => "load",
        }
    }

    /// Header text.
    pub fn header(self) -> &'static str {
        match self {
            Col::Color => "CLR",
            Col::Art => "ART",
            Col::Track => "TRACK",
            Col::Comment => "COMMENT",
            Col::Overview => "OVERVIEW",
            Col::Genre => "GENRE",
            Col::Bpm => "BPM",
            Col::Key => "KEY",
            Col::Time => "TIME",
            Col::Rating => "RATING",
            Col::Load => "LOAD",
        }
    }

    /// Name in the show/hide panel.
    pub fn panel_name(self) -> &'static str {
        match self {
            Col::Color => "COLOR",
            Col::Art => "ARTWORK",
            other => other.header(),
        }
    }

    /// CSS class of the cell and header.
    pub fn class(self) -> &'static str {
        match self {
            Col::Color => "c-cl",
            Col::Art => "c-a",
            Col::Track => "c-t",
            Col::Comment => "c-cm",
            Col::Overview => "c-w",
            Col::Genre => "c-g",
            Col::Bpm => "c-b",
            Col::Key => "c-k",
            Col::Time => "c-m",
            Col::Rating => "c-r",
            Col::Load => "c-ld",
        }
    }

    /// Default width in px for the columns that can be resized.
    pub fn default_width(self) -> Option<u32> {
        match self {
            Col::Comment => Some(220),
            Col::Overview => Some(130),
            Col::Genre => Some(110),
            Col::Bpm => Some(60),
            Col::Key | Col::Time => Some(56),
            Col::Rating => Some(70),
            _ => None,
        }
    }

    pub fn resizable(self) -> bool {
        self.default_width().is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Columns {
    pub order: Vec<Col>,
    #[serde(default)]
    pub hidden: BTreeSet<Col>,
    #[serde(default)]
    pub widths: BTreeMap<Col, u32>,
}

const MIN_W: u32 = 40;
const MAX_W: u32 = 600;

impl Default for Columns {
    fn default() -> Self {
        Self {
            order: Col::ALL.to_vec(),
            hidden: BTreeSet::new(),
            widths: BTreeMap::new(),
        }
    }
}

impl Columns {
    /// Repair a stored value after a release added or removed columns:
    /// drop duplicates, append missing columns, and keep `Track` visible.
    pub fn normalized(mut self) -> Self {
        let mut seen = BTreeSet::new();
        self.order.retain(|c| seen.insert(*c));
        for c in Col::ALL {
            if !seen.contains(&c) {
                self.order.push(c);
            }
        }
        self.hidden.remove(&Col::Track);
        self
    }

    pub fn visible(&self) -> impl Iterator<Item = Col> + '_ {
        self.order
            .iter()
            .copied()
            .filter(|c| !self.hidden.contains(c))
    }

    pub fn is_hidden(&self, c: Col) -> bool {
        self.hidden.contains(&c)
    }

    /// Move `from` to the position `to` currently holds.
    pub fn move_to(&mut self, from: Col, to: Col) {
        let (Some(f), Some(t)) = (
            self.order.iter().position(|c| *c == from),
            self.order.iter().position(|c| *c == to),
        ) else {
            return;
        };
        if f != t {
            let c = self.order.remove(f);
            self.order.insert(t, c);
        }
    }

    /// Move one place left (`-1`) or right (`1`).
    pub fn step(&mut self, c: Col, delta: isize) {
        let Some(i) = self.order.iter().position(|x| *x == c) else {
            return;
        };
        let j = i as isize + delta;
        if (0..self.order.len() as isize).contains(&j) {
            self.order.swap(i, j as usize);
        }
    }

    /// Show or hide a column. The track column cannot be hidden.
    pub fn toggle(&mut self, c: Col) {
        if c == Col::Track {
            return;
        }
        if !self.hidden.remove(&c) {
            self.hidden.insert(c);
        }
    }

    pub fn width(&self, c: Col) -> Option<u32> {
        self.widths.get(&c).copied().or_else(|| c.default_width())
    }

    pub fn set_width(&mut self, c: Col, px: i64) {
        if c.resizable() {
            self.widths
                .insert(c, px.clamp(MIN_W as i64, MAX_W as i64) as u32);
        }
    }

    pub fn reset_width(&mut self, c: Col) {
        self.widths.remove(&c);
    }

    /// CSS custom properties for the list container. The load column is wider
    /// in the queue view, which has more buttons.
    pub fn css_vars(&self, load_width: u32) -> String {
        let mut parts: Vec<String> = self
            .order
            .iter()
            .enumerate()
            .map(|(i, c)| format!("--o-{}:{}", c.css(), i + 1))
            .collect();
        parts.extend(self.hidden.iter().map(|c| format!("--v-{}:none", c.css())));
        parts.extend(
            Col::ALL
                .iter()
                .filter_map(|c| self.width(*c).map(|w| format!("--w-{}:{w}px", c.css()))),
        );
        parts.push(format!("--w-load:{load_width}px"));
        parts.join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_step_and_toggle() {
        let mut c = Columns::default();
        c.move_to(Col::Rating, Col::Comment);
        assert_eq!(c.order[3], Col::Rating);
        assert_eq!(c.order[4], Col::Comment);
        c.step(Col::Rating, -1);
        assert_eq!(c.order[2], Col::Rating);
        c.step(Col::Color, -1); // already first
        assert_eq!(c.order[0], Col::Color);
        c.toggle(Col::Genre);
        assert!(c.is_hidden(Col::Genre));
        assert!(!c.visible().any(|x| x == Col::Genre));
        c.toggle(Col::Genre);
        assert!(!c.is_hidden(Col::Genre));
        c.toggle(Col::Track);
        assert!(!c.is_hidden(Col::Track), "the track column stays");
    }

    #[test]
    fn widths_clamp_reset_and_ignore_fixed_columns() {
        let mut c = Columns::default();
        assert_eq!(c.width(Col::Comment), Some(220));
        c.set_width(Col::Comment, 5);
        assert_eq!(c.width(Col::Comment), Some(40));
        c.set_width(Col::Comment, 9999);
        assert_eq!(c.width(Col::Comment), Some(600));
        c.reset_width(Col::Comment);
        assert_eq!(c.width(Col::Comment), Some(220));
        c.set_width(Col::Art, 300);
        assert_eq!(c.width(Col::Art), None);
    }

    #[test]
    fn css_vars_carry_order_hidden_and_widths() {
        let mut c = Columns::default();
        c.toggle(Col::Overview);
        c.set_width(Col::Bpm, 80);
        let v = c.css_vars(104);
        assert!(v.starts_with("--o-color:1;--o-art:2;--o-track:3;"));
        assert!(v.contains("--v-overview:none"));
        assert!(v.contains("--w-bpm:80px"));
        assert!(v.contains("--w-comment:220px"));
        assert!(v.ends_with("--w-load:104px"));
    }

    #[test]
    fn stored_values_are_repaired() {
        let stored =
            r#"{"order":["bpm","bpm","track"],"hidden":["track","genre"],"widths":{"bpm":70}}"#;
        let c: Columns = serde_json::from_str(stored).unwrap();
        let c = c.normalized();
        assert_eq!(c.order.len(), Col::ALL.len());
        assert_eq!(&c.order[..2], &[Col::Bpm, Col::Track]);
        assert!(!c.is_hidden(Col::Track));
        assert!(c.is_hidden(Col::Genre));
        assert_eq!(c.width(Col::Bpm), Some(70));
    }
}
