//! Smart playlist rules: what the editor offers and the JSON the daemon expects
//! (`api/smart_playlist.go` in the daemon: fields, operators and value shapes).

use serde_json::{json, Value};
use vynull_api::{SmartCond, SmartRules};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Number,
    Text,
    Key,
    Tag,
}

/// `(field, label, kind)`.
pub const FIELDS: [(&str, &str, Kind); 15] = [
    ("bpm", "BPM", Kind::Number),
    ("year", "YEAR", Kind::Number),
    ("rating", "RATING", Kind::Number),
    ("play_count", "PLAY COUNT", Kind::Number),
    ("duration_sec", "LENGTH (S)", Kind::Number),
    ("color", "COLOR", Kind::Number),
    ("title", "TITLE", Kind::Text),
    ("artist", "ARTIST", Kind::Text),
    ("album", "ALBUM", Kind::Text),
    ("genre", "GENRE", Kind::Text),
    ("label", "LABEL", Kind::Text),
    ("remixer", "REMIXER", Kind::Text),
    ("comment", "COMMENT", Kind::Text),
    ("key", "KEY", Kind::Key),
    ("tag", "TAG", Kind::Tag),
];

pub fn kind_of(field: &str) -> Kind {
    FIELDS
        .iter()
        .find(|f| f.0 == field)
        .map_or(Kind::Text, |f| f.2)
}

/// `(operator, label)` choices for a kind of field.
pub fn operators(kind: Kind) -> &'static [(&'static str, &'static str)] {
    match kind {
        Kind::Number => &[
            ("eq", "="),
            ("ne", "≠"),
            ("gt", ">"),
            ("gte", "≥"),
            ("lt", "<"),
            ("lte", "≤"),
            ("between", "BETWEEN"),
        ],
        Kind::Text => &[
            ("contains", "CONTAINS"),
            ("not_contains", "DOES NOT CONTAIN"),
            ("is", "IS"),
            ("is_not", "IS NOT"),
            ("starts_with", "STARTS WITH"),
            ("ends_with", "ENDS WITH"),
            ("is_empty", "IS EMPTY"),
            ("not_empty", "IS NOT EMPTY"),
        ],
        Kind::Key => &[
            ("is", "IS"),
            ("is_not", "IS NOT"),
            ("compatible", "MIXES WITH"),
        ],
        Kind::Tag => &[
            ("has", "HAS"),
            ("not_has", "HAS NOT"),
            ("has_any", "HAS ANY TAG"),
            ("none", "HAS NO TAG"),
        ],
    }
}

/// Operators that need no value.
pub fn takes_value(op: &str) -> bool {
    !matches!(op, "is_empty" | "not_empty" | "has_any" | "none")
}

/// One row of the editor, as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub field: String,
    pub op: String,
    /// The value, or the low end of a range.
    pub a: String,
    /// The high end of a range.
    pub b: String,
}

impl Default for Row {
    fn default() -> Self {
        Row {
            field: "bpm".into(),
            op: "gte".into(),
            a: String::new(),
            b: String::new(),
        }
    }
}

impl Row {
    /// The operator for a field of this kind if the current one does not apply.
    pub fn fix_operator(&mut self) {
        let ops = operators(kind_of(&self.field));
        if !ops.iter().any(|o| o.0 == self.op) {
            self.op = ops[0].0.to_string();
        }
    }

    /// The condition for the daemon, or `None` while the row is incomplete or invalid.
    pub fn to_cond(&self) -> Option<SmartCond> {
        let kind = kind_of(&self.field);
        if !operators(kind).iter().any(|o| o.0 == self.op) {
            return None;
        }
        let value: Value = if !takes_value(&self.op) {
            Value::Null
        } else if kind == Kind::Number && self.op == "between" {
            let (lo, hi) = (
                self.a.trim().parse::<f64>().ok()?,
                self.b.trim().parse::<f64>().ok()?,
            );
            json!([lo.min(hi), lo.max(hi)])
        } else if kind == Kind::Number {
            json!(self.a.trim().parse::<f64>().ok()?)
        } else {
            let v = self.a.trim();
            if v.is_empty() {
                return None;
            }
            json!(v)
        };
        Some(SmartCond {
            group: None,
            field: self.field.clone(),
            operator: self.op.clone(),
            value,
        })
    }
}

/// Rules from the editor rows. `None` when no row is complete: the daemon would
/// treat empty rules as "every track", which is never what the user means.
pub fn build_rules(all: bool, rows: &[Row]) -> Option<SmartRules> {
    let conditions: Vec<SmartCond> = rows.iter().filter_map(Row::to_cond).collect();
    if conditions.is_empty() {
        return None;
    }
    Some(SmartRules {
        combinator: if all { "all" } else { "any" }.into(),
        conditions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(field: &str, op: &str, a: &str, b: &str) -> Row {
        Row {
            field: field.into(),
            op: op.into(),
            a: a.into(),
            b: b.into(),
        }
    }

    #[test]
    fn numbers_ranges_and_text() {
        assert_eq!(
            row("bpm", "gte", " 124 ", "").to_cond().unwrap().value,
            json!(124.0)
        );
        assert_eq!(
            row("bpm", "between", "128", "120").to_cond().unwrap().value,
            json!([120.0, 128.0])
        );
        assert_eq!(
            row("genre", "contains", "House", "")
                .to_cond()
                .unwrap()
                .value,
            json!("House")
        );
        assert_eq!(
            row("tag", "has_any", "", "").to_cond().unwrap().value,
            Value::Null
        );
        assert_eq!(
            row("key", "compatible", "9A", "").to_cond().unwrap().field,
            "key"
        );
    }

    #[test]
    fn incomplete_or_invalid_rows_are_dropped() {
        assert!(row("bpm", "gte", "", "").to_cond().is_none());
        assert!(row("bpm", "gte", "fast", "").to_cond().is_none());
        assert!(row("bpm", "between", "120", "").to_cond().is_none());
        assert!(row("genre", "contains", "  ", "").to_cond().is_none());
        assert!(
            row("genre", "gte", "x", "").to_cond().is_none(),
            "operator does not fit the field"
        );
    }

    #[test]
    fn changing_the_field_repairs_the_operator() {
        let mut r = row("genre", "contains", "x", "");
        r.field = "bpm".into();
        r.fix_operator();
        assert_eq!(r.op, "eq");
        r.field = "key".into();
        r.fix_operator();
        assert_eq!(r.op, "is");
    }

    #[test]
    fn empty_rules_are_refused() {
        assert!(build_rules(true, &[]).is_none());
        assert!(build_rules(true, &[row("bpm", "gte", "", "")]).is_none());
        let r = build_rules(
            false,
            &[row("bpm", "gte", "124", ""), row("key", "is", "8A", "")],
        )
        .unwrap();
        assert_eq!((r.combinator.as_str(), r.conditions.len()), ("any", 2));
        // The JSON matches the daemon's shape: no `group`, value only when needed.
        let j = serde_json::to_value(&r.conditions[0]).unwrap();
        assert_eq!(j["field"], "bpm");
        assert_eq!(j["operator"], "gte");
    }
}
