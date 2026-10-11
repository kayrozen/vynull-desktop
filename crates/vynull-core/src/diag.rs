//! The diagnostics screen: turning the daemon's log lines and counters into
//! what it shows.

use vynull_api::LogLine;

/// Which part of the daemon a log line is about, for the filter pills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    All,
    Dbserver,
    Nfs,
    Analysis,
    Link,
}

impl Source {
    pub const ALL: [Source; 5] = [
        Source::All,
        Source::Dbserver,
        Source::Nfs,
        Source::Analysis,
        Source::Link,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Source::All => "ALL",
            Source::Dbserver => "DBSERVER",
            Source::Nfs => "NFS",
            Source::Analysis => "ANALYSIS",
            Source::Link => "LINK",
        }
    }

    /// Words that put a line under this source. The daemon does not tag its
    /// lines, so this is a match on its own wording (`nfs: ...`, `dbserver error: ...`,
    /// `device: LINK on`, `analysis(pending=...)`).
    fn words(self) -> &'static [&'static str] {
        match self {
            Source::All => &[],
            Source::Dbserver => &["dbserver"],
            Source::Nfs => &["nfs", "portmap"],
            Source::Analysis => &["analysis", "analyz"],
            Source::Link => &["link", "device", "peer", "announce"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn class(self) -> &'static str {
        match self {
            Level::Info => "ln",
            Level::Warn => "ln wn",
            Level::Error => "ln er",
        }
    }
}

/// A log line ready to show.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub seq: i64,
    /// Unix milliseconds.
    pub time: i64,
    /// The line without the `2026/10/11 09:21:40 ` stamp the daemon puts first.
    pub text: String,
    pub level: Level,
}

/// Drop a leading `YYYY/MM/DD HH:MM:SS ` stamp.
pub fn strip_stamp(line: &str) -> &str {
    let b = line.as_bytes();
    let stamped = b.len() > 20
        && b[4] == b'/'
        && b[7] == b'/'
        && b[10] == b' '
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b' '
        && b[..4].iter().all(u8::is_ascii_digit);
    if stamped {
        &line[20..]
    } else {
        line
    }
}

pub fn level_of(text: &str) -> Level {
    let t = text.to_ascii_lowercase();
    if t.contains("error")
        || t.contains("failed")
        || t.contains("panic")
        || t.contains("cannot")
        || t.contains("not found")
    {
        Level::Error
    } else if t.contains("warn")
        || t.contains("missing")
        || t.contains("unlink")
        || t.contains("timeout")
    {
        Level::Warn
    } else {
        Level::Info
    }
}

pub fn entry(l: &LogLine) -> Entry {
    let text = strip_stamp(&l.line).to_string();
    Entry {
        seq: l.seq,
        time: l.time,
        level: level_of(&text),
        text,
    }
}

/// The lines of `log` that pass the filters, in order.
pub fn filter<'a>(
    log: &'a [Entry],
    text: &str,
    errors_only: bool,
    source: Source,
) -> Vec<&'a Entry> {
    let needle = text.trim().to_lowercase();
    log.iter()
        .filter(|e| !errors_only || e.level == Level::Error)
        .filter(|e| {
            let t = e.text.to_lowercase();
            (needle.is_empty() || t.contains(&needle))
                && (source == Source::All || source.words().iter().any(|w| t.contains(w)))
        })
        .collect()
}

/// Add new lines to `log`, ignoring any already seen, and keep the last `keep`.
pub fn append(log: &mut Vec<Entry>, new: &[LogLine], keep: usize) {
    let last = log.last().map_or(0, |e| e.seq);
    log.extend(new.iter().filter(|l| l.seq > last).map(entry));
    if log.len() > keep {
        log.drain(..log.len() - keep);
    }
}

