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
        0 => (320, 180, 6, 42),  // Saver
        2 => (640, 360, 12, 46), // Sharp: more traffic, steps quality down if a frame is too big
        _ => (480, 270, 10, 50), // Balanced (default)
    }
}
const MAX_JPEG_BYTES: usize = 36_000;
const CHUNK: usize = 960;
const HEADER: usize = 23; // marker + sender u64 + camera session u64 + seq u32 + indexes
const MAX_CHUNKS: usize = 40;
const MAGIC: u8 = 0xF6; // v0.8, requires same version on all peers

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
    session_id: u64,
    seq: u32,
    parts: Vec<Option<Vec<u8>>>,
    /// One XOR parity fragment can reconstruct one missing UDP packet.
    parity: Option<(usize, Vec<u8>)>,
    recovered: bool,
    updated: Instant,
}
impl Assembly {
    fn new(session_id: u64, seq: u32, total: usize) -> Self {
        Self {
            session_id, seq,
            parts: vec![None; total],
            parity: None,
            recovered: false,
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
                self.recovered = true;
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
fn make_fragments(id: u64, session_id: u64, seq: u32, jpg: &[u8]) -> Option<Vec<Vec<u8>>> {
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
        datagram.extend_from_slice(&session_id.to_be_bytes());
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
        datagram.extend_from_slice(&session_id.to_be_bytes());
        datagram.extend_from_slice(&seq.to_be_bytes());
        datagram.push(total as u8); // reserved index identifies parity
        datagram.push(total as u8);
        datagram.extend_from_slice(&(jpg.len() as u32).to_be_bytes());
        datagram.extend_from_slice(&parity);
        result.push(datagram);
    }
    Some(result)
}
fn parse_fragment(datagram: &[u8]) -> Option<(u64, u64, u32, usize, usize, &[u8])> {
    if datagram.len() <= HEADER || datagram.len() > HEADER + CHUNK + 4 || datagram[0] != MAGIC { return None; }
    let sender = u64::from_be_bytes(datagram[1..9].try_into().ok()?);
    let session_id = u64::from_be_bytes(datagram[9..17].try_into().ok()?);
    let seq = u32::from_be_bytes(datagram[17..21].try_into().ok()?);
    let part = datagram[21] as usize;
    let total = datagram[22] as usize;
    if total == 0 || total > MAX_CHUNKS || part > total { return None; }
    let payload = &datagram[HEADER..];
    if part == total {
        if total < 2 || payload.len() != CHUNK + 4 { return None; }
    } else if payload.is_empty() || payload.len() > CHUNK { return None; }
    Some((sender, session_id, seq, part, total, payload))
}
fn decode_jpeg(id: u64, data: &[u8]) -> Option<VideoFrame> {
    if data.len() > MAX_JPEG_BYTES || data.is_empty() { return None; }
    let image = image::load_from_memory_with_format(data, ImageFormat::Jpeg).ok()?.to_rgb8();
    if !matches!((image.width(), image.height()), (320, 180) | (480, 270) | (640, 360)) { return None; }
    Some(VideoFrame {
        sender_id: id, width: image.width() as usize, height: image.height() as usize,
        rgb: image.into_raw(),
    })
}
/// Names of the cameras Windows reports, as `(index, name)`.
pub fn list_cameras() -> Vec<(u32, String)> {
    nokhwa::query(nokhwa::utils::ApiBackend::Auto)
        .map(|found| {
            found.into_iter()
                .filter_map(|info| Some((info.index().as_index().ok()?, info.human_name())))
                .collect()
        })
        .unwrap_or_default()
}

/// Cameras differ a lot: many laptop webcams offer MJPEG only at 720p, others only
/// raw YUY2/NV12. Try the formats we want most first and fall back gracefully
/// instead of failing with "format not found".
fn open_camera(index: u32) -> Result<Camera, String> {
    let attempts = [
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(1280, 720), FrameFormat::MJPEG, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(640, 480), FrameFormat::MJPEG, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(1280, 720), FrameFormat::YUYV, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(640, 480), FrameFormat::YUYV, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(640, 480), FrameFormat::NV12, 30)),
        RequestedFormatType::AbsoluteHighestFrameRate,
    ];
    let mut last_error = String::from("камера не підтримує жодного відомого формату");
    for attempt in attempts {
        let requested = RequestedFormat::new::<RgbFormat>(attempt);
        match Camera::new(CameraIndex::Index(index), requested) {
            Ok(mut camera) => match camera.open_stream() {
                Ok(()) => return Ok(camera),
                Err(error) => last_error = error.to_string(),
            },
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(last_error)
}

/// Cut the largest centred 16:9 region out of a frame. Webcams commonly deliver
/// 4:3; squeezing that into 16:9 makes everybody look flattened.
fn crop_to_16x9(image: &RgbImage) -> RgbImage {
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 { return image.clone(); }
    let (crop_w, crop_h) = if w * 9 > h * 16 { (h * 16 / 9, h) } else { (w, w * 9 / 16) };
    if (crop_w, crop_h) == (w, h) { return image.clone(); }
    image::imageops::crop_imm(image, (w - crop_w) / 2, (h - crop_h) / 2, crop_w, crop_h).to_image()
}

fn webcam_loop(
    active: Arc<AtomicBool>,
    live: Arc<AtomicBool>,
    id: u64,
    targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    events: SyncSender<VideoEvent>,
    frames_sent: Arc<AtomicU64>,
    camera_index: u32,
    quality: Arc<AtomicU32>,
) {
    if !active.load(Ordering::Relaxed) { return; }
    let mut camera = match open_camera(camera_index) {
        Ok(camera) => camera,
        Err(error) => {
            let _ = events.try_send(VideoEvent::Error(format!(
                "Камера не запустилася: {error}. Перевір, що в Параметрах Windows (Конфіденційність → Камера) \
                 дозволено доступ для класичних програм і що камеру не займає інша програма."
            )));
            return;
        }
    };
    let socket = match UdpSocket::bind(("0.0.0.0", 0)) {
        Ok(socket) => socket,
        Err(error) => {
            let _ = events.try_send(VideoEvent::Error(format!("Порт передачі камери: {error}")));
            let _ = camera.stop_stream();
            return;
        }
    };
    let mut seq = 0u32;
    let video_session_id = rand::random::<u64>().max(1);
    let mut last_sent: Option<Vec<u8>> = None;
    let mut last_send_time = Instant::now() - Duration::from_secs(2);
    let mut last_processed = Instant::now() - Duration::from_secs(1);
    let mut failures = 0u32;
    while active.load(Ordering::Relaxed) {
        let (width, height, fps, jpeg_quality) = video_preset(quality.load(Ordering::Relaxed));
        // Always pull frames at the camera's own pace so the driver queue never
        // fills up (that would add seconds of delay); only *process* at `fps`.
        let frame = match camera.frame() {
            Ok(frame) => { failures = 0; frame }
            Err(error) => {
                failures += 1;
                if failures >= 8 {
                    let _ = events.try_send(VideoEvent::Error(format!("Камера перестала відповідати: {error}")));
                    break;
                }
                thread::sleep(Duration::from_millis(40));
                continue;
            }
        };
        if !active.load(Ordering::Relaxed) { break; }
        if last_processed.elapsed() < Duration::from_millis(1000 / fps) { continue; }
        last_processed = Instant::now();
        let decoded = match frame.decode_image::<RgbFormat>() {
            Ok(image) => image,
            Err(_) => continue, // a single corrupt frame is not worth a notification
        };
        // Crop to 16:9 first, then shrink *before* encoding.
        let cropped = crop_to_16x9(&decoded);
        let small: RgbImage = image::imageops::resize(&cropped, width, height, FilterType::CatmullRom);
        live.store(true, Ordering::Relaxed);
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
        let ips = targets.lock().map(|t| t.clone()).unwrap_or_default();
        if !ips.is_empty() && should_send {
            // A detailed frame may exceed the VPN-safe bound. Retry at lower quality.
            for jpeg_quality in [jpeg_quality, 40, 32, 26] {
                let mut compressed = Vec::new();
                let encoded = JpegEncoder::new_with_quality(&mut compressed, jpeg_quality)
                    .encode(small.as_raw(), width, height, ExtendedColorType::Rgb8);
                if encoded.is_err() || compressed.len() > MAX_JPEG_BYTES { continue; }
                if let Some(chunks) = make_fragments(id, video_session_id, seq, &compressed) {
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
    }
    live.store(false, Ordering::Relaxed);
    let _ = camera.stop_stream();
}
pub struct VideoSession {
    active: Arc<AtomicBool>,
    /// True once the camera has produced its first frame.
    live: Arc<AtomicBool>,
    recv_stop: Arc<AtomicBool>,
    webcam_worker: Option<JoinHandle<()>>,
    recv_worker: Option<JoinHandle<()>>,
    decode_worker: Option<JoinHandle<()>>,
    events_tx: SyncSender<VideoEvent>,
    events_rx: Receiver<VideoEvent>,
    local_id: u64,
    targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    camera_index: Arc<AtomicU32>,
    quality: Arc<AtomicU32>,
    pub frames_sent: Arc<AtomicU64>,
    pub frames_received: Arc<AtomicU64>,
    pub frames_recovered: Arc<AtomicU64>,
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
        let decode_stop = Arc::clone(&recv_stop);
        let tx = events_tx.clone();
        let (jpeg_tx, jpeg_rx) = mpsc::sync_channel::<(u64, Vec<u8>, bool)>(6);
        let valid_peers = Arc::clone(&peers);
        let frames_received = Arc::new(AtomicU64::new(0));
        let rx_count = Arc::clone(&frames_received);
        let frames_recovered = Arc::new(AtomicU64::new(0));
        let recovered_count = Arc::clone(&frames_recovered);
        // JPEG decoding is CPU-intensive. Keep it off the UDP read thread so
        // four webcams cannot block packet reception and cause extra frame loss.
        let decode_worker = thread::spawn(move || {
            while !decode_stop.load(Ordering::Relaxed) {
                match jpeg_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok((sender_id, jpeg, recovered)) => {
                        if let Some(frame) = decode_jpeg(sender_id, &jpeg) {
                            if tx.try_send(VideoEvent::Frame(frame)).is_ok() {
                                rx_count.fetch_add(1, Ordering::Relaxed);
                                if recovered { recovered_count.fetch_add(1, Ordering::Relaxed); }
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        let worker = thread::spawn(move || {
            let mut packets = [0u8; 1500];
            let mut partial: HashMap<IpAddr, Assembly> = HashMap::new();
            let mut completed: HashMap<IpAddr, (u64, u64, u32, Instant)> = HashMap::new();
            while !stop.load(Ordering::Relaxed) {
                match rx_socket.recv_from(&mut packets) {
                    Ok((size, source)) => {
                        let IpAddr::V4(ip) = source.ip() else { continue; };
                        if !valid_peers.lock().map(|p| p.contains(&ip)).unwrap_or(false) { continue; }
                        let Some((id, session_id, seq, part, total, payload)) =
                            parse_fragment(&packets[..size]) else { continue; };
                        // The camera session changes on re-enable; a stream restart
                        // is accepted immediately rather than waiting for a two-second expiry.
                        if let Some((previous_id, previous_session, previous_seq, time)) = completed.get(&source.ip()) {
                            if *previous_id == id && *previous_session == session_id
                                && time.elapsed() < Duration::from_secs(2)
                                && !seq_newer(seq, *previous_seq) { continue; }
                        }
                        let entry = partial.entry(source.ip())
                            .or_insert_with(|| Assembly::new(session_id, seq, total));
                        if entry.session_id != session_id || entry.seq != seq || entry.parts.len() != total {
                            if entry.session_id == session_id
                                && entry.updated.elapsed() < Duration::from_millis(700)
                                && !seq_newer(seq, entry.seq) { continue; }
                            *entry = Assembly::new(session_id, seq, total);
                        }
                        if let Some(jpeg) = entry.insert(part, payload) {
                            let was_recovered = entry.recovered;
                            partial.remove(&source.ip());
                            completed.insert(source.ip(), (id, session_id, seq, Instant::now()));
                            // Bounded queue: drop excess video frames under overload,
                            // never pause voice or fill the system UDP receive buffer.
                            let _ = jpeg_tx.try_send((id, jpeg, was_recovered));
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
                completed.retain(|_, (_,_,_,time)| time.elapsed() < Duration::from_secs(2));
            }
        });
        Ok(Self {
            active: Arc::new(AtomicBool::new(false)),
            live: Arc::new(AtomicBool::new(false)),
            recv_stop,
            webcam_worker: None,
            recv_worker: Some(worker),
            decode_worker: Some(decode_worker),
            events_tx, events_rx, local_id: id, targets: peers,
            camera_index, quality,
            frames_sent: Arc::new(AtomicU64::new(0)),
            frames_received,
            frames_recovered,
        })
    }
    pub fn enabled(&self) -> bool { self.active.load(Ordering::Relaxed) }
    /// Camera was switched on but has not delivered a frame yet.
    pub fn starting(&self) -> bool { self.enabled() && !self.live.load(Ordering::Relaxed) }
    pub fn enable(&mut self) {
        if self.enabled() { return; }
        self.active = Arc::new(AtomicBool::new(true));
        self.live = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&self.active);
        let live = Arc::clone(&self.live);
        let tx = self.events_tx.clone();
        let id = self.local_id;
        let peers = Arc::clone(&self.targets);
        let count = Arc::clone(&self.frames_sent);
        let camera_index = self.camera_index.load(Ordering::Relaxed);
        let quality = Arc::clone(&self.quality);
        // Opening a camera can take seconds. A previous worker may still be
        // releasing the device, so the new one waits for it on its own thread
        // instead of blocking the UI.
        let previous = self.webcam_worker.take();
        self.webcam_worker = Some(thread::spawn(move || {
            if let Some(previous) = previous { let _ = previous.join(); }
            webcam_loop(Arc::clone(&stop), live, id, peers, tx, count, camera_index, quality);
            stop.store(false, Ordering::Relaxed);
        }));
    }
    /// Asks the camera worker to stop; never blocks the caller.
    pub fn disable(&mut self) {
        self.active.store(false, Ordering::Relaxed);
        self.live.store(false, Ordering::Relaxed);
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
        if let Some(worker) = self.webcam_worker.take() { let _ = worker.join(); }
        self.recv_stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.recv_worker.take() { let _ = worker.join(); }
        if let Some(worker) = self.decode_worker.take() { let _ = worker.join(); }
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
        assert_eq!(video_preset(0), (320, 180, 6, 42));
        assert_eq!(video_preset(2), (640, 360, 12, 46));
        assert_eq!(video_preset(99), (480, 270, 10, 50));
    }
    #[test]
    fn every_preset_resolution_is_accepted_by_the_receiver() {
        for mode in 0..3 {
            let (w, h, _, q) = video_preset(mode);
            let image = RgbImage::from_fn(w, h, |x, y| image::Rgb([x as u8, y as u8, 90]));
            let mut jpeg = Vec::new();
            JpegEncoder::new_with_quality(&mut jpeg, q)
                .encode(image.as_raw(), w, h, ExtendedColorType::Rgb8).unwrap();
            let frame = decode_jpeg(1, &jpeg).unwrap_or_else(|| panic!("preset {mode} rejected"));
            assert_eq!((frame.width as u32, frame.height as u32), (w, h));
        }
    }
    #[test]
    fn crop_keeps_aspect_instead_of_squeezing() {
        let four_three = RgbImage::new(640, 480);
        let cropped = crop_to_16x9(&four_three);
        assert_eq!((cropped.width(), cropped.height()), (640, 360));
        let ultrawide = RgbImage::new(1000, 300);
        let cropped = crop_to_16x9(&ultrawide);
        assert_eq!((cropped.width(), cropped.height()), (533, 300));
        let already = RgbImage::new(1280, 720);
        assert_eq!(crop_to_16x9(&already).dimensions(), (1280, 720));
    }
    #[test]
    fn crop_takes_the_centre_of_the_frame() {
        let mut image = RgbImage::new(160, 120);
        for y in 0..120 { for x in 0..160 {
            image.put_pixel(x, y, image::Rgb([0, y as u8, 0]));
        }}
        let cropped = crop_to_16x9(&image); // 160x90 starting at y = 15
        assert_eq!(cropped.get_pixel(0, 0)[1], 15);
    }
    #[test]
    fn fragments_roundtrip_and_out_of_order() {
        let jpg = (0..10_987).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let parts = make_fragments(7654, 81, 99, &jpg).unwrap();
        assert!(parts.len()>2);
        let total = parts.len() - 1;
        let mut assembly = Assembly::new(81, 99, total);
        let mut result = None;
        for part in parts.iter().rev() {
            let (id,session,seq,idx,count,payload) = parse_fragment(part).unwrap();
            assert_eq!((id,session,seq,count),(7654,81,99,total));
            if let Some(done) = assembly.insert(idx,payload) { result = Some(done); }
        }
        assert_eq!(result.unwrap(),jpg);
    }

    #[test]
    fn parity_recovers_one_lost_fragment_any_position() {
        let jpg = (0..17_777).map(|i| ((i * 17) % 251) as u8).collect::<Vec<_>>();
        let fragments = make_fragments(45, 81, 17, &jpg).unwrap();
        let total = fragments.len() - 1;
        for dropped in [0, total / 2, total - 1] {
            let mut assembly = Assembly::new(81, 17, total);
            let mut reconstructed = None;
            for (i, fragment) in fragments.iter().enumerate().rev() {
                if i == dropped { continue; }
                let (_, _, _, part, _, payload) = parse_fragment(fragment).unwrap();
                if let Some(bytes) = assembly.insert(part, payload) {
                    reconstructed = Some(bytes);
                }
            }
            assert!(assembly.recovered, "parity not used for fragment {dropped}");
            assert_eq!(reconstructed, Some(jpg.clone()), "missing fragment {dropped}");
        }
    }

    #[test]
    fn parity_does_not_claim_to_recover_two_missing_packets() {
        let fragments = make_fragments(45, 81, 17, &vec![7u8; 9_999]).unwrap();
        let total = fragments.len() - 1;
        let mut assembly = Assembly::new(81, 17, total);
        for (i, fragment) in fragments.iter().enumerate() {
            if i < 2 { continue; }
            let (_, _, _, part, _, payload) = parse_fragment(fragment).unwrap();
            assert!(assembly.insert(part, payload).is_none());
        }
    }
    #[test]
    fn camera_sessions_are_distinct_even_when_sequences_restart() {
        let first = make_fragments(9, 100, 0, &[1; 1000]).unwrap();
        let next = make_fragments(9, 200, 0, &[1; 1000]).unwrap();
        let (_, first_session, first_seq, _, _, _) = parse_fragment(&first[0]).unwrap();
        let (_, next_session, next_seq, _, _, _) = parse_fragment(&next[0]).unwrap();
        assert_ne!(first_session, next_session);
        assert_eq!(first_seq, next_seq);
    }
    #[test]
    fn jpeg_real_codec_recovers_frame_after_packet_loss() {
        let source = RgbImage::from_fn(320, 180, |x,y| {
            image::Rgb([x as u8,y as u8, 120])
        });
        let mut compressed = Vec::new();
        JpegEncoder::new_with_quality(&mut compressed, 46).encode(
            source.as_raw(), 320, 180, ExtendedColorType::Rgb8
        ).unwrap();
        assert!(compressed.len() < MAX_JPEG_BYTES);
        let pieces = make_fragments(77, 8, 66, &compressed).unwrap();
        let total = pieces.len()-1;
        assert!(total > 2);
        let mut assembler = Assembly::new(8, 66, total);
        let mut jpg = None;
        for (i, piece) in pieces.iter().enumerate().rev() {
            if i == 1 { continue; } // simulate one lost fragment
            let (_, _, _, part, _, payload) = parse_fragment(piece).unwrap();
            if let Some(frame) = assembler.insert(part, payload) { jpg = Some(frame); }
        }
        assert!(assembler.recovered);
        let frame = decode_jpeg(77, &jpg.unwrap()).unwrap();
        assert_eq!((frame.sender_id,frame.width,frame.height),(77,320,180));
        assert_eq!(frame.rgb.len(),320*180*3);
    }

    #[test]
    fn reject_oversized_bad_packets() {
        assert!(make_fragments(2,81,1,&vec![0;MAX_JPEG_BYTES+1]).is_none());
        assert!(parse_fragment(&[MAGIC,0]).is_none());
        let mut bytes = make_fragments(1, 81, 1, &[3; 16]).unwrap().remove(0);
        bytes[22]=0;
        assert!(parse_fragment(&bytes).is_none());
    }
}
