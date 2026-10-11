//! The mixer behind native playback. It holds one decoded track and renders
//! output blocks, so everything about seeking, ending, volume, sample-rate
//! conversion and metronome clicks can be tested without a sound card.

use std::f32::consts::TAU;

/// What the track is doing, apart from where it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Load {
    #[default]
    Empty,
    Loading,
    Ready,
    Failed(String),
}

#[derive(Debug, Clone, Copy)]
struct Click {
    /// Output frame (since the engine started) where the click begins.
    start: u64,
    accent: bool,
}

const CLICK_SECS: f32 = 0.04;

#[derive(Debug)]
pub struct Engine {
    pub load: Load,
    /// Bumped by every `load`, so a slow download that finishes late is ignored.
    pub generation: u64,
    samples: Vec<f32>,
    rate: u32,
    channels: usize,
    /// Playhead in source frames.
    pos: f64,
    pub playing: bool,
    pub ended: bool,
    pub volume: f32,
    out_rate: u32,
    /// Output frames rendered so far: the audio clock the clicks are scheduled on.
    clock: u64,
    clicks: Vec<Click>,
}

impl Engine {
    pub fn new(out_rate: u32) -> Self {
        Engine {
            load: Load::Empty,
            generation: 0,
            samples: Vec::new(),
            rate: 44_100,
            channels: 2,
            pos: 0.0,
            playing: false,
            ended: false,
            volume: 1.0,
            out_rate: out_rate.max(1),
            clock: 0,
            clicks: Vec::new(),
        }
    }

    /// Start loading a new track: silence until `set_audio`.
    pub fn begin_load(&mut self) -> u64 {
        self.generation += 1;
        self.load = Load::Loading;
        self.samples = Vec::new();
        self.pos = 0.0;
        self.playing = false;
        self.ended = false;
        self.clicks.clear();
        self.generation
    }

    pub fn set_audio(&mut self, rate: u32, channels: usize, samples: Vec<f32>) {
        self.rate = rate.max(1);
        self.channels = channels.max(1);
        self.samples = samples;
        self.load = Load::Ready;
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels
    }

    pub fn duration_secs(&self) -> Option<f64> {
        (self.load == Load::Ready).then(|| self.frames() as f64 / self.rate as f64)
    }

    pub fn position_secs(&self) -> f64 {
        self.pos / self.rate as f64
    }

    pub fn seek(&mut self, secs: f64) {
        let max = self.frames() as f64;
        self.pos = (secs * self.rate as f64).clamp(0.0, max);
        if self.ended && self.pos < max {
            self.ended = false;
        }
    }

    pub fn play(&mut self) {
        if self.load != Load::Ready {
            return;
        }
        if self.ended {
            self.pos = 0.0;
            self.ended = false;
        }
        self.playing = true;
    }

    /// Sound a click `delay_secs` from now on the audio clock.
    pub fn schedule_click(&mut self, delay_secs: f64, accent: bool) {
        let start = self.clock + (delay_secs.max(0.0) * self.out_rate as f64) as u64;
        self.clicks.push(Click { start, accent });
    }

    /// Fill `out` (interleaved, `out_channels` per frame) with the next block.
    pub fn render(&mut self, out: &mut [f32], out_channels: usize) {
        let out_channels = out_channels.max(1);
        let step = self.rate as f64 / self.out_rate as f64;
        let frames = self.frames();
        let click_len = (CLICK_SECS * self.out_rate as f32) as u64;
        for (i, frame) in out.chunks_mut(out_channels).enumerate() {
            let now = self.clock + i as u64;
            let mut mix = [0.0f32; 8];
            let used = out_channels.min(mix.len());

            if self.playing && frames > 0 {
                let f0 = self.pos as usize;
                if f0 >= frames {
                    self.playing = false;
                    self.ended = true;
                } else {
                    let f1 = (f0 + 1).min(frames - 1);
                    let t = (self.pos - f0 as f64) as f32;
                    for (c, m) in mix.iter_mut().enumerate().take(used) {
                        // Mono plays on every channel; extra output channels repeat the last source one.
                        let sc = c.min(self.channels - 1);
                        let a = self.samples[f0 * self.channels + sc];
                        let b = self.samples[f1 * self.channels + sc];
                        *m = (a + (b - a) * t) * self.volume;
                    }
                    self.pos += step;
                }
            }

            for k in &self.clicks {
                if now >= k.start && now - k.start < click_len {
                    let t = (now - k.start) as f32 / self.out_rate as f32;
                    let freq = if k.accent { 1600.0 } else { 1000.0 };
                    let gain = if k.accent { 0.35 } else { 0.2 }
                        * (1.0 - (now - k.start) as f32 / click_len as f32);
                    let s = (TAU * freq * t).sin() * gain;
                    for m in mix.iter_mut().take(used) {
                        *m += s;
                    }
                }
            }

            for (c, o) in frame.iter_mut().enumerate() {
                *o = mix[c.min(used - 1)].clamp(-1.0, 1.0);
            }
        }
        self.clock += (out.len() / out_channels) as u64;
        self.clicks.retain(|k| k.start + click_len > self.clock);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(rate: u32, channels: usize, frames: usize, f: impl Fn(usize) -> f32) -> Engine {
        let mut e = Engine::new(rate);
        e.begin_load();
        let samples = (0..frames)
            .flat_map(|i| std::iter::repeat_n(f(i), channels))
            .collect();
        e.set_audio(rate, channels, samples);
        e
    }

    #[test]
    fn nothing_plays_until_ready_and_asked() {
        let mut e = Engine::new(1000);
        e.play();
        assert!(!e.playing, "no track loaded");
        e.begin_load();
        e.play();
        assert!(!e.playing, "still loading");
        let mut e = ready(1000, 2, 100, |_| 0.5);
        let mut out = vec![9.0; 20];
        e.render(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0), "paused is silent");
        assert_eq!(e.position_secs(), 0.0);
    }

