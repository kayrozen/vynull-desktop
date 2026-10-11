//! Track playback for the drawer: play, seek, cue jump, volume and metronome
//! clicks. Backends: the browser (`HtmlAudioElement` + WebAudio) and, later,
//! native (symphonia + cpal). [`FakeAudio`] drives UI and logic tests.
//!
//! The UI polls [`AudioPlayer::state`] each frame, the same way it reads the
//! SSE stream. Commands never fail directly: a problem shows up as
//! [`PlayStatus::Failed`] with a message in [`AudioState::error`]. As with
//! [`crate::Store`], there is no `Send`/`Sync` bound.
//!
//! The metronome has no scheduler here. The UI looks ahead a short window
//! from the reported position and the beat grid, and calls
//! [`AudioPlayer::click_at`] for each beat in it, so clicks line up with the
//! audio clock and not with UI timers.

use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayStatus {
    /// Nothing loaded.
    #[default]
    Empty,
    /// Loaded but not yet able to play (fetching or decoding).
    Loading,
    Paused,
    Playing,
    /// Reached the end of the track.
    Ended,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioState {
    pub status: PlayStatus,
    /// Playhead in seconds from the start of the decoded audio.
    pub position_secs: f64,
    /// Length of the decoded audio. `None` until known. Compare it with the
    /// analysis length to show the length difference (Δlen).
    pub duration_secs: Option<f64>,
    /// 0.0 to 1.0.
    pub volume: f64,
    pub error: Option<String>,
}

pub trait AudioPlayer {
    /// Replace the current track. It starts paused at 0. `url` comes from
    /// `Client::audio_url` (it supports Range requests, so seeking is cheap).
    fn load(&self, url: &str);
    fn play(&self);
    fn pause(&self);
    /// Move the playhead. Clamped to the track once its length is known.
    fn seek(&self, secs: f64);
    /// Clamped to 0.0..=1.0.
    fn set_volume(&self, volume: f64);
    fn state(&self) -> AudioState;
    /// Sound one metronome click `delay_secs` from now on the audio clock.
    /// `accent` marks the downbeat.
    fn click_at(&self, delay_secs: f64, accent: bool);
}

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::WebAudio;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeAudio;

/// A scripted player for tests. Time moves only when [`FakeAudio::advance`] is called.
#[derive(Debug, Default)]
pub struct FakeAudio {
    st: RefCell<AudioState>,
    url: RefCell<Option<String>>,
    clicks: RefCell<Vec<(f64, bool)>>,
}

impl FakeAudio {
    pub fn new() -> Self {
        let f = Self::default();
        f.st.borrow_mut().volume = 1.0;
        f
    }

    /// Finish loading a track of the given length.
    pub fn finish_loading(&self, duration_secs: f64) {
        let mut st = self.st.borrow_mut();
        if st.status == PlayStatus::Loading {
            st.status = PlayStatus::Paused;
            st.duration_secs = Some(duration_secs);
        }
    }

    /// Fail the current load or playback.
    pub fn fail(&self, why: &str) {
        let mut st = self.st.borrow_mut();
        st.status = PlayStatus::Failed;
        st.error = Some(why.to_string());
    }

    /// Let `secs` of audio time pass.
    pub fn advance(&self, secs: f64) {
        let mut st = self.st.borrow_mut();
        if st.status != PlayStatus::Playing {
            return;
        }
        st.position_secs += secs;
        if let Some(d) = st.duration_secs {
            if st.position_secs >= d {
                st.position_secs = d;
                st.status = PlayStatus::Ended;
            }
        }
    }

    pub fn loaded_url(&self) -> Option<String> {
        self.url.borrow().clone()
    }

    /// `(delay_secs, accent)` for each click requested so far.
    pub fn clicks(&self) -> Vec<(f64, bool)> {
        self.clicks.borrow().clone()
    }
}

impl AudioPlayer for FakeAudio {
    fn load(&self, url: &str) {
        *self.url.borrow_mut() = Some(url.to_string());
        let volume = self.st.borrow().volume;
        *self.st.borrow_mut() = AudioState {
            status: PlayStatus::Loading,
            volume,
            ..Default::default()
        };
    }

    fn play(&self) {
        let mut st = self.st.borrow_mut();
        match st.status {
            PlayStatus::Paused => st.status = PlayStatus::Playing,
            // Playing again from the end restarts the track, like a media element.
            PlayStatus::Ended => {
                st.position_secs = 0.0;
                st.status = PlayStatus::Playing;
            }
            _ => {}
        }
    }

    fn pause(&self) {
        let mut st = self.st.borrow_mut();
        if st.status == PlayStatus::Playing {
            st.status = PlayStatus::Paused;
        }
    }

    fn seek(&self, secs: f64) {
        let mut st = self.st.borrow_mut();
        let max = st.duration_secs.unwrap_or(f64::MAX);
        st.position_secs = secs.clamp(0.0, max);
        if st.status == PlayStatus::Ended && st.position_secs < max {
            st.status = PlayStatus::Paused;
        }
    }

    fn set_volume(&self, volume: f64) {
        self.st.borrow_mut().volume = volume.clamp(0.0, 1.0);
    }

    fn state(&self) -> AudioState {
        self.st.borrow().clone()
    }

    fn click_at(&self, delay_secs: f64, accent: bool) {
        self.clicks.borrow_mut().push((delay_secs, accent));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_ignores_play_until_ready() {
        let a: Box<dyn AudioPlayer> = Box::new(FakeAudio::new());
        assert_eq!(a.state().status, PlayStatus::Empty);
        a.load("http://h/api/audio/1");
        assert_eq!(a.state().status, PlayStatus::Loading);
        a.play(); // ignored until loaded
        assert_eq!(a.state().status, PlayStatus::Loading);
    }

    #[test]
    fn transport_with_scripted_time() {
        let a = FakeAudio::new();
        a.load("u");
        a.finish_loading(10.0);
        assert_eq!(a.state().duration_secs, Some(10.0));
        a.play();
        a.advance(4.0);
        assert_eq!(a.state().position_secs, 4.0);
        a.seek(99.0); // cue jump past the end clamps
        assert_eq!(a.state().position_secs, 10.0);
        a.seek(2.5);
        a.pause();
        a.advance(5.0); // paused: no movement
        assert_eq!(a.state().position_secs, 2.5);
        a.play();
        a.advance(20.0);
        assert_eq!(a.state().status, PlayStatus::Ended);
        a.play();
        assert_eq!(a.state().position_secs, 0.0);
        assert_eq!(a.state().status, PlayStatus::Playing);
    }

    #[test]
    fn volume_survives_load_and_clamps() {
        let a = FakeAudio::new();
        a.set_volume(7.0);
        assert_eq!(a.state().volume, 1.0);
        a.set_volume(0.25);
        a.load("u");
        assert_eq!(a.state().volume, 0.25);
    }

    #[test]
    fn failure_and_clicks_are_reported() {
        let a = FakeAudio::new();
        a.load("u");
        a.fail("404");
        assert_eq!(a.state().status, PlayStatus::Failed);
        assert_eq!(a.state().error.as_deref(), Some("404"));
        a.click_at(0.1, true);
        a.click_at(0.6, false);
        assert_eq!(a.clicks(), vec![(0.1, true), (0.6, false)]);
    }
}