/// `3h 12m`, `5m 4s`, `42s`.
pub fn uptime_label(secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// `1.4 GB`, `512 MB`, `0 B`.
pub fn bytes_label(n: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n.max(0) as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 || v >= 100.0 {
        format!("{v:.0} {}", UNITS[u])
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

/// Share of the analysis work that is done, 0 to 100.
pub fn analysis_percent(pending: i64, analyzed: i64) -> f64 {
    let total = pending + analyzed;
    if total <= 0 {
        100.0
    } else {
        analyzed as f64 / total as f64 * 100.0
    }
}

/// `1,123`
pub fn thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(seq: i64, text: &str) -> LogLine {
        LogLine {
            seq,
            time: seq * 1000,
            line: text.to_string(),
        }
    }

    #[test]
    fn stamps_and_levels() {
        assert_eq!(
            strip_stamp("2026/10/11 09:21:40 started with args"),
            "started with args"
        );
        assert_eq!(strip_stamp("no stamp here"), "no stamp here");
        assert_eq!(strip_stamp("2026/10/11"), "2026/10/11");
        assert_eq!(level_of("nfs: file not found for track 31"), Level::Error);
        assert_eq!(level_of("dbserver error: bind"), Level::Error);
        assert_eq!(level_of("dbserver: unlink closed 2 sessions"), Level::Warn);
        assert_eq!(level_of("analysis: track 14 queued"), Level::Info);
    }

    #[test]
    fn filters_combine() {
        let mut log = Vec::new();
        append(
            &mut log,
            &[
                line(1, "2026/10/11 09:00:00 link: player 1 playing"),
                line(2, "2026/10/11 09:00:01 nfs: read track 2"),
                line(3, "2026/10/11 09:00:02 nfs: file not found for track 31"),
                line(4, "2026/10/11 09:00:03 analysis: track 14 queued"),
                line(5, "2026/10/11 09:00:04 dbserver error: bind discovery port"),
            ],
            100,
        );
        let texts = |v: Vec<&Entry>| v.iter().map(|e| e.seq).collect::<Vec<_>>();
        assert_eq!(
            texts(filter(&log, "", false, Source::All)),
            vec![1, 2, 3, 4, 5]
        );
        assert_eq!(texts(filter(&log, "", true, Source::All)), vec![3, 5]);
        assert_eq!(texts(filter(&log, "", false, Source::Nfs)), vec![2, 3]);
        assert_eq!(texts(filter(&log, "TRACK", false, Source::Nfs)), vec![2, 3]);
        assert_eq!(texts(filter(&log, "31", true, Source::Nfs)), vec![3]);
        assert_eq!(texts(filter(&log, "  ", false, Source::Analysis)), vec![4]);
        assert_eq!(
            texts(filter(&log, "bind", false, Source::Dbserver)),
            vec![5]
        );
        assert_eq!(texts(filter(&log, "player", false, Source::Link)), vec![1]);
    }

    #[test]
    fn append_skips_seen_lines_and_keeps_the_tail() {
        let mut log = Vec::new();
        append(&mut log, &[line(1, "a"), line(2, "b")], 3);
        append(&mut log, &[line(2, "b"), line(3, "c"), line(4, "d")], 3);
        assert_eq!(log.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![2, 3, 4]);
        append(&mut log, &[], 3);
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn labels() {
        assert_eq!(uptime_label(3 * 3600 + 12 * 60 + 5), "3h 12m");
        assert_eq!(uptime_label(304), "5m 4s");
        assert_eq!(uptime_label(42), "42s");
        assert_eq!(uptime_label(-5), "0s");
        assert_eq!(bytes_label(0), "0 B");
        assert_eq!(bytes_label(1_500_000_000), "1.4 GB");
        assert_eq!(bytes_label(512 * 1024 * 1024), "512 MB");
        assert_eq!(thousands(3910), "3,910");
        assert_eq!(thousands(12), "12");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(analysis_percent(12, 3880), 3880.0 / 3892.0 * 100.0);
        assert_eq!(analysis_percent(0, 0), 100.0);
    }
}
