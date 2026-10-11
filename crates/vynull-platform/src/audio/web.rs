use std::cell::RefCell;
use std::rc::Rc;

use web_sys::{AudioContext, AudioContextState, HtmlAudioElement, OscillatorType};

use super::{AudioPlayer, AudioState, PlayStatus};

/// `HtmlAudioElement` for the track, WebAudio oscillators for metronome clicks.
pub struct WebAudio {
    el: HtmlAudioElement,
    ctx: RefCell<Option<AudioContext>>,
    /// Set when `play()` is rejected, for example by the autoplay policy.
    play_error: Rc<RefCell<Option<String>>>,
    loaded: RefCell<bool>,
}

impl WebAudio {
    pub fn new() -> Result<Self, String> {
        let el = HtmlAudioElement::new().map_err(|e| format!("{e:?}"))?;
        // Only the length until the user plays: a library is mostly not played.
        el.set_preload("metadata");
        Ok(Self {
            el,
            ctx: RefCell::new(None),
            play_error: Rc::default(),
            loaded: RefCell::new(false),
        })
    }

    /// The click context starts suspended until a user gesture; `click_at` is
    /// called after the user pressed play, so resuming here is allowed.
    fn context(&self) -> Option<AudioContext> {
        let mut slot = self.ctx.borrow_mut();
        if slot.is_none() {
            *slot = AudioContext::new().ok();
        }
        let ctx = slot.clone()?;
        if ctx.state() == AudioContextState::Suspended {
            let _ = ctx.resume();
        }
        Some(ctx)
    }
}

impl AudioPlayer for WebAudio {
    fn load(&self, url: &str) {
        *self.play_error.borrow_mut() = None;
        *self.loaded.borrow_mut() = true;
        self.el.set_src(url);
        self.el.load();
    }

    fn play(&self) {
        *self.play_error.borrow_mut() = None;
        if let Ok(promise) = self.el.play() {
            let err = self.play_error.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = wasm_bindgen_futures::JsFuture::from(promise).await {
                    *err.borrow_mut() = Some(format!("{e:?}"));
                }
            });
        }
    }

    fn pause(&self) {
        let _ = self.el.pause();
    }

    fn seek(&self, secs: f64) {
        let max = self.duration().unwrap_or(f64::MAX);
        self.el.set_current_time(secs.clamp(0.0, max));
    }

    fn set_volume(&self, volume: f64) {
        self.el.set_volume(volume.clamp(0.0, 1.0));
    }

    fn state(&self) -> AudioState {
        let error = self
            .el
            .error()
            .map(|e| format!("media error {}: {}", e.code(), e.message()))
            .or_else(|| self.play_error.borrow().clone());
        let status = if !*self.loaded.borrow() {
            PlayStatus::Empty
        } else if error.is_some() {
            PlayStatus::Failed
        } else if self.el.ended() {
            PlayStatus::Ended
        } else if self.el.ready_state() < 2 {
            PlayStatus::Loading
        } else if self.el.paused() {
            PlayStatus::Paused
        } else {
            PlayStatus::Playing
        };
        AudioState {
            status,
            position_secs: self.el.current_time(),
            duration_secs: self.duration(),
            volume: self.el.volume(),
            error,
        }
    }

    fn click_at(&self, delay_secs: f64, accent: bool) {
        let Some(ctx) = self.context() else { return };
        let (Ok(osc), Ok(gain)) = (ctx.create_oscillator(), ctx.create_gain()) else {
            return;
        };
        osc.set_type(OscillatorType::Square);
        osc.frequency()
            .set_value(if accent { 1600.0 } else { 1000.0 });
        let t = ctx.current_time() + delay_secs.max(0.0);
        let g = gain.gain();
        let _ = g.set_value_at_time(0.0, t);
        let _ = g.linear_ramp_to_value_at_time(if accent { 0.35 } else { 0.2 }, t + 0.002);
        let _ = g.linear_ramp_to_value_at_time(0.0, t + 0.04);
        let _ = osc.connect_with_audio_node(&gain);
        let _ = gain.connect_with_audio_node(&ctx.destination());
        let _ = osc.start_with_when(t);
        let _ = osc.stop_with_when(t + 0.05);
    }
}

impl WebAudio {
    /// Streams of unknown length report infinity until known, and NaN before metadata.
    fn duration(&self) -> Option<f64> {
        let d = self.el.duration();
        (d.is_finite() && d > 0.0).then_some(d)
    }
}
