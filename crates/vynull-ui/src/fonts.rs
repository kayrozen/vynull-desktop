//! The bundled fonts as one `@font-face` stylesheet. The files are embedded as
//! data URLs, so the client needs no network for fonts and the same code works
//! on the web and in a desktop webview.

use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

const ROBOTO_LATIN: &[u8] = include_bytes!("../assets/fonts/roboto-latin.woff2");
const ROBOTO_LATIN_EXT: &[u8] = include_bytes!("../assets/fonts/roboto-latin-ext.woff2");
const COUSINE_400: &[u8] = include_bytes!("../assets/fonts/cousine-400-latin.woff2");
const COUSINE_700: &[u8] = include_bytes!("../assets/fonts/cousine-700-latin.woff2");

const LATIN: &str = "U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD";
const LATIN_EXT: &str = "U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C4,U+2113,U+2C60-2C7F,U+A720-A7FF";

fn face(family: &str, weight: &str, bytes: &[u8], range: &str) -> String {
    format!(
        "@font-face{{font-family:'{family}';font-style:normal;font-weight:{weight};font-display:swap;src:url(data:font/woff2;base64,{}) format('woff2');unicode-range:{range}}}",
        STANDARD.encode(bytes)
    )
}

/// The stylesheet, built once.
pub fn css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        [
            face("Roboto", "100 900", ROBOTO_LATIN, LATIN),
            face("Roboto", "100 900", ROBOTO_LATIN_EXT, LATIN_EXT),
            face("Cousine", "400", COUSINE_400, LATIN),
            face("Cousine", "700", COUSINE_700, LATIN),
        ]
        .join("\n")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_faces_with_real_woff2_data() {
        let css = css();
        assert_eq!(css.matches("@font-face").count(), 4);
        for b in [ROBOTO_LATIN, ROBOTO_LATIN_EXT, COUSINE_400, COUSINE_700] {
            assert_eq!(&b[..4], b"wOF2", "not a woff2 file");
        }
        assert!(css.contains("font-family:'Cousine'") && css.contains("font-family:'Roboto'"));
    }
}
