//! Lightweight native Windows webcam video for small, trusted Radmin VPN rooms.
//! Camera is ON ONLY when explicitly enabled by the user. JPEG uses low resolution
//! and a strict bandwidth cap; this is a beta transport, not a low-latency H.264 stack.
use crate::protocol::VIDEO_PORT;
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, ExtendedColorType, ImageFormat, RgbImage};
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution},
    Camera,
};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

// Three practical presets: stability first, then smoothness, then detail.
fn video_preset(mode: u32) -> (u32, u32, u64, u8) {
    match mode {
        0 => (320, 180, 5, 42),  // Saver
        2 => (480, 270, 8, 50), // Sharper, limited to 8 fps
        _ => (320, 180, 9, 54), // Balanced (default)
    }
}
const MAX_JPEG_BYTES: usize = 36_000;
const CHUNK: usize = 960;
const HEADER: usize = 15;
const MAX_CHUNKS: usize = 40;
const MAGIC: u8 = 0xF5;

#[derive(Debug)]
pub struct VideoFrame {
    pub sender_id: u64,
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}
#[derive(Debug)]
pub enum VideoEvent {
    Frame(VideoFrame),
    Error(String),
}
struct Assembly {
    seq: u32,
    parts: Vec<Option<Vec<u8>>>,
    /// One XOR parity fragment can reconstruct one missing UDP packet.
    parity: Option<(usize, Vec<u8>)>,
    updated: Instant,
}
impl Assembly {
    fn new(seq: u32, total: usize) -> Self {
        Self {
            seq,
            parts: vec![None; total],
            parity: None,
            updated: Instant::now(),
        }
    }
    fn insert(&mut self, part: usize, bytes: &[u8]) -> Option<Vec<u8>> {
        let count = self.parts.len();
        if part == count {
            // Parity payload: 4-byte original JPEG length plus CHUNK XOR bytes.
            if count < 2 || bytes.len() != CHUNK + 4 { return None; }
            let total_len = u32::from_be_bytes(bytes[..4].try_into().ok()?) as usize;
            if total_len > MAX_JPEG_BYTES
                || total_len <= (count - 1) * CHUNK
                || total_len > count * CHUNK { return None; }
            self.parity = Some((total_len, bytes[4..].to_vec()));
        } else {
            if part >= count || bytes.is_empty() || bytes.len() > CHUNK { return None; }
            self.parts[part].get_or_insert_with(|| bytes.to_vec());
        }
        self.updated = Instant::now();
        let missing = self.parts.iter().enumerate()
            .filter_map(|(i, p)| p.is_none().then_some(i))
            .collect::<Vec<_>>();
        if missing.len() == 1 {
            if let Some((total_len, xor)) = self.parity.as_ref() {
                let index = missing[0];
                let mut recovered = xor.clone();
                for piece in self.parts.iter().filter_map(|p| p.as_ref()) {
                    for (i, byte) in piece.iter().enumerate() {
                        recovered[i] ^= byte;
                    }
                }
                let expected = if index == count - 1 {
                    total_len - (count - 1) * CHUNK
                } else { CHUNK };
                recovered.truncate(expected);
                self.parts[index] = Some(recovered);
            }
        }
        if self.parts.iter().any(Option::is_none) { return None; }
        let mut result = Vec::with_capacity(count * CHUNK);
        for part in self.parts.iter_mut() {
            result.extend_from_slice(part.take().as_ref()?);
        }
        (result.len() <= MAX_JPEG_BYTES).then_some(result)
    }
}
// Wrapping sequence order handles a long-running call and ignores delayed fragments.
fn seq_newer(candidate: u32, previous: u32) -> bool {
    let difference = candidate.wrapping_sub(previous);
    difference != 0 && difference < (1u32 << 31)
}
fn make_fragments(id: u64, seq: u32, jpg: &[u8]) -> Option<Vec<Vec<u8>>> {
    if jpg.is_empty() || jpg.len() > MAX_JPEG_BYTES { return None; }
    let total = jpg.len().div_ceil(CHUNK);
    if total > MAX_CHUNKS { return None; }
    let mut result = Vec::with_capacity(total + usize::from(total > 1));
    let mut parity = [0u8; CHUNK];
    for (part, bytes) in jpg.chunks(CHUNK).enumerate() {
        for (i, byte) in bytes.iter().enumerate() {
            parity[i] ^= byte;
        }
        let mut datagram = Vec::with_capacity(HEADER + bytes.len());
        datagram.push(MAGIC);
        datagram.extend_from_slice(&id.to_be_bytes());
        datagram.extend_from_slice(&seq.to_be_bytes());
        datagram.push(part as u8);
        datagram.push(total as u8);
        datagram.extend_from_slice(bytes);
        result.push(datagram);
    }
    if total > 1 {
        // Backwards compatible: older receivers ignore part == total.
        let mut datagram = Vec::with_capacity(HEADER + 4 + CHUNK);
        datagram.push(MAGIC);
        datagram.extend_from_slice(&id.to_be_bytes());
        datagram.extend_from_slice(&seq.to_be_bytes());
        datagram.push(total as u8); // reserved index identifies parity
        datagram.push(total as u8);
        datagram.extend_from_slice(&(jpg.len() as u32).to_be_bytes());
        datagram.extend_from_slice(&parity);
        result.push(datagram);
    }
    Some(result)
}
fn parse_fragment(datagram: &[u8]) -> Option<(u64, u32, usize, usize, &[u8])> {
    if datagram.len() <= HEADER || datagram.len() > HEADER + CHUNK + 4 || datagram[0] != MAGIC { return None; }
    let sender = u64::from_be_bytes(datagram[1..9].try_into().ok()?);
    let seq = u32::from_be_bytes(datagram[9..13].try_into().ok()?);
    let part = datagram[13] as usize;
    let total = datagram[14] as usize;
    if total == 0 || total > MAX_CHUNKS || part > total { return None; }
    let payload = &datagram[HEADER..];
    if part == total {
        if total < 2 || payload.len() != CHUNK + 4 { return None; }
    } else if payload.is_empty() || payload.len() > CHUNK { return None; }
    Some((sender, seq, part, total, payload))
}
fn decode_jpeg(id: u64, data: &[u8]) -> Option<VideoFrame> {
    if data.len() > MAX_JPEG_BYTES || data.is_empty() { return None; }
    let image = image::load_from_memory_with_format(data, ImageFormat::Jpeg).ok()?.to_rgb8();
    if !matches!((image.width(), image.height()), (320, 180) | (480, 270)) { return None; }
    Some(VideoFrame {
        sender_id: id, width: image.width() as usize, height: image.height() as usize,
        rgb: image.into_raw(),
    })
}
fn webcam_loop(
    active: Arc<AtomicBool>,
    id: u64,
    targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    events: SyncSender<VideoEvent>,
    frames_sent: Arc<AtomicU64>,
    camera_index: u32,
    quality: Arc<AtomicU32>,
) {
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(640, 480), FrameFormat::MJPEG, 15)
    ));
    let mut camera = match Camera::new(CameraIndex::Index(camera_index), requested) {
        Ok(camera) => camera,
        Err(error) => {
            let _ = events.try_send(VideoEvent::Error(format!("Камера недоступна: {error}")));
            return;
        }
    };
    if let Err(error) = camera.open_stream() {
        let _ = events.try_send(VideoEvent::Error(format!("Не вдається увімкнути камеру: {error}")));
        return;
    }
    let socket = match UdpSocket::bind(("0.0.0.0", 0)) {
        Ok(socket) => socket,
        Err(error) => {
            let _ = events.try_send(VideoEvent::Error(format!("Порт передачі камери: {error}")));
            return;
        }
    };
    let mut seq = 0u32;
    let mut last_sent: Option<Vec<u8>> = None;
    let mut last_send_time = Instant::now() - Duration::from_secs(2);
    while active.load(Ordering::Relaxed) {
        let start = Instant::now();
        let (width, height, fps, jpeg_quality) =
            video_preset(quality.load(Ordering::Relaxed));
        let frame = match camera.frame() {
            Ok(frame) => frame,
            Err(error) => {
                let _ = events.try_send(VideoEvent::Error(format!("Помилка кадру з камери: {error}")));
                break;
            }
        };
        if !active.load(Ordering::Relaxed) { break; }
        let decoded = match frame.decode_image::<RgbFormat>() {
            Ok(image) => image,
            Err(error) => {
                let _ = events.try_send(VideoEvent::Error(format!("Декодування камери: {error}")));
                continue;
            }
        };
        // Resize *before* encoding, avoiding expensive full-resolution video transmission.
        let small: RgbImage = image::imageops::resize(&decoded, width, height, FilterType::Triangle);
        // Keep the preview responsive, but reduce traffic when almost nothing moves.
        let _ = events.try_send(VideoEvent::Frame(VideoFrame {
            sender_id: id, width: width as usize, height: height as usize,
            rgb: small.as_raw().clone(),
        }));
        let has_motion = last_sent.as_ref().is_none_or(|old| {
            let next = small.as_raw();
            if old.len() != next.len() { return true; }
            let mut delta = 0u32;
            let mut count = 0u32;
            for (a, b) in old.iter().zip(next.iter()).step_by(67) {
                delta += a.abs_diff(*b) as u32;
                count += 1;
            }
            delta > count.saturating_mul(4)
        });
        let should_send = has_motion || last_send_time.elapsed() >= Duration::from_millis(950);
        let ips = targets.lock().map(|t|t.clone()).unwrap_or_default();
        if !ips.is_empty() && should_send {
            // A higher-quality frame may exceed the VPN-safe bound. Retry at lower quality.
            for jpeg_quality in [jpeg_quality, 40, 30] {
                let mut compressed = Vec::new();
                let encoded = JpegEncoder::new_with_quality(&mut compressed, jpeg_quality)
                    .encode(small.as_raw(), width, height, ExtendedColorType::Rgb8);
                if encoded.is_err() || compressed.len() > MAX_JPEG_BYTES { continue; }
                if let Some(chunks) = make_fragments(id, seq, &compressed) {
                    for ip in &ips {
                        let addr = SocketAddr::V4(SocketAddrV4::new(*ip, VIDEO_PORT));
                        for part in &chunks {
                            let _ = socket.send_to(part, addr);
                        }
                    }
                    last_sent = Some(small.as_raw().clone());
                    last_send_time = Instant::now();
                    frames_sent.fetch_add(1, Ordering::Relaxed);
                    seq = seq.wrapping_add(1);
                    break;
                }
            }
        }
        let interval = Duration::from_millis(1000 / fps);
        if let Some(rest) = interval.checked_sub(start.elapsed()) {
            thread::sleep(rest);
        }
    }
    let _ = camera.stop_stream();
}
pub struct VideoSession {
    active: Arc<AtomicBool>,
    recv_stop: Arc<AtomicBool>,
    webcam_worker: Option<JoinHandle<()>>,
    recv_worker: Option<JoinHandle<()>>,
    events_tx: SyncSender<VideoEvent>,
    events_rx: Receiver<VideoEvent>,
    local_id: u64,
    targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    camera_index: Arc<AtomicU32>,
    quality: Arc<AtomicU32>,
    pub frames_sent: Arc<AtomicU64>,
    pub frames_received: Arc<AtomicU64>,
}
impl VideoSession {
    pub fn start(id: u64, peers: Arc<Mutex<Vec<Ipv4Addr>>>,
        camera_index: Arc<AtomicU32>, quality: Arc<AtomicU32>) -> Result<Self, String> {
        let rx_socket = UdpSocket::bind(("0.0.0.0", VIDEO_PORT))
            .map_err(|e| format!("Не вдалося відкрити UDP-порт камери {VIDEO_PORT}: {e}"))?;
        rx_socket.set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|e| e.to_string())?;
        let (events_tx, events_rx) = mpsc::sync_channel(12);
        let recv_stop = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&recv_stop);
        let tx = events_tx.clone();
        let valid_peers = Arc::clone(&peers);
        let frames_received = Arc::new(AtomicU64::new(0));
        let rx_count = Arc::clone(&frames_received);
        let worker = thread::spawn(move || {
            let mut packets = [0u8; 1500];
            let mut partial: HashMap<IpAddr, Assembly> = HashMap::new();
            let mut completed: HashMap<IpAddr, (u64, u32, Instant)> = HashMap::new();
            while !stop.load(Ordering::Relaxed) {
                match rx_socket.recv_from(&mut packets) {
                    Ok((size, source)) => {
                        let IpAddr::V4(ip) = source.ip() else { continue; };
                        if !valid_peers.lock().map(|p| p.contains(&ip)).unwrap_or(false) { continue; }
                        let Some((id, seq, part, total, payload)) = parse_fragment(&packets[..size]) else { continue; };
                        // Older fragments should never replace a newer in-progress frame.
                        if let Some((previous_id, previous_seq, time)) = completed.get(&source.ip()) {
                            if *previous_id == id && time.elapsed() < Duration::from_secs(2)
                                && !seq_newer(seq, *previous_seq) { continue; }
                        }
                        let entry = partial.entry(source.ip())
                            .or_insert_with(|| Assembly::new(seq, total));
                        if entry.seq != seq || entry.parts.len() != total {
                            if entry.updated.elapsed() < Duration::from_millis(700)
                                && !seq_newer(seq, entry.seq) { continue; }
                            *entry = Assembly::new(seq, total);
                        }
                        if let Some(jpeg) = entry.insert(part, payload) {
                            partial.remove(&source.ip());
                            if let Some(frame) = decode_jpeg(id, &jpeg) {
                                completed.insert(source.ip(), (id, seq, Instant::now()));
                                if tx.try_send(VideoEvent::Frame(frame)).is_ok() {
                                    rx_count.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::TimedOut
                        || error.kind() == std::io::ErrorKind::WouldBlock
                        || error.raw_os_error() == Some(10054) => {}
                    Err(error) => {
                        eprintln!("Video UDP: {error}");
                        thread::sleep(Duration::from_millis(70));
                    }
                }
                partial.retain(|_,a|a.updated.elapsed() < Duration::from_millis(800));
                completed.retain(|_, (_,_,time)| time.elapsed() < Duration::from_secs(2));
            }
        });
        Ok(Self {
            active: Arc::new(AtomicBool::new(false)),
            recv_stop,
            webcam_worker: None,
            recv_worker: Some(worker),
            events_tx, events_rx, local_id: id, targets: peers,
            camera_index, quality,
            frames_sent: Arc::new(AtomicU64::new(0)),
            frames_received,
        })
    }
    pub fn enabled(&self) -> bool { self.active.load(Ordering::Relaxed) }
    pub fn enable(&mut self) {
        if self.enabled() { return; }
        self.disable();
        self.active = Arc::new(AtomicBool::new(true));
        let stop = Arc::clone(&self.active);
        let tx = self.events_tx.clone();
        let id = self.local_id;
        let peers = Arc::clone(&self.targets);
        let count = Arc::clone(&self.frames_sent);
        let camera_index = self.camera_index.load(Ordering::Relaxed);
        let quality = Arc::clone(&self.quality);
        self.webcam_worker = Some(thread::spawn(move || {
            webcam_loop(Arc::clone(&stop), id, peers, tx, count, camera_index, quality);
            stop.store(false, Ordering::Relaxed);
        }));
    }
    pub fn disable(&mut self) {
        self.active.store(false, Ordering::Relaxed);
        if let Some(worker) = self.webcam_worker.take() { let _ = worker.join(); }
    }
    pub fn poll_events(&mut self) -> Vec<VideoEvent> {
        let mut found = Vec::new();
        while let Ok(event) = self.events_rx.try_recv() {
            found.push(event);
            if found.len() >= 12 { break; }
        }
        found
    }
}
impl Drop for VideoSession {
    fn drop(&mut self) {
        self.disable();
        self.recv_stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.recv_worker.take() { let _ = worker.join(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sequence_and_presets_are_bounded() {
        assert!(seq_newer(0, u32::MAX));
        assert!(!seq_newer(5, 10));
        assert!(!seq_newer(20, 20));
        assert_eq!(video_preset(0), (320, 180, 5, 42));
        assert_eq!(video_preset(2), (480, 270, 8, 50));
        assert_eq!(video_preset(99), (320, 180, 9, 54));
    }
    #[test]
    fn fragments_roundtrip_and_out_of_order() {
        let jpg = (0..10_987).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let parts = make_fragments(7654, 99, &jpg).unwrap();
        assert!(parts.len()>2);
        let total = parts.len() - 1;
        let mut assembly = Assembly::new(99, total);
        let mut result = None;
        for part in parts.iter().rev() {
            let (id,seq,idx,count,payload) = parse_fragment(part).unwrap();
            assert_eq!((id,seq,count),(7654,99,total));
            if let Some(done) = assembly.insert(idx,payload) { result = Some(done); }
        }
        assert_eq!(result.unwrap(),jpg);
    }

    #[test]
    fn parity_recovers_one_lost_fragment_any_position() {
        let jpg = (0..17_777).map(|i| ((i * 17) % 251) as u8).collect::<Vec<_>>();
        let fragments = make_fragments(45, 17, &jpg).unwrap();
        let total = fragments.len() - 1;
        for dropped in [0, total / 2, total - 1] {
            let mut assembly = Assembly::new(17, total);
            let mut reconstructed = None;
            for (i, fragment) in fragments.iter().enumerate().rev() {
                if i == dropped { continue; }
                let (_, _, part, _, payload) = parse_fragment(fragment).unwrap();
                if let Some(bytes) = assembly.insert(part, payload) {
                    reconstructed = Some(bytes);
                }
            }
            assert_eq!(reconstructed, Some(jpg.clone()), "missing fragment {dropped}");
        }
    }

    #[test]
    fn parity_does_not_claim_to_recover_two_missing_packets() {
        let fragments = make_fragments(45, 17, &vec![7u8; 9_999]).unwrap();
        let total = fragments.len() - 1;
        let mut assembly = Assembly::new(17, total);
        for (i, fragment) in fragments.iter().enumerate() {
            if i < 2 { continue; }
            let (_, _, part, _, payload) = parse_fragment(fragment).unwrap();
            assert!(assembly.insert(part, payload).is_none());
        }
    }
    #[test]
    fn reject_oversized_bad_packets() {
        assert!(make_fragments(2,1,&vec![0;MAX_JPEG_BYTES+1]).is_none());
        assert!(parse_fragment(&[MAGIC,0]).is_none());
        let mut bytes = make_fragments(1, 1, &[3; 16]).unwrap().remove(0);
        bytes[14]=0;
        assert!(parse_fragment(&bytes).is_none());
    }
}
