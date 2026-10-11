//! Whole-file decoding to interleaved `f32`. Tracks are decoded once on load,
//! so seeking and looping are exact and the audio callback only mixes.
//! mp3, FLAC, WAV and Ogg Vorbis: the formats the daemon serves directly (it
//! converts everything else to WAV).

use std::io::{Cursor, ErrorKind};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct Decoded {
    pub rate: u32,
    pub channels: usize,
    /// Interleaved samples, `channels` per frame.
    pub samples: Vec<f32>,
}

pub fn decode(bytes: Vec<u8>) -> Result<Decoded, String> {
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unknown audio format: {e}"))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("no audio track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("cannot decode this audio: {e}"))?;

    let (mut rate, mut channels, mut samples) = (0u32, 0usize, Vec::new());
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // End of the stream is reported as an unexpected EOF.
            Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("read error: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                rate = buf.spec().rate;
                channels = buf.spec().channels.count();
                let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, *buf.spec());
                sb.copy_interleaved_ref(buf);
                samples.extend_from_slice(sb.samples());
            }
            // A damaged packet: skip it, keep playing.
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode error: {e}")),
        }
    }
    if samples.is_empty() || rate == 0 || channels == 0 {
        return Err("the file has no audio".into());
    }
    Ok(Decoded {
        rate,
        channels,
        samples,
    })
}

#[cfg(test)]
pub fn wav_bytes(rate: u32, channels: u16, frames: usize, f: impl Fn(usize) -> f32) -> Vec<u8> {
    let data_len = (frames * channels as usize * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    b.extend_from_slice(&(channels * 2).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let v = (f(i).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        for _ in 0..channels {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_wav() {
        let d = decode(wav_bytes(8000, 2, 4000, |i| (i as f32 / 40.0).sin() * 0.5)).unwrap();
        assert_eq!((d.rate, d.channels), (8000, 2));
        assert_eq!(d.samples.len(), 4000 * 2);
        assert!(d.samples.iter().any(|s| s.abs() > 0.4));
    }

    #[test]
    fn refuses_junk_and_empty_audio() {
        assert!(decode(b"this is not audio".to_vec()).is_err());
        assert!(decode(Vec::new()).is_err());
        assert!(decode(wav_bytes(8000, 1, 0, |_| 0.0)).is_err());
    }
}
