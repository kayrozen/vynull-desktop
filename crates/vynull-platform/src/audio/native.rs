//! Native playback: tracks are fetched over HTTP and decoded once (symphonia),
//! then mixed in the sound card's callback (cpal). Seeking is exact because
//! the whole track is in memory.

mod decode;
mod engine;

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use super::{AudioPlayer, AudioState, PlayStatus};
use engine::{Engine, Load};

pub struct NativeAudio {
    engine: Arc<Mutex<Engine>>,
    /// Keeps the output stream alive.
    _stream: cpal::Stream,
}

fn lock(e: &Mutex<Engine>) -> MutexGuard<'_, Engine> {
    // A panic elsewhere must not silence playback for good.
    e.lock().unwrap_or_else(|p| p.into_inner())
}

impl NativeAudio {
    /// Open the default output device. Fails when there is no sound card or it is busy.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no sound output device")?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("sound device: {e}"))?;
        let format = supported.sample_format();
        let config = supported.config();
        let (rate, channels) = (config.sample_rate, config.channels as usize);
        let engine = Arc::new(Mutex::new(Engine::new(rate)));

        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, &engine, channels),
            SampleFormat::I16 => build::<i16>(&device, &config, &engine, channels),
            SampleFormat::U16 => build::<u16>(&device, &config, &engine, channels),
            other => return Err(format!("unsupported sample format {other:?}")),
        }?;
        stream
            .play()
            .map_err(|e| format!("cannot start the sound stream: {e}"))?;
        Ok(NativeAudio {
            engine,
            _stream: stream,
        })
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    engine: &Arc<Mutex<Engine>>,
    channels: usize,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let engine = engine.clone();
    let mut scratch: Vec<f32> = Vec::new();
    device
        .build_output_stream::<T, _, _>(
            *config,
            move |data: &mut [T], _| {
                scratch.resize(data.len(), 0.0);
                lock(&engine).render(&mut scratch, channels);
                for (o, s) in data.iter_mut().zip(&scratch) {
                    *o = T::from_sample(*s);
                }
            },
            |_| {},
            None,
        )
        .map_err(|e| format!("cannot open the sound stream: {e}"))
}

impl AudioPlayer for NativeAudio {
    fn load(&self, url: &str) {
        let generation = lock(&self.engine).begin_load();
        let engine = self.engine.clone();
        let url = url.to_string();
        std::thread::spawn(move || {
            let result = fetch(&url).and_then(decode::decode);
            let mut e = lock(&engine);
            if e.generation != generation {
                return; // another track was loaded meanwhile
            }
            match result {
                Ok(d) => e.set_audio(d.rate, d.channels, d.samples),
                Err(why) => e.load = Load::Failed(why),
            }
        });
    }

    fn play(&self) {
        lock(&self.engine).play();
    }

    fn pause(&self) {
        lock(&self.engine).pause();
    }

    fn seek(&self, secs: f64) {
        lock(&self.engine).seek(secs);
    }

    fn set_volume(&self, volume: f64) {
        lock(&self.engine).volume = volume.clamp(0.0, 1.0) as f32;
    }

    fn state(&self) -> AudioState {
        let e = lock(&self.engine);
        let (status, error) = match &e.load {
            Load::Empty => (PlayStatus::Empty, None),
            Load::Loading => (PlayStatus::Loading, None),
            Load::Failed(why) => (PlayStatus::Failed, Some(why.clone())),
            Load::Ready if e.ended => (PlayStatus::Ended, None),
            Load::Ready if e.playing => (PlayStatus::Playing, None),
            Load::Ready => (PlayStatus::Paused, None),
        };
        AudioState {
            status,
            position_secs: e.position_secs(),
            duration_secs: e.duration_secs(),
            volume: e.volume as f64,
            error,
        }
    }

    fn click_at(&self, delay_secs: f64, accent: bool) {
        lock(&self.engine).schedule_click(delay_secs, accent);
    }
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    // The blocking client's default total timeout is 30 s and covers the whole body, which a long
    // or transcoded track (tens of MB) over a slow link can exceed. Ten minutes is generous for any
    // track and still ends a download that stalls (the blocking client has no per-read timeout).
    let http = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| format!("cannot fetch the audio: {e}"))?;
    let resp = http
        .get(url)
        .send()
        .map_err(|e| format!("cannot fetch the audio: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "the daemon answered {} for the audio",
            resp.status()
        ));
    }
    resp.bytes()
        .map(|b| b.to_vec())
        .map_err(|e| format!("cannot read the audio: {e}"))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use super::decode::wav_bytes;
    use super::*;

    /// Serve `body` once over HTTP on a free local port.
    fn serve(body: Vec<u8>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/api/audio/1", l.local_addr().unwrap());
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        url
    }

    fn wait_for(audio: &NativeAudio, what: impl Fn(&AudioState) -> bool) -> AudioState {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let st = audio.state();
            if what(&st) || Instant::now() > end {
                return st;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Plays two seconds of a 440 Hz tone through the real default output device,
    /// at a very low volume.
    #[test]
    #[ignore = "needs a sound card (and plays a quiet tone)"]
    fn plays_through_the_sound_card() {
        let audio = NativeAudio::new().expect("a default output device");
        audio.set_volume(0.02);
        audio.load(&serve(wav_bytes(44_100, 2, 44_100 * 2, |i| {
            (i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin()
        })));
        let st = wait_for(&audio, |s| s.status != PlayStatus::Loading);
        assert_eq!(st.status, PlayStatus::Paused, "{:?}", st.error);
        assert!((st.duration_secs.unwrap() - 2.0).abs() < 0.01);

        audio.play();
        std::thread::sleep(Duration::from_millis(700));
        let st = audio.state();
        assert_eq!(st.status, PlayStatus::Playing);
        assert!(
            st.position_secs > 0.5 && st.position_secs < 1.0,
            "the playhead follows the sound card clock: {}",
            st.position_secs
        );

        audio.pause();
        let held = audio.state().position_secs;
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            audio.state().position_secs,
            held,
            "pause holds the playhead"
        );

        audio.seek(1.9);
        audio.play();
        let st = wait_for(&audio, |s| s.status == PlayStatus::Ended);
        assert_eq!(st.status, PlayStatus::Ended, "playback reaches the end");
    }

    #[test]
    fn a_failed_download_is_reported() {
        // No device needed for the fetch path: use the engine directly through a closed port.
        assert!(fetch("http://127.0.0.1:9/nothing").is_err());
        assert!(decode::decode(b"<html>not audio</html>".to_vec()).is_err());
    }
}
