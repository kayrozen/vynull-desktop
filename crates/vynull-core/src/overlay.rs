//! The stream overlay settings: what the editor offers. The daemon sanitises whatever it is sent, but the editor keeps
//! to the same ranges so a click never asks for something invalid.

pub const POSITIONS: [(&str, &str); 6] = [
    ("top-left", "TOP LEFT"),
    ("top-center", "TOP CENTER"),
    ("top-right", "TOP RIGHT"),
    ("bottom-left", "BOTTOM LEFT"),
    ("bottom-center", "BOTTOM CENTER"),
    ("bottom-right", "BOTTOM RIGHT"),
];

/// `(name, hex)`.
pub const ACCENTS: [(&str, &str); 6] = [
    ("Orange", "#FF7714"),
    ("Amber", "#FFC15A"),
    ("Teal", "#3FD6C0"),
    ("Green", "#5FD38D"),
    ("Purple", "#B07CFF"),
    ("Pink", "#FF8AD0"),
];

pub const WIDTH: (i64, i64, i64) = (280, 1000, 20); // min, max, step
pub const HISTORY: (i64, i64, i64) = (1, 10, 1);
pub const BAND: (i64, i64, i64) = (40, 300, 20);

/// `value` moved by `steps` of `range`, kept inside it.
pub fn step(value: i64, steps: i64, range: (i64, i64, i64)) -> i64 {
    (value + steps * range.2).clamp(range.0, range.1)
}

/// Whether two hex colors are the same, ignoring case.
pub fn same_color(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The overlay page the daemon serves.
pub fn url(base: &str) -> String {
    format!("{}/overlay", base.trim_end_matches('/'))
}

/// The daemon's overlay page with a `<base>` so its relative requests (`/api/...`) go to the daemon
/// when the page is shown from somewhere else, such as an `iframe` `srcdoc` in the app.
pub fn with_base(html: &str, base: &str) -> String {
    let base = base
        .trim_end_matches('/')
        .replace('&', "&amp;")
        .replace('"', "&quot;");
    let tag = format!("<base href=\"{base}/\">");
    let lower = html.to_ascii_lowercase();
    match lower
        .find("<head")
        .and_then(|i| lower[i..].find('>').map(|j| i + j + 1))
    {
        Some(at) => format!("{}{tag}{}", &html[..at], &html[at..]),
        None => format!("{tag}{html}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steppers_stay_in_range() {
        assert_eq!(step(440, 1, WIDTH), 460);
        assert_eq!(step(440, -1, WIDTH), 420);
        assert_eq!(step(280, -1, WIDTH), 280);
        assert_eq!(step(1000, 1, WIDTH), 1000);
        assert_eq!(step(1, -1, HISTORY), 1);
        assert_eq!(step(10, 1, HISTORY), 10);
        assert_eq!(step(40, -1, BAND), 40);
        assert_eq!(step(290, 1, BAND), 300);
    }

    #[test]
    fn the_base_goes_right_after_the_head_tag() {
        let h =
            "<!DOCTYPE html>\n<html><HEAD lang=x><meta charset=utf-8></HEAD><body>hi</body></html>";
        let out = with_base(h, "http://10.0.0.5:9443/");
        assert!(
            out.contains("<HEAD lang=x><base href=\"http://10.0.0.5:9443/\"><meta charset=utf-8>"),
            "{out}"
        );
        assert_eq!(out.matches("<base").count(), 1);
        assert!(with_base("<p>no head</p>", "http://h").starts_with("<base href=\"http://h/\"><p>"));
        assert!(
            with_base(h, "http://h\"x").contains("http://h&quot;x/"),
            "a quote cannot end the attribute"
        );
    }

    #[test]
    fn colors_compare_without_case_and_the_page_url_has_one_slash() {
        assert!(same_color("#ff7714", "#FF7714"));
        assert!(!same_color("#ff7714", "#FFC15A"));
        assert_eq!(url("http://h:9443/"), "http://h:9443/overlay");
        assert_eq!(url("http://h:9443"), "http://h:9443/overlay");
    }
}
