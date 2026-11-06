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
/// Speech gate is opt-in. Hold a short tail after speech so quiet word endings
/// are not clipped and verify that Push-to-Talk always takes precedence.
struct SpeechGate {
    silent_frames: usize,
}
impl SpeechGate {
    fn new() -> Self { Self { silent_frames: 20 } }
    fn allow(&mut self, samples: &[f32], threshold: f32) -> bool {
        let energy = samples.iter().map(|x| x * x).sum::<f32>() / samples.len().max(1) as f32;
        let active = energy.sqrt() >= threshold.clamp(0.002, 0.12);
        if active { self.silent_frames = 0; }
        else { self.silent_frames = self.silent_frames.saturating_add(1); }
        active || self.silent_frames <= 12 // 240ms hold time
    }
}
const FRAME_SIZE: usize = 960; // 20 ms at 48 kHz
// Never let a temporary WASAPI stall build up half a second of stale speech.
const MAX_AUDIO_QUEUE: usize = 6 * FRAME_SIZE;
const MAX_CAPTURE_QUEUE: usize = 6 * FRAME_SIZE;
const AUDIO_HEADER_BYTES: usize = 20; // 64-bit peer id + 64-bit call id + 32-bit sequence
/// Circular sequence comparison; reordered and duplicate UDP packets must not re-decode Opus.
fn sequence_is_newer(candidate: u32, previous: u32) -> bool {
    let distance = candidate.wrapping_sub(previous);
    distance != 0 && distance < (1u32 << 31)
}
fn parse_audio_packet(packet: &[u8]) -> Option<(u64, u64, u32, &[u8])> {
    if packet.len() <= AUDIO_HEADER_BYTES { return None; }
    let user_id = u64::from_be_bytes(packet[0..8].try_into().ok()?);
    let session_id = u64::from_be_bytes(packet[8..16].try_into().ok()?);
    let seq = u32::from_be_bytes(packet[16..20].try_into().ok()?);
    Some((user_id, session_id, seq, &packet[AUDIO_HEADER_BYTES..]))
}
struct OpusStream {
    decoder: Decoder,
    session_id: u64,
    last_seq: u32,
}


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
    pub duplicate_packets: Arc<AtomicU64>,
    pub concealed_frames: Arc<AtomicU64>,
    /// Independent session volume for each known user (0..=2).
    pub per_peer_volumes: Arc<Mutex<HashMap<u64, f32>>>,
    active_speakers: Arc<Mutex<HashMap<u64, Instant>>>,
}

