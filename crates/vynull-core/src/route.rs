//! URL routes (`#/library/playlist/5/track/123`), so a link, a reload and the
//! browser's back button all return to the same screen.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Players,
    Library,
    Tags,
    Settings,
    Diag,
    Overlay,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::Players,
        Page::Library,
        Page::Tags,
        Page::Settings,
        Page::Diag,
        Page::Overlay,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Page::Players => "players",
            Page::Library => "library",
            Page::Tags => "tags",
            Page::Settings => "settings",
            Page::Diag => "diag",
            Page::Overlay => "overlay",
        }
    }

    fn from_slug(s: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.slug() == s)
    }
}

/// What the library list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    #[default]
    Collection,
    Playlist(i64),
    Queue,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Route {
    pub page: Page,
    pub source: Source,
    /// The track whose drawer is open (library only).
    pub track: Option<i64>,
}

impl Route {
    pub fn page(page: Page) -> Self {
        Route {
            page,
            ..Default::default()
        }
    }

    /// Parse a location hash such as `#/library/queue/track/7`. Anything that
    /// does not parse gives the default route instead of an error.
    pub fn parse(hash: &str) -> Route {
        let path = hash.trim_start_matches('#').trim_matches('/');
        let mut parts = path.split('/').filter(|p| !p.is_empty());
        let Some(page) = parts.next().and_then(Page::from_slug) else {
            return Route::default();
        };
        let mut r = Route::page(page);
        if page != Page::Library {
            return r;
        }
        let rest: Vec<&str> = parts.collect();
        let mut i = 0;
        while i < rest.len() {
            match rest[i] {
                "queue" => r.source = Source::Queue,
                "history" => r.source = Source::History,
                "playlist" => {
                    if let Some(id) = rest.get(i + 1).and_then(|v| v.parse().ok()) {
                        r.source = Source::Playlist(id);
                        i += 1;
                    }
                }
                "track" => {
                    if let Some(id) = rest.get(i + 1).and_then(|v| v.parse().ok()) {
                        r.track = Some(id);
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        r
    }

    /// The hash for this route, always starting with `#/`.
    pub fn hash(&self) -> String {
        let mut s = format!("#/{}", self.page.slug());
        if self.page == Page::Library {
            match self.source {
                Source::Collection => {}
                Source::Playlist(id) => s.push_str(&format!("/playlist/{id}")),
                Source::Queue => s.push_str("/queue"),
                Source::History => s.push_str("/history"),
            }
            if let Some(t) = self.track {
                s.push_str(&format!("/track/{t}"));
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for r in [
            Route::default(),
            Route::page(Page::Diag),
            Route {
                page: Page::Library,
                source: Source::Collection,
                track: None,
            },
            Route {
                page: Page::Library,
                source: Source::Playlist(5),
                track: Some(123),
            },
            Route {
                page: Page::Library,
                source: Source::Queue,
                track: Some(7),
            },
            Route {
                page: Page::Library,
                source: Source::History,
                track: None,
            },
        ] {
            assert_eq!(Route::parse(&r.hash()), r, "{}", r.hash());
        }
    }

    #[test]
    fn hashes_look_right() {
        assert_eq!(Route::default().hash(), "#/players");
        let r = Route {
            page: Page::Library,
            source: Source::Playlist(5),
            track: Some(123),
        };
        assert_eq!(r.hash(), "#/library/playlist/5/track/123");
    }

    #[test]
    fn bad_input_is_forgiven() {
        assert_eq!(Route::parse(""), Route::default());
        assert_eq!(Route::parse("#"), Route::default());
        assert_eq!(Route::parse("#/nowhere"), Route::default());
        assert_eq!(
            Route::parse("#/library/track/abc"),
            Route::page(Page::Library)
        );
        assert_eq!(
            Route::parse("library/queue"),
            Route {
                page: Page::Library,
                source: Source::Queue,
                track: None
            }
        );
        // Track and source only mean something in the library.
        assert_eq!(Route::parse("#/tags/track/9"), Route::page(Page::Tags));
    }
}
