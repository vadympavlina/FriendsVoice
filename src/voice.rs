use crate::protocol::AUDIO_PORT;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, SampleRate, Stream, StreamConfig,
};
use opus::{Application, Channels, Decoder, Encoder};
use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

const SAMPLE_RATE: u32 = 48_000;
const FRAME_SIZE: usize = 960; // 20 ms at 48 kHz
const MAX_AUDIO_QUEUE: usize = 8 * FRAME_SIZE;

#[derive(Default)]
struct SpeakerBuffer {
    samples: VecDeque<f32>,
    primed: bool,
}

impl SpeakerBuffer {
    fn add(&mut self, decoded: &[f32]) {
        self.samples.extend(decoded.iter().copied());
        while self.samples.len() > MAX_AUDIO_QUEUE {
            self.samples.pop_front();
        }
    }

    fn next(&mut self) -> f32 {
        if !self.primed && self.samples.len() >= 2 * FRAME_SIZE {
            self.primed = true;
        }
        if !self.primed {
            return 0.0;
        }
        match self.samples.pop_front() {
            Some(sample) => sample,
            None => {
                self.primed = false;
                0.0
            }
        }
    }
}

/// Native audio capture/playback. Stream fields keep WASAPI devices alive while joined.
pub struct VoiceSession {
    _input: Stream,
    _output: Stream,
    stop: Arc<AtomicBool>,
    pub microphone_level: Arc<AtomicU32>,
}

impl Drop for VoiceSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut result: Vec<String> = host.input_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    result.sort();
    result.dedup();
    result
}

pub fn list_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut result: Vec<String> = host.output_devices()
        .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    result.sort();
    result.dedup();
    result
}

fn input_config(device: &cpal::Device) -> Result<StreamConfig, String> {
    device.supported_input_configs()
        .map_err(|e| format!("Налаштування мікрофона: {e}"))?
        .find(|r| r.sample_format() == SampleFormat::F32
            && r.min_sample_rate().0 <= SAMPLE_RATE
            && r.max_sample_rate().0 >= SAMPLE_RATE)
        .map(|r| StreamConfig {
            channels: r.channels(),
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        })
        .ok_or_else(|| "Мікрофон не підтримує 48 кГц / F32. Вибери інший пристрій.".to_string())
}

fn output_config(device: &cpal::Device) -> Result<StreamConfig, String> {
    device.supported_output_configs()
        .map_err(|e| format!("Налаштування динаміків: {e}"))?
        .find(|r| r.sample_format() == SampleFormat::F32
            && r.min_sample_rate().0 <= SAMPLE_RATE
            && r.max_sample_rate().0 >= SAMPLE_RATE)
        .map(|r| StreamConfig {
            channels: r.channels(),
            sample_rate: SampleRate(SAMPLE_RATE),
            buffer_size: cpal::BufferSize::Default,
        })
        .ok_or_else(|| "Динаміки не підтримують 48 кГц / F32. Вибери інший пристрій.".to_string())
}

impl VoiceSession {
    pub fn start(
        my_id: u64,
        peers: Arc<Mutex<Vec<Ipv4Addr>>>,
        input_name: &str,
        output_name: &str,
        mic_enabled: Arc<AtomicBool>,
        deafened: Arc<AtomicBool>,
        volume: Arc<AtomicU32>,
    ) -> Result<Self, String> {
        let socket = UdpSocket::bind(("0.0.0.0", AUDIO_PORT))
            .map_err(|e| format!("Не вдалося відкрити голосовий P2P UDP-порт {AUDIO_PORT}: {e}"))?;
        socket.set_read_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| e.to_string())?;
        let receiver = socket.try_clone().map_err(|e| e.to_string())?;

        let host = cpal::default_host();
        let input = if input_name.is_empty() {
            host.default_input_device()
        } else {
            host.input_devices().ok().and_then(|mut devices| {
                devices.find(|d| d.name().ok().as_deref() == Some(input_name))
            }).or_else(|| host.default_input_device())
        }.ok_or("Не знайдено мікрофон")?;
        let output = if output_name.is_empty() {
            host.default_output_device()
        } else {
            host.output_devices().ok().and_then(|mut devices| {
                devices.find(|d| d.name().ok().as_deref() == Some(output_name))
            }).or_else(|| host.default_output_device())
        }.ok_or("Не знайдено пристрій відтворення")?;

        let in_config = input_config(&input)?;
        let out_config = output_config(&output)?;
        let in_channels = in_config.channels as usize;
        let out_channels = out_config.channels as usize;

        let stop = Arc::new(AtomicBool::new(false));
        let level = Arc::new(AtomicU32::new(0f32.to_bits()));
        let samples = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let playback = Arc::new(Mutex::new(HashMap::<u64, SpeakerBuffer>::new()));

        let capture_samples = Arc::clone(&samples);
        let capture_level = Arc::clone(&level);
        let input_stream = input.build_input_stream(
            &in_config,
            move |buffer: &[f32], _| {
                let mut queue = match capture_samples.lock() {
                    Ok(q) => q,
                    Err(_) => return,
                };
                let mut sum_sq = 0f32;
                let mut frames = 0usize;
                for channels in buffer.chunks(in_channels) {
                    let mono = channels.iter().copied().sum::<f32>() / channels.len() as f32;
                    sum_sq += mono * mono;
                    frames += 1;
                    queue.push_back(mono);
                }
                while queue.len() > SAMPLE_RATE as usize / 2 {
                    queue.pop_front();
                }
                let rms = (sum_sq / frames.max(1) as f32).sqrt().min(1.0);
                capture_level.store(rms.to_bits(), Ordering::Relaxed);
            },
            |err| eprintln!("Audio input: {err}"),
            None,
        ).map_err(|e| format!("Мікрофон: {e}"))?;

