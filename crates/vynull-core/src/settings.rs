//! The settings screen: grouping and naming the daemon's player settings, and
//! editing them in the whole-config JSON the API wants back.
//!
//! `GET /api/settings` returns the config under `full`; `POST` replaces it as a
//! whole, so an edit is "copy `full`, change one value, send all of it".

use serde_json::Value;

/// Which part of the config a setting lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sub {
    My,
    My2,
    Dev,
    Djm,
}

impl Sub {
    pub fn key(self) -> &'static str {
        match self {
            Sub::My => "my_setting",
            Sub::My2 => "my_setting_2",
            Sub::Dev => "dev_setting",
            Sub::Djm => "djm_my_setting",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Display,
    Playback,
    Other,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Display => "DISPLAY",
            Group::Playback => "PLAYBACK",
            Group::Other => "OTHER",
        }
    }
}

/// Settings in the order the screen lists them, per group. Anything the daemon
/// has that is not here is listed last, under OTHER.
const DISPLAY: [&str; 17] = [
    "waveform_color",
    "overview_type",
    "key_display",
    "wave_position",
    "time_mode",
    "lcd_brightness",
    "jog_lcd_brightness",
    "jog_display_mode",
    "jog_ring_brightness",
    "jog_ring_indicator",
    "on_air_display",
    "phase_meter",
    "waveform",
    "waveform_divisions",
    "pad_button_brightness",
    "disc_slot_illumination",
    "language",
];
const PLAYBACK: [&str; 16] = [
    "quantize",
    "quantize_beat_value",
    "auto_cue",
    "auto_cue_level",
    "sync",
    "master_tempo",
    "tempo_range",
    "jog_mode",
    "vinyl_speed_adjust",
    "beat_jump_beat_value",
    "play_mode",
    "hotcue_autoload",
    "hotcue_color",
    "slip_flashing",
    "needle_lock",
    "eject_lock",
];

pub fn group_of(key: &str) -> Group {
    if DISPLAY.contains(&key) {
        Group::Display
    } else if PLAYBACK.contains(&key) {
        Group::Playback
    } else {
        Group::Other
    }
}

