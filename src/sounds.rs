//! Short interface sounds (microphone, camera, screen share, people joining and leaving).
//!
//! Nothing is read from files: every sound is synthesised from a few musical notes,
//! so the sounds are crisp at any sample rate and add nothing to the download.
//! Rising pitch means "on / arrived", falling pitch means "off / left".
use crate::voice;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::SampleFormat;
use std::{
    collections::HashMap,
    f32::consts::TAU,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sound {
    MicOn,
    MicOff,
    DeafenOn,
    DeafenOff,
    CameraOn,
    CameraOff,
    ScreenOn,
    ScreenOff,
    JoinSelf,
    LeaveSelf,
    PeerJoin,
    PeerLeave,
}

#[cfg(test)]
pub const ALL_SOUNDS: [Sound; 12] = [
    Sound::MicOn, Sound::MicOff, Sound::DeafenOn, Sound::DeafenOff, Sound::CameraOn, Sound::CameraOff,
    Sound::ScreenOn, Sound::ScreenOff, Sound::JoinSelf, Sound::LeaveSelf, Sound::PeerJoin, Sound::PeerLeave,
];

#[derive(Clone, Copy)]
enum Timbre {
    /// Warm sine with a little second and third harmonic.
    Soft,
    /// Inharmonic partials that ring like a small bell.
    Bell,
}

struct Note {
    freq: f32,
    start: f32,
    dur: f32,
    gain: f32,
    timbre: Timbre,
}
const fn note(freq: f32, start: f32, dur: f32, gain: f32, timbre: Timbre) -> Note {
    Note { freq, start, dur, gain, timbre }
}

// Equal-tempered note frequencies.
const G4: f32 = 392.0;
const B4: f32 = 493.9;
const FS4: f32 = 370.0;
const C5: f32 = 523.3;
const D5: f32 = 587.3;
const E5: f32 = 659.3;
const G5: f32 = 784.0;
const A5: f32 = 880.0;
const B5: f32 = 987.8;
const D6: f32 = 1174.7;
const E6: f32 = 1318.5;

fn notes(sound: Sound) -> Vec<Note> {
    use Timbre::*;
    match sound {
        Sound::MicOn => vec![note(E5, 0.0, 0.10, 0.50, Soft), note(A5, 0.075, 0.16, 0.50, Soft)],
        Sound::MicOff => vec![note(A5, 0.0, 0.10, 0.42, Soft), note(E5, 0.075, 0.16, 0.42, Soft)],
        Sound::DeafenOn => vec![note(B4, 0.0, 0.11, 0.50, Soft), note(FS4, 0.085, 0.20, 0.50, Soft)],
        Sound::DeafenOff => vec![note(FS4, 0.0, 0.11, 0.42, Soft), note(B4, 0.085, 0.20, 0.50, Soft)],
        Sound::CameraOn => vec![
            note(C5, 0.0, 0.10, 0.42, Soft), note(E5, 0.055, 0.10, 0.42, Soft), note(G5, 0.11, 0.20, 0.46, Soft)],
        Sound::CameraOff => vec![
            note(G5, 0.0, 0.10, 0.40, Soft), note(E5, 0.055, 0.10, 0.38, Soft), note(C5, 0.11, 0.20, 0.40, Soft)],
        Sound::ScreenOn => vec![
            note(D5, 0.0, 0.12, 0.42, Soft), note(A5, 0.07, 0.12, 0.42, Soft), note(D6, 0.14, 0.26, 0.42, Bell)],
        Sound::ScreenOff => vec![
            note(D6, 0.0, 0.12, 0.38, Soft), note(A5, 0.07, 0.12, 0.38, Soft), note(D5, 0.14, 0.26, 0.38, Soft)],
        Sound::JoinSelf => vec![
            note(G4, 0.0, 0.22, 0.44, Soft), note(C5, 0.085, 0.22, 0.44, Soft),
            note(E5, 0.17, 0.24, 0.44, Soft), note(G5, 0.255, 0.50, 0.48, Bell)],
        Sound::LeaveSelf => vec![
            note(G5, 0.0, 0.16, 0.40, Soft), note(E5, 0.075, 0.16, 0.40, Soft),
            note(C5, 0.15, 0.18, 0.40, Soft), note(G4, 0.225, 0.34, 0.42, Soft)],
        // A friend arriving is a two-note bell: clearly different from your own actions.
        Sound::PeerJoin => vec![note(B5, 0.0, 0.34, 0.42, Bell), note(E6, 0.11, 0.52, 0.46, Bell)],
        Sound::PeerLeave => vec![note(E6, 0.0, 0.26, 0.30, Bell), note(B5, 0.10, 0.40, 0.30, Bell)],
    }
}

/// Renders a sound as mono samples at `rate` Hz.
pub fn synthesize(sound: Sound, rate: u32) -> Vec<f32> {
    let list = notes(sound);
    let total = list.iter().map(|n| n.start + n.dur).fold(0.0, f32::max) + 0.02;
    let mut samples = vec![0.0f32; (total * rate as f32) as usize];
    for n in &list {
        let first = (n.start * rate as f32) as usize;
        let count = (n.dur * rate as f32) as usize;
        let tau = n.dur * 0.42;
        for i in 0..count {
            let Some(out) = samples.get_mut(first + i) else { break; };
            let t = i as f32 / rate as f32;
            let attack = (t / 0.006).min(1.0);
            let release = ((n.dur - t) / 0.014).clamp(0.0, 1.0);
            let wave = match n.timbre {
                Timbre::Soft => {
                    ((TAU * n.freq * t).sin() + 0.28 * (TAU * 2.0 * n.freq * t).sin()
                        + 0.08 * (TAU * 3.0 * n.freq * t).sin()) / 1.36
                }
                Timbre::Bell => {
                    // Higher partials die away faster, like a struck bell.
                    let partial = |ratio: f32, level: f32, speed: f32| {
                        level * (TAU * n.freq * ratio * t).sin() * (-t * speed / tau).exp()
                    };
                    (partial(1.0, 1.0, 1.0) + partial(2.0, 0.45, 2.0) + partial(2.76, 0.32, 3.0)
                        + partial(5.4, 0.14, 5.0)) / 1.6
                }
            };
            *out += n.gain * attack * release * (-t / tau).exp().max(0.0) * wave;
        }
    }
    samples
}

// ── Mixer ──────────────────────────────────────────────────────────────────
struct Voice {
    samples: Arc<Vec<f32>>,
    position: usize,
}

struct Mixer {
    voices: Vec<Voice>,
    gain: f32,
}
impl Mixer {
    fn next_sample(&mut self) -> f32 {
        let mut sum = 0.0;
        for voice in &mut self.voices {
            sum += voice.samples.get(voice.position).copied().unwrap_or(0.0);
            voice.position += 1;
        }
        self.voices.retain(|v| v.position < v.samples.len());
        // Soft limiter: several sounds at once never clip.
        (sum * self.gain).tanh()
    }
}

fn fill<T: Copy>(buffer: &mut [T], channels: usize, mixer: &Mutex<Mixer>, convert: fn(f32) -> T) {
    let Ok(mut mixer) = mixer.lock() else { return; };
    for frame in buffer.chunks_mut(channels.max(1)) {
        let value = convert(mixer.next_sample());
        for sample in frame { *sample = value; }
    }
}
fn to_i16(x: f32) -> i16 { (x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16 }
fn to_u16(x: f32) -> u16 { ((x.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16 }

const SAMPLE_RATE: u32 = 48_000;

pub struct SoundPlayer {
    mixer: Arc<Mutex<Mixer>>,
    stream: Option<cpal::Stream>,
    cache: HashMap<Sound, Arc<Vec<f32>>>,
    pub enabled: bool,
    pub error: Option<String>,
}

impl SoundPlayer {
    pub fn new(device: &str, enabled: bool, volume: f32) -> Self {
        let mut player = Self {
            mixer: Arc::new(Mutex::new(Mixer { voices: Vec::new(), gain: volume.clamp(0.0, 1.0) })),
            stream: None,
            cache: HashMap::new(),
            enabled,
            error: None,
        };
        player.open(device);
        player
    }

    /// (Re)opens the output stream on the named device ("" = system default).
    pub fn open(&mut self, device_name: &str) {
        self.stream = None;
        self.error = None;
        match self.build_stream(device_name) {
            Ok(stream) => self.stream = Some(stream),
            Err(error) => self.error = Some(error),
        }
    }

    fn build_stream(&self, device_name: &str) -> Result<cpal::Stream, String> {
        let device = voice::find_output_device(device_name).ok_or("Не знайдено пристрій відтворення")?;
        let (config, format) = voice::output_config(&device)?;
        let channels = config.channels as usize;
        let error = |e| eprintln!("Sounds output: {e}");
        let mixer = Arc::clone(&self.mixer);
        let stream = match format {
            SampleFormat::F32 => device.build_output_stream(&config,
                move |buf: &mut [f32], _| fill(buf, channels, &mixer, |x| x), error, None),
            SampleFormat::I16 => device.build_output_stream(&config,
                move |buf: &mut [i16], _| fill(buf, channels, &mixer, to_i16), error, None),
            SampleFormat::U16 => device.build_output_stream(&config,
                move |buf: &mut [u16], _| fill(buf, channels, &mixer, to_u16), error, None),
            _ => return Err("Формат пристрою не підтримується".into()),
        }.map_err(|e| format!("Звуки: {e}"))?;
        stream.play().map_err(|e| format!("Звуки: {e}"))?;
        Ok(stream)
    }

    pub fn set_volume(&self, volume: f32) {
        if let Ok(mut mixer) = self.mixer.lock() { mixer.gain = volume.clamp(0.0, 1.0); }
    }

    pub fn play(&mut self, sound: Sound) {
        if !self.enabled || self.stream.is_none() { return; }
        let samples = Arc::clone(self.cache.entry(sound)
            .or_insert_with(|| Arc::new(synthesize(sound, SAMPLE_RATE))));
        if let Ok(mut mixer) = self.mixer.lock() {
            if mixer.gain <= 0.0 || mixer.voices.len() >= 8 { return; }
            mixer.voices.push(Voice { samples, position: 0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dominant frequency of a slice, estimated from zero crossings.
    fn pitch(samples: &[f32], rate: u32) -> f32 {
        let crossings = samples.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        crossings as f32 * rate as f32 / samples.len() as f32
    }

    #[test]
    fn every_sound_is_audible_short_clean_and_never_clips() {
        for rate in [44_100, 48_000] {
            for sound in ALL_SOUNDS {
                let samples = synthesize(sound, rate);
                let seconds = samples.len() as f32 / rate as f32;
                let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                assert!((0.12..1.0).contains(&seconds), "{sound:?} lasts {seconds}s");
                assert!(peak > 0.15, "{sound:?} is too quiet ({peak})");
                assert!(peak < 0.95, "{sound:?} clips ({peak})");
                assert!(samples[0].abs() < 0.02, "{sound:?} starts with a click");
                assert!(samples.last().unwrap().abs() < 0.02, "{sound:?} ends with a click");
                assert!(samples.iter().all(|s| s.is_finite()));
            }
        }
    }

    #[test]
    fn all_sounds_are_different_from_each_other() {
        let rendered: Vec<Vec<f32>> = ALL_SOUNDS.iter().map(|s| synthesize(*s, 48_000)).collect();
        for i in 0..rendered.len() {
            for j in i + 1..rendered.len() {
                assert_ne!(rendered[i], rendered[j], "{:?} equals {:?}", ALL_SOUNDS[i], ALL_SOUNDS[j]);
            }
        }
    }

    #[test]
    fn on_sounds_rise_and_off_sounds_fall() {
        for (up, down) in [
            (Sound::MicOn, Sound::MicOff), (Sound::DeafenOff, Sound::DeafenOn),
            (Sound::CameraOn, Sound::CameraOff), (Sound::ScreenOn, Sound::ScreenOff),
            (Sound::JoinSelf, Sound::LeaveSelf), (Sound::PeerJoin, Sound::PeerLeave),
        ] {
            for (sound, rising) in [(up, true), (down, false)] {
                let list = notes(sound);
                let (first, last) = (list.first().unwrap().freq, list.last().unwrap().freq);
                assert!(if rising { last > first } else { last < first }, "{sound:?}: {first} -> {last}");
            }
        }
    }

    #[test]
    fn rendered_audio_starts_on_the_first_notes_pitch() {
        for sound in [Sound::MicOn, Sound::MicOff, Sound::CameraOn, Sound::JoinSelf, Sound::DeafenOn] {
            let samples = synthesize(sound, 48_000);
            let want = notes(sound)[0].freq;
            let got = pitch(&samples[48..48 * 60], 48_000); // 1 ms .. 60 ms, only the first note sounds
            assert!((got - want).abs() / want < 0.12, "{sound:?}: wanted {want} Hz, measured {got:.0} Hz");
        }
    }

    #[test]
    fn friend_joining_is_brighter_than_your_own_join() {
        // A friend's arrival must be recognisable without looking: it sits clearly higher.
        let average = |sound| { let n = notes(sound); n.iter().map(|x| x.freq).sum::<f32>() / n.len() as f32 };
        assert!(average(Sound::PeerJoin) > average(Sound::JoinSelf) * 1.5);
    }

    #[test]
    fn mixer_sums_voices_limits_loudness_and_drops_finished_ones() {
        let mut mixer = Mixer { voices: Vec::new(), gain: 1.0 };
        let loud = Arc::new(vec![0.9f32; 4]);
        for _ in 0..3 { mixer.voices.push(Voice { samples: Arc::clone(&loud), position: 0 }); }
        let first = mixer.next_sample();
        assert!(first < 1.0 && first > 0.9, "soft limiter, got {first}");
        for _ in 0..3 { mixer.next_sample(); }
        assert!(mixer.voices.is_empty());
        assert_eq!(mixer.next_sample(), 0.0);
        mixer.gain = 0.0;
        mixer.voices.push(Voice { samples: loud, position: 0 });
        assert_eq!(mixer.next_sample(), 0.0, "volume 0 is silent");
    }

    #[test]
    fn disabled_or_deviceless_player_never_panics() {
        let mut player = SoundPlayer { mixer: Arc::new(Mutex::new(Mixer { voices: vec![], gain: 1.0 })),
            stream: None, cache: HashMap::new(), enabled: true, error: None };
        player.play(Sound::MicOn); // no stream: ignored
        player.enabled = false;
        player.play(Sound::MicOn);
        assert!(player.mixer.lock().unwrap().voices.is_empty());
    }
}
