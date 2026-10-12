//! The library as the client holds it: every track parsed once, a folded
//! search index, tags and playlists. It reloads when `/api/tracks/rev` moves.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;
use vynull_api::{Client, PlaylistInfo, TagInfo, TrackInfo};
use vynull_core::search::SearchIndex;
use vynull_platform::time;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Load {
    #[default]
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Default)]
pub struct LibState {
    pub load: Load,
    pub tracks: Rc<Vec<TrackInfo>>,
    pub index: Rc<SearchIndex>,
    pub by_id: Rc<HashMap<i64, usize>>,
    pub tags: Rc<Vec<TagInfo>>,
    pub playlists: Rc<Vec<PlaylistInfo>>,
}

impl LibState {
    /// A copy with one track replaced (after an edit), so the list and search
    /// show it at once instead of waiting for the next poll.
    pub fn with_track(&self, t: TrackInfo) -> LibState {
        let Some(&i) = self.by_id.get(&t.id) else {
            return self.clone();
        };
        let mut tracks = (*self.tracks).clone();
        tracks[i] = t;
        LibState {
            index: Rc::new(self.index.with_row(i, &tracks[i])),
            tracks: Rc::new(tracks),
            ..self.clone()
        }
    }
}

async fn fetch(c: &Client) -> Result<LibState, String> {
    let tracks = c.tracks().await.map_err(|e| e.to_string())?;
    // Tags and playlists are secondary: a failure leaves them empty.
    let tags = c.tags().await.unwrap_or_default();
    let playlists = c.playlists().await.unwrap_or_default();
    let index = SearchIndex::new(&tracks);
    let by_id = tracks.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
    Ok(LibState {
        load: Load::Ready,
        tracks: Rc::new(tracks),
        index: Rc::new(index),
        by_id: Rc::new(by_id),
        tags: Rc::new(tags),
        playlists: Rc::new(playlists),
    })
}

/// Load the library and keep it current. Call once, high enough that the data
/// survives tab changes.
pub fn use_library(client: Client) -> Signal<LibState> {
    let mut state = use_signal(LibState::default);
    use_future(move || {
        let c = client.clone();
        async move {
            let mut loaded = false;
            let mut rev = None;
            loop {
                let now = c.library_rev().await.ok();
                if !loaded || (now.is_some() && now != rev) {
                    match fetch(&c).await {
                        Ok(s) => {
                            state.set(s);
                            loaded = true;
                            rev = now;
                        }
                        Err(e) => {
                            if !loaded {
                                state.write().load = Load::Failed(e);
                            }
                        }
                    }
                }
                time::sleep(Duration::from_secs(5)).await;
            }
        }
    });
    state
}