/// `on_air_display` as `On air display`.
pub fn key_label(key: &str) -> String {
    let s = key.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// `sixteen` as `16`, `minus_36db` as `-36 DB`, `steep_top` as `STEEP TOP`.
pub fn option_label(v: &str) -> String {
    let n = match v {
        "one" => Some("1"),
        "two" => Some("2"),
        "three" => Some("3"),
        "four" => Some("4"),
        "five" => Some("5"),
        "six" => Some("6"),
        "eight" => Some("8"),
        "ten" => Some("10"),
        "sixteen" => Some("16"),
        "thirty_two" => Some("32"),
        "sixty_four" => Some("64"),
        "half" => Some("1/2"),
        _ => None,
    };
    if let Some(n) = n {
        return n.to_string();
    }
    if let Some(db) = v.strip_prefix("minus_").and_then(|r| r.strip_suffix("db")) {
        if db.chars().all(|c| c.is_ascii_digit()) {
            return format!("-{db} DB");
        }
    }
    v.replace('_', " ").to_uppercase()
}

/// Which sub-object of `full` holds `key`, if any. A key can only be in one.
pub fn locate(full: &Value, key: &str) -> Option<Sub> {
    [Sub::My, Sub::My2, Sub::Dev, Sub::Djm]
        .into_iter()
        .find(|s| full.get(s.key()).is_some_and(|o| o.get(key).is_some()))
}

/// `(key, value)` pairs of one sub-object, as strings.
pub fn entries(full: &Value, sub: Sub) -> Vec<(String, String)> {
    full.get(sub.key())
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.as_str().map_or_else(|| v.to_string(), str::to_string),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The editable player settings by group: `my_setting` and `my_setting_2`, known
/// ones first in the mockup's order. `dev_setting` and `djm_my_setting` are
/// read-only (plan decision) and are not here.
pub fn player_settings(full: &Value) -> Vec<(Group, Vec<(String, String)>)> {
    let mut all: Vec<(String, String)> = Vec::new();
    for sub in [Sub::My, Sub::My2] {
        all.extend(entries(full, sub));
    }
    let mut out = Vec::new();
    for g in [Group::Display, Group::Playback, Group::Other] {
        let order: &[&str] = match g {
            Group::Display => &DISPLAY,
            Group::Playback => &PLAYBACK,
            Group::Other => &[],
        };
        let mut rows: Vec<(String, String)> = order
            .iter()
            .filter_map(|k| all.iter().find(|(a, _)| a == k).cloned())
            .collect();
        if g == Group::Other {
            rows = all
                .iter()
                .filter(|(k, _)| group_of(k) == Group::Other)
                .cloned()
                .collect();
        }
        if !rows.is_empty() {
            out.push((g, rows));
        }
    }
    out
}

/// A copy of `full` with `key` set to `value`, or `None` if `key` is unknown.
pub fn with_value(full: &Value, key: &str, value: &str) -> Option<Value> {
    let sub = locate(full, key)?;
    let mut next = full.clone();
    next.get_mut(sub.key())?
        .as_object_mut()?
        .insert(key.to_string(), Value::String(value.to_string()));
    Some(next)
}

/// The analysis tempo range in `full`.
pub fn bpm_range(full: &Value) -> (i64, i64) {
    let g = |k: &str, d: i64| full.get(k).and_then(Value::as_i64).unwrap_or(d);
    (g("bpm_range_min", 60), g("bpm_range_max", 200))
}

/// A copy of `full` with a new tempo range.
pub fn with_bpm_range(full: &Value, min: i64, max: i64) -> Value {
    let mut next = full.clone();
    if let Some(o) = next.as_object_mut() {
        o.insert("bpm_range_min".into(), Value::from(min));
        o.insert("bpm_range_max".into(), Value::from(max));
    }
    next
}

pub const BPM_LIMITS: (i64, i64) = (60, 200);
const MIN_GAP: i64 = 10;

/// Which end of the range the user moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Min,
    Max,
}

/// The range after moving one end to `value`: kept inside 60 to 200 and at
/// least 10 BPM wide, the moved end stopping against the other.
pub fn move_range_end(min: i64, max: i64, end: End, value: i64) -> (i64, i64) {
    let v = value.clamp(BPM_LIMITS.0, BPM_LIMITS.1);
    match end {
        End::Min => (v.min(max - MIN_GAP).max(BPM_LIMITS.0), max),
        End::Max => (min, v.max(min + MIN_GAP).min(BPM_LIMITS.1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full() -> Value {
        json!({
            "my_setting": {"quantize": "on", "time_mode": "remain", "language": "english", "zzz_new": "x"},
            "my_setting_2": {"beat_jump_beat_value": "sixteen", "vinyl_speed_adjust": "touch"},
            "dev_setting": {"waveform_color": "blue", "overview_type": "half"},
            "djm_my_setting": {"cross_fader_curve": "fast_cut"},
            "track_detail": "artist", "bpm_range_min": 100, "bpm_range_max": 200
        })
    }

    #[test]
    fn labels() {
        assert_eq!(key_label("on_air_display"), "On air display");
        assert_eq!(option_label("sixteen"), "16");
        assert_eq!(option_label("thirty_two"), "32");
        assert_eq!(option_label("three"), "3");
        assert_eq!(option_label("minus_36db"), "-36 DB");
        assert_eq!(option_label("steep_top"), "STEEP TOP");
        assert_eq!(option_label("on"), "ON");
        assert_eq!(option_label("half"), "1/2");
    }

    #[test]
    fn groups_keep_the_mockup_order_and_collect_the_rest() {
        let g = player_settings(&full());
        let names: Vec<_> = g.iter().map(|(g, _)| g.label()).collect();
        assert_eq!(names, vec!["DISPLAY", "PLAYBACK", "OTHER"]);
        let keys = |i: usize| g[i].1.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>();
        assert_eq!(keys(0), vec!["time_mode", "language"]);
        assert_eq!(
            keys(1),
            vec!["quantize", "vinyl_speed_adjust", "beat_jump_beat_value"]
        );
        assert_eq!(keys(2), vec!["zzz_new"]);
        assert_eq!(g[0].1[0].1, "remain");
        assert!(
            g.iter().all(|(_, rows)| rows
                .iter()
                .all(|(k, _)| k != "cross_fader_curve" && k != "waveform_color")),
            "djm and dev are read-only"
        );
    }

    #[test]
    fn editing_one_value_keeps_the_rest_of_the_config() {
        let f = full();
        assert_eq!(locate(&f, "waveform_color"), Some(Sub::Dev));
        assert_eq!(locate(&f, "quantize"), Some(Sub::My));
        assert_eq!(locate(&f, "vinyl_speed_adjust"), Some(Sub::My2));
        assert_eq!(locate(&f, "nope"), None);
        let n = with_value(&f, "waveform_color", "rgb").unwrap();
        assert_eq!(n["dev_setting"]["waveform_color"], "rgb");
        assert_eq!(n["dev_setting"]["overview_type"], "half");
        assert_eq!(n["bpm_range_min"], 100);
        assert_eq!(n["track_detail"], "artist");
        assert_eq!(
            f["dev_setting"]["waveform_color"], "blue",
            "the original is untouched"
        );
        assert!(with_value(&f, "nope", "x").is_none());
    }

    #[test]
    fn tempo_range() {
        let f = full();
        assert_eq!(bpm_range(&f), (100, 200));
        assert_eq!(bpm_range(&json!({})), (60, 200));
        let n = with_bpm_range(&f, 90, 180);
        assert_eq!(bpm_range(&n), (90, 180));
        assert_eq!(n["dev_setting"]["overview_type"], "half");
        assert_eq!(move_range_end(90, 180, End::Min, 30), (60, 180));
        assert_eq!(
            move_range_end(90, 180, End::Min, 175),
            (170, 180),
            "stops 10 below the top"
        );
        assert_eq!(
            move_range_end(90, 180, End::Max, 95),
            (90, 100),
            "stops 10 above the bottom"
        );
        assert_eq!(move_range_end(90, 180, End::Max, 500), (90, 200));
    }

    #[test]
    fn entries_stringify_odd_values() {
        let f = json!({"dev_setting": {"a": "x", "b": 3, "c": true}});
        let e = entries(&f, Sub::Dev);
        assert_eq!(
            e,
            vec![
                ("a".into(), "x".into()),
                ("b".into(), "3".into()),
                ("c".into(), "true".into())
            ]
        );
        assert!(entries(&f, Sub::Djm).is_empty());
    }
}