        let play_samples = Arc::clone(&playback);
        let play_deafened = Arc::clone(&deafened);
        let play_volume = Arc::clone(&volume);
        let output_stream = output.build_output_stream(
            &out_config,
            move |data: &mut [f32], _| {
                let Ok(mut speakers) = play_samples.lock() else {
                    data.fill(0.0);
                    return;
                };
                let volume = f32::from_bits(play_volume.load(Ordering::Relaxed)).clamp(0.0, 2.0);
                let muted = play_deafened.load(Ordering::Relaxed);
                for frame in data.chunks_mut(out_channels) {
                    let mut mixed = 0f32;
                    for speaker in speakers.values_mut() {
                        mixed += speaker.next();
                    }
                    let sample = if muted { 0.0 } else { (mixed * volume).clamp(-1.0, 1.0) };
                    frame.fill(sample);
                }
            },
            |err| eprintln!("Audio output: {err}"),
            None,
        ).map_err(|e| format!("Відтворення: {e}"))?;

        input_stream.play().map_err(|e| format!("Запуск мікрофона: {e}"))?;
        output_stream.play().map_err(|e| format!("Запуск динаміків: {e}"))?;

        let mic = Arc::clone(&mic_enabled);
        let send_peers = Arc::clone(&peers);
        let stop_send = Arc::clone(&stop);
        thread::spawn(move || {
            let Ok(mut encoder) = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip) else {
                eprintln!("Opus encoder startup failed");
                return;
            };
            let mut payload = [0u8; 1100];
            let mut pcm = [0f32; FRAME_SIZE];
            while !stop_send.load(Ordering::Relaxed) {
                let ready = {
                    let Ok(mut queue) = samples.lock() else { break; };
                    if queue.len() < FRAME_SIZE {
                        false
                    } else {
                        for sample in &mut pcm {
                            *sample = queue.pop_front().unwrap_or(0.0);
                        }
                        true
                    }
                };
                if !ready {
                    thread::sleep(Duration::from_millis(4));
                    continue;
                }
                if !mic.load(Ordering::Relaxed) {
                    pcm.fill(0.0);
                }
                if let Ok(n) = encoder.encode_float(&pcm, &mut payload) {
                    let mut packet = Vec::with_capacity(8 + n);
                    packet.extend_from_slice(&my_id.to_be_bytes());
                    packet.extend_from_slice(&payload[..n]);
                    let destinations = send_peers.lock().map(|p| p.clone()).unwrap_or_default();
                    for ip in destinations {
                        let addr = SocketAddr::V4(SocketAddrV4::new(ip, AUDIO_PORT));
                        let _ = socket.send_to(&packet, addr);
                    }
                }
            }
        });

        let recv_peers = Arc::clone(&peers);
        let stop_recv = Arc::clone(&stop);
        thread::spawn(move || {
            let mut decoders: HashMap<u64, Decoder> = HashMap::new();
            let mut input = [0u8; 1500];
            let mut decoded = [0f32; 5760];
            while !stop_recv.load(Ordering::Relaxed) {
                match receiver.recv_from(&mut input) {
                    Ok((n, addr)) if n > 8 && addr.port() == AUDIO_PORT && {
                        match addr.ip() {
                            IpAddr::V4(ip) => recv_peers.lock()
                                .map(|known| known.contains(&ip)).unwrap_or(false),
                            _ => false,
                        }
                    } => {
                        let mut id_bytes = [0u8; 8];
                        id_bytes.copy_from_slice(&input[..8]);
                        let id = u64::from_be_bytes(id_bytes);
                        if let std::collections::hash_map::Entry::Vacant(e) = decoders.entry(id) {
                            let Ok(decoder) = Decoder::new(SAMPLE_RATE, Channels::Mono) else { continue; };
                            e.insert(decoder);
                        }
                        let Some(decoder) = decoders.get_mut(&id) else { continue; };
                        if let Ok(samples) = decoder.decode_float(&input[8..n], &mut decoded, false) {
                            if let Ok(mut speakers) = playback.lock() {
                                speakers.entry(id).or_default().add(&decoded[..samples]);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            _input: input_stream,
            _output: output_stream,
            stop,
            microphone_level: level,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_roundtrip() {
        let mut encoder = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip).unwrap();
        let mut decoder = Decoder::new(SAMPLE_RATE, Channels::Mono).unwrap();
        let wave: Vec<f32> = (0..FRAME_SIZE)
            .map(|i| ((i as f32 / 48_000.0) * 440.0 * std::f32::consts::TAU).sin() * 0.2)
            .collect();
        let mut packet = [0u8; 1100];
        let size = encoder.encode_float(&wave, &mut packet).unwrap();
        let mut decoded = [0f32; FRAME_SIZE * 2];
        assert_eq!(decoder.decode_float(&packet[..size], &mut decoded, false).unwrap(), FRAME_SIZE);
    }
}