    #[test]
    fn playing_advances_and_mixes_the_samples() {
        let mut e = ready(1000, 2, 1000, |_| 0.5);
        e.play();
        let mut out = vec![0.0; 200]; // 100 stereo frames = 0.1 s
        e.render(&mut out, 2);
        assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-6));
        assert!((e.position_secs() - 0.1).abs() < 1e-9);
        e.volume = 0.5;
        e.render(&mut out, 2);
        assert!(out.iter().all(|s| (*s - 0.25).abs() < 1e-6));
    }

    #[test]
    fn mono_fills_every_output_channel_and_rates_convert() {
        // A 500 Hz track on a 1000 Hz device plays at half speed.
        let mut e = Engine::new(1000);
        e.begin_load();
        e.set_audio(500, 1, (0..500).map(|i| i as f32 / 500.0).collect());
        e.play();
        let mut out = vec![0.0; 40]; // 10 frames of 4 channels
        e.render(&mut out, 4);
        assert!(out.chunks(4).all(|f| f.iter().all(|s| *s == f[0])));
        assert!(
            (e.position_secs() - 0.01).abs() < 1e-9,
            "10 output frames = 5 source frames = 10 ms"
        );
        // Linear interpolation between source frames: output 1 is halfway between 0 and 1.
        assert!((out[4] - 0.001).abs() < 1e-6);
    }

    #[test]
    fn the_end_stops_playback_and_play_restarts() {
        let mut e = ready(1000, 1, 50, |_| 0.5);
        e.play();
        let mut out = vec![0.0; 100];
        e.render(&mut out, 1);
        assert!(e.ended && !e.playing);
        assert_eq!(out[75], 0.0, "silent after the end");
        e.play();
        assert!(e.playing && !e.ended);
        assert_eq!(e.position_secs(), 0.0, "play from the end starts over");
    }

    #[test]
    fn seek_clamps_and_clears_the_ended_flag() {
        let mut e = ready(1000, 1, 1000, |_| 0.1);
        e.seek(99.0);
        assert_eq!(e.position_secs(), 1.0);
        e.seek(-5.0);
        assert_eq!(e.position_secs(), 0.0);
        e.ended = true;
        e.seek(0.5);
        assert!(!e.ended);
        assert_eq!(e.duration_secs(), Some(1.0));
    }

    #[test]
    fn clicks_sound_on_schedule_and_are_louder_on_the_downbeat() {
        // 48 kHz: the clicks are 1 and 1.6 kHz tones, which need a real sample rate.
        let mut e = Engine::new(48_000);
        e.begin_load();
        e.set_audio(48_000, 1, vec![0.0; 48_000]);
        e.schedule_click(0.05, true); // frame 2400
        e.schedule_click(0.2, false); // frame 9600
        let mut out = vec![0.0; 12_000];
        e.render(&mut out, 1);
        let peak = |a: usize, b: usize| out[a..b].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            out[..2400].iter().all(|s| *s == 0.0),
            "nothing before the first click"
        );
        let accent = peak(2400, 4320);
        let plain = peak(9600, 11_520);
        assert!(accent > 0.25, "accent click is audible: {accent}");
        assert!(
            plain > 0.12 && plain < accent,
            "plain click is quieter: {plain} vs {accent}"
        );
        assert!(
            out[4320..9600].iter().all(|s| *s == 0.0),
            "silence between clicks"
        );
        assert!(e.clicks.is_empty(), "finished clicks are dropped");
    }

    #[test]
    fn a_new_load_discards_the_old_track() {
        let mut e = ready(1000, 1, 100, |_| 0.5);
        e.play();
        let g = e.begin_load();
        assert_eq!(e.load, Load::Loading);
        assert!(!e.playing);
        assert_eq!(e.duration_secs(), None);
        assert!(g > 1);
    }
}
