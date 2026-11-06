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
        atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const SAMPLE_RATE: u32 = 48_000;
#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(vkey: i32) -> i16;
}
/// Global keyboard state: works even while a game holds the window focus.
#[inline]
fn key_is_down(vkey: i32) -> bool {
    #[cfg(target_os = "windows")]
    { unsafe { (GetAsyncKeyState(vkey) as u16 & 0x8000) != 0 } }
    #[cfg(not(target_os = "windows"))]
    { let _ = vkey; false }
}
pub const PTT_VK: i32 = 0x77; // F8 default, user-selectable
pub const MUTE_VK: i32 = 0x78; // F9
fn allowed_to_transmit(master_on: bool, push_to_talk: bool, key_down: bool) -> bool {
    master_on && (!push_to_talk || key_down)
}
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
    send_worker: Option<thread::JoinHandle<()>>,
    receive_worker: Option<thread::JoinHandle<()>>,
    pub microphone_level: Arc<AtomicU32>,
    pub transmitting: Arc<AtomicBool>,
    pub packets_sent: Arc<AtomicU64>,
    pub packets_received: Arc<AtomicU64>,
    pub udp_errors: Arc<AtomicU64>,
    /// Independent session volume for each known user (0..=2).
    pub per_peer_volumes: Arc<Mutex<HashMap<u64, f32>>>,
}

impl Drop for VoiceSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Workers must finish before reopening UDP 45889 in a new voice session.
        // The receive socket has a short timeout and will exit within ~200 ms.
        if let Some(handle) = self.send_worker.take() { let _ = handle.join(); }
        if let Some(handle) = self.receive_worker.take() { let _ = handle.join(); }
    }
}

fn recoverable_udp_receive_error(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || error.kind() == std::io::ErrorKind::TimedOut
        || error.kind() == std::io::ErrorKind::Interrupted
        // Windows reports ICMP Port Unreachable following UDP send as 10054.
        || error.raw_os_error() == Some(10054)
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
        push_to_talk: Arc<AtomicBool>,
        ptt_key: Arc<AtomicI32>,
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
        let packets_sent = Arc::new(AtomicU64::new(0));
        let transmitting = Arc::new(AtomicBool::new(false));
        let packets_received = Arc::new(AtomicU64::new(0));
        let udp_errors = Arc::new(AtomicU64::new(0));
        let samples = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let playback = Arc::new(Mutex::new(HashMap::<u64, SpeakerBuffer>::new()));
        let per_peer_volumes = Arc::new(Mutex::new(HashMap::<u64, f32>::new()));

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
        let play_peer_volumes = Arc::clone(&per_peer_volumes);
        let output_stream = output.build_output_stream(
            &out_config,
            move |data: &mut [f32], _| {
                let Ok(mut speakers) = play_samples.lock() else {
                    data.fill(0.0);
                    return;
                };
                let volume = f32::from_bits(play_volume.load(Ordering::Relaxed)).clamp(0.0, 2.0);
                let muted = play_deafened.load(Ordering::Relaxed);
                // One lock per callback, never per sample, keeps the realtime audio path light.
                let peer_gains = play_peer_volumes.lock().ok();
                for frame in data.chunks_mut(out_channels) {
                    let mut mixed = 0f32;
                    for (id, speaker) in speakers.iter_mut() {
                        let gain = peer_gains.as_ref().and_then(|vols| vols.get(id).copied())
                            .unwrap_or(1.0).clamp(0.0, 2.0);
                        mixed += speaker.next() * gain;
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
        let sent_count = Arc::clone(&packets_sent);
        let tx_indicator = Arc::clone(&transmitting);
        let send_worker = thread::spawn(move || {
            let mut last_mute_press = false;
            let Ok(mut encoder) = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip) else {
                eprintln!("Opus encoder startup failed");
                return;
            };
            let mut payload = [0u8; 1100];
            let mut pcm = [0f32; FRAME_SIZE];
            while !stop_send.load(Ordering::Relaxed) {
                let mute_pressed = key_is_down(MUTE_VK);
                if mute_pressed && !last_mute_press {
                    mic.fetch_xor(true, Ordering::Relaxed);
                }
                last_mute_press = mute_pressed;
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
                let should_transmit = allowed_to_transmit(
                    mic.load(Ordering::Relaxed),
                    push_to_talk.load(Ordering::Relaxed),
                    key_is_down(ptt_key.load(Ordering::Relaxed)),
                );
                tx_indicator.store(should_transmit, Ordering::Relaxed);
                // No Opus work or UDP traffic while muted or PTT key is released.
                if !should_transmit { continue; }
                if let Ok(n) = encoder.encode_float(&pcm, &mut payload) {
                    let mut packet = Vec::with_capacity(8 + n);
                    packet.extend_from_slice(&my_id.to_be_bytes());
                    packet.extend_from_slice(&payload[..n]);
                    let destinations = send_peers.lock().map(|p| p.clone()).unwrap_or_default();
                    for ip in destinations {
                        let addr = SocketAddr::V4(SocketAddrV4::new(ip, AUDIO_PORT));
                        if socket.send_to(&packet, addr).is_ok() {
                            sent_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        });

        let recv_peers = Arc::clone(&peers);
        let stop_recv = Arc::clone(&stop);
        let receive_count = Arc::clone(&packets_received);
        let error_count = Arc::clone(&udp_errors);
        let receive_worker = thread::spawn(move || {
            let mut decoders: HashMap<u64, Decoder> = HashMap::new();
            let mut last_audio: HashMap<u64, Instant> = HashMap::new();
            let mut last_cleanup = Instant::now();
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
                                receive_count.fetch_add(1, Ordering::Relaxed);
                                last_audio.insert(id, Instant::now());
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) if recoverable_udp_receive_error(&e) => {
                        if e.raw_os_error() == Some(10054) {
                            error_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(e) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                        eprintln!("Friends Voice UDP receive error (retrying): {e}");
                        // Do not permanently kill audio reception after a transient network
                        // failure. This is especially important during Radmin VPN changes.
                        thread::sleep(Duration::from_millis(80));
                    },
                }
                // A user who disappears should not leave an Opus decoder and jitter buffer
                // in memory for the rest of a long-running room session.
                if last_cleanup.elapsed() >= Duration::from_secs(5) {
                    last_audio.retain(|_, t| t.elapsed() < Duration::from_secs(10));
                    decoders.retain(|id, _| last_audio.contains_key(id));
                    if let Ok(mut speakers) = playback.lock() {
                        speakers.retain(|id, _| last_audio.contains_key(id));
                    }
                    last_cleanup = Instant::now();
                }
            }
        });
        Ok(Self {
            _input: input_stream,
            _output: output_stream,
            stop,
            send_worker: Some(send_worker),
            receive_worker: Some(receive_worker),
            microphone_level: level,
            transmitting,
            packets_sent,
            packets_received,
            udp_errors,
            per_peer_volumes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptt_logic() {
        assert!(allowed_to_transmit(true, false, false));
        assert!(!allowed_to_transmit(false, false, true));
        assert!(!allowed_to_transmit(true, true, false));
        assert!(allowed_to_transmit(true, true, true));
    }

    #[test]
    fn windows_udp_icmp_reset_is_recoverable() {
        let error = std::io::Error::from_raw_os_error(10054);
        assert!(recoverable_udp_receive_error(&error));
    }

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