impl VoiceSession {
    pub fn is_speaking(&self, peer: u64) -> bool {
        self.active_speakers.lock().ok()
            .and_then(|s| s.get(&peer).copied())
            .is_some_and(|t| t.elapsed() < Duration::from_millis(280))
    }
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
        noise_gate: Arc<AtomicBool>,
        gate_threshold: Arc<AtomicU32>,
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
        let duplicate_packets = Arc::new(AtomicU64::new(0));
        let concealed_frames = Arc::new(AtomicU64::new(0));
        let samples = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let playback = Arc::new(Mutex::new(HashMap::<u64, SpeakerBuffer>::new()));
        let per_peer_volumes = Arc::new(Mutex::new(HashMap::<u64, f32>::new()));
        let active_speakers = Arc::new(Mutex::new(HashMap::<u64, Instant>::new()));

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
                while queue.len() > MAX_CAPTURE_QUEUE {
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
            let mut speech_gate = SpeechGate::new();
            let Ok(mut encoder) = Encoder::new(SAMPLE_RATE, Channels::Mono, Application::Voip) else {
                eprintln!("Opus encoder startup failed");
                return;
            };
            let stream_id = rand::random::<u64>().max(1);
            let mut sequence = 0u32;
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
                let ptt = push_to_talk.load(Ordering::Relaxed);
                let mut should_transmit = allowed_to_transmit(
                    mic.load(Ordering::Relaxed),
                    ptt,
                    key_is_down(ptt_key.load(Ordering::Relaxed)),
                );
                // Optional noise gate only in always-on mode. In PTT mode the key
                // decides when to transmit, even if the player speaks quietly.
                if should_transmit && !ptt && noise_gate.load(Ordering::Relaxed) {
                    let threshold = f32::from_bits(gate_threshold.load(Ordering::Relaxed));
                    should_transmit = speech_gate.allow(&pcm, threshold);
                }
                tx_indicator.store(should_transmit, Ordering::Relaxed);
                // No Opus work or UDP traffic while muted or PTT key is released.
                if !should_transmit { continue; }
                if let Ok(n) = encoder.encode_float(&pcm, &mut payload) {
                    let mut packet = Vec::with_capacity(AUDIO_HEADER_BYTES + n);
                    packet.extend_from_slice(&my_id.to_be_bytes());
                    packet.extend_from_slice(&stream_id.to_be_bytes());
                    packet.extend_from_slice(&sequence.to_be_bytes());
                    packet.extend_from_slice(&payload[..n]);
                    sequence = sequence.wrapping_add(1);
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
        let speaking_ref = Arc::clone(&active_speakers);
        let dropped_counter = Arc::clone(&duplicate_packets);
        let concealed_counter = Arc::clone(&concealed_frames);
        let receive_worker = thread::spawn(move || {
            let mut decoders: HashMap<u64, OpusStream> = HashMap::new();
            let mut last_audio: HashMap<u64, Instant> = HashMap::new();
            let mut last_cleanup = Instant::now();
            let mut input = [0u8; 1500];
            let mut decoded = [0f32; 5760];
            while !stop_recv.load(Ordering::Relaxed) {
                match receiver.recv_from(&mut input) {
                    Ok((n, addr)) if n > AUDIO_HEADER_BYTES && addr.port() == AUDIO_PORT && {
                        match addr.ip() {
                            IpAddr::V4(ip) => recv_peers.lock()
                                .map(|known| known.contains(&ip)).unwrap_or(false),
                            _ => false,
                        }
                    } => {
                        let Some((id, session_id, seq, compressed)) =
                            parse_audio_packet(&input[..n]) else { continue };
                        if id == my_id { continue; }
                        let session_changed = !decoders.get(&id)
                            .is_some_and(|d| d.session_id == session_id);
                        if session_changed {
                            let Ok(decoder) = Decoder::new(SAMPLE_RATE, Channels::Mono) else { continue; };
                            decoders.insert(id, OpusStream {
                                decoder, session_id, last_seq: seq.wrapping_sub(1),
                            });
                            // Never mix stale audio from a previous leave/rejoin session.
                            if let Ok(mut speakers) = playback.lock() {
                                speakers.remove(&id);
                            }
                        }
                        let Some(stream) = decoders.get_mut(&id) else { continue };
                        if !sequence_is_newer(seq, stream.last_seq) {
                            dropped_counter.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                        let missing = seq.wrapping_sub(stream.last_seq).saturating_sub(1);
                        // Very short packet loss can be concealed by Opus itself. For larger
                        // outages reset playback latency rather than playing stale buffered voice.
                        if missing > 0 && missing <= 2 {
                            let mut plc = [0f32; FRAME_SIZE];
                            for _ in 0..missing {
                                if let Ok(samples) = stream.decoder.decode_float(&[], &mut plc, false) {
                                    if let Ok(mut speakers) = playback.lock() {
                                        speakers.entry(id).or_default().add(&plc[..samples]);
                                    }
                                    concealed_counter.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        } else if missing > 2 {
                            if let Ok(mut speakers) = playback.lock() {
                                speakers.remove(&id);
                            }
                        }
                        stream.last_seq = seq;
                        if let Ok(samples) = stream.decoder.decode_float(compressed, &mut decoded, false) {
                            // Track audible speech, not merely that a peer is connected.
                            let rms = (decoded[..samples].iter().map(|v| v*v).sum::<f32>()
                                / samples.max(1) as f32).sqrt();
                            if rms > 0.012 {
                                if let Ok(mut active) = speaking_ref.lock() {
                                    active.insert(id, Instant::now());
                                }
                            }
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
                    if let Ok(mut activity) = speaking_ref.lock() {
                        activity.retain(|id, t| t.elapsed() < Duration::from_secs(10)
                            && last_audio.contains_key(id));
                    }
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
            duplicate_packets,
            concealed_frames,
            per_peer_volumes,
            active_speakers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_packet_header_and_ordering() {
        let mut packet = Vec::new();
        packet.extend_from_slice(&5u64.to_be_bytes());
        packet.extend_from_slice(&42u64.to_be_bytes());
        packet.extend_from_slice(&u32::MAX.to_be_bytes());
        packet.extend_from_slice(&[0x90, 0xFA, 0x12]);
        let (user, session, seq, compressed) = parse_audio_packet(&packet).unwrap();
        assert_eq!((user,session,seq),(5,42,u32::MAX));
        assert_eq!(compressed, &[0x90,0xFA,0x12]);
        assert!(parse_audio_packet(&packet[..AUDIO_HEADER_BYTES]).is_none());
        assert!(sequence_is_newer(0, u32::MAX)); // wrap
        assert!(!sequence_is_newer(u32::MAX, 0)); // late
        assert!(!sequence_is_newer(100, 100)); // duplicate
        assert!(sequence_is_newer(102, 100));
    }

    #[test]
    fn noise_gate_keeps_short_word_endings_and_saves_silence() {
        let mut gate = SpeechGate::new();
        let silent = [0.0f32; FRAME_SIZE];
        let voice = [0.035f32; FRAME_SIZE];
        assert!(!gate.allow(&silent, 0.02));
        assert!(gate.allow(&voice, 0.02));
        for _ in 0..12 { assert!(gate.allow(&silent, 0.02)); }
        assert!(!gate.allow(&silent, 0.02));
    }

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
