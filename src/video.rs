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
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const VIDEO_WIDTH: u32 = 320;
const VIDEO_HEIGHT: u32 = 180;
const VIDEO_FPS: u64 = 6;
const JPEG_QUALITY: u8 = 46;
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
    updated: Instant,
}
impl Assembly {
    fn new(seq: u32, total: usize) -> Self {
        Self {
            seq,
            parts: vec![None; total],
            updated: Instant::now(),
        }
    }
    fn insert(&mut self, part: usize, bytes: &[u8]) -> Option<Vec<u8>> {
        if part >= self.parts.len() || bytes.len() > CHUNK { return None; }
        self.parts[part].get_or_insert_with(|| bytes.to_vec());
        self.updated = Instant::now();
        if self.parts.iter().all(Option::is_some) {
            let mut result = Vec::with_capacity(self.parts.len() * CHUNK);
            for p in self.parts.iter_mut() {
                result.extend_from_slice(p.take().as_ref()?);
            }
            (result.len() <= MAX_JPEG_BYTES).then_some(result)
        } else {
            None
        }
    }
}
fn make_fragments(id: u64, seq: u32, jpg: &[u8]) -> Option<Vec<Vec<u8>>> {
    if jpg.is_empty() || jpg.len() > MAX_JPEG_BYTES { return None; }
    let total = jpg.len().div_ceil(CHUNK);
    if total > MAX_CHUNKS { return None; }
    let mut result = Vec::with_capacity(total);
    for (part, bytes) in jpg.chunks(CHUNK).enumerate() {
        let mut datagram = Vec::with_capacity(HEADER + bytes.len());
        datagram.push(MAGIC);
        datagram.extend_from_slice(&id.to_be_bytes());
        datagram.extend_from_slice(&seq.to_be_bytes());
        datagram.push(part as u8);
        datagram.push(total as u8);
        datagram.extend_from_slice(bytes);
        result.push(datagram);
    }
    Some(result)
}
fn parse_fragment(datagram: &[u8]) -> Option<(u64, u32, usize, usize, &[u8])> {
    if datagram.len() <= HEADER || datagram.len() > HEADER + CHUNK || datagram[0] != MAGIC { return None; }
    let sender = u64::from_be_bytes(datagram[1..9].try_into().ok()?);
    let seq = u32::from_be_bytes(datagram[9..13].try_into().ok()?);
    let part = datagram[13] as usize;
    let total = datagram[14] as usize;
    if total == 0 || total > MAX_CHUNKS || part >= total { return None; }
    Some((sender, seq, part, total, &datagram[HEADER..]))
}
fn decode_jpeg(id: u64, data: &[u8]) -> Option<VideoFrame> {
    if data.len() > MAX_JPEG_BYTES || data.is_empty() { return None; }
    let image = image::load_from_memory_with_format(data, ImageFormat::Jpeg).ok()?.to_rgb8();
    if image.width() != VIDEO_WIDTH || image.height() != VIDEO_HEIGHT { return None; }
    Some(VideoFrame { sender_id: id, width: VIDEO_WIDTH as usize, height: VIDEO_HEIGHT as usize, rgb: image.into_raw() })
}
fn webcam_loop(
    active: Arc<AtomicBool>,
    id: u64,
    targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    events: SyncSender<VideoEvent>,
    frames_sent: Arc<AtomicU64>,
) {
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(640, 480), FrameFormat::MJPEG, 15)
    ));
    let mut camera = match Camera::new(CameraIndex::index(0), requested) {
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
    let interval = Duration::from_millis(1000 / VIDEO_FPS);
    while active.load(Ordering::Relaxed) {
        let start = Instant::now();
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
        let small: RgbImage = image::imageops::resize(&decoded, VIDEO_WIDTH, VIDEO_HEIGHT, FilterType::Triangle);
        let _ = events.try_send(VideoEvent::Frame(VideoFrame {
            sender_id: id, width: VIDEO_WIDTH as usize, height: VIDEO_HEIGHT as usize,
            rgb: small.as_raw().clone(),
        }));
        let ips = targets.lock().map(|t|t.clone()).unwrap_or_default();
        if !ips.is_empty() {
            let mut compressed = Vec::new();
            let encoded = JpegEncoder::new_with_quality(&mut compressed, JPEG_QUALITY)
                .encode(small.as_raw(), VIDEO_WIDTH, VIDEO_HEIGHT, ExtendedColorType::Rgb8);
            if encoded.is_ok() {
                if let Some(chunks) = make_fragments(id, seq, &compressed) {
                    for ip in ips {
                        let addr = SocketAddr::V4(SocketAddrV4::new(ip, VIDEO_PORT));
                        for part in &chunks {
                            let _ = socket.send_to(part, addr);
                        }
                    }
                    frames_sent.fetch_add(1, Ordering::Relaxed);
                    seq = seq.wrapping_add(1);
                }
            }
        }
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
    pub frames_sent: Arc<AtomicU64>,
    pub frames_received: Arc<AtomicU64>,
}
impl VideoSession {
    pub fn start(id: u64, peers: Arc<Mutex<Vec<Ipv4Addr>>>) -> Result<Self, String> {
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
            while !stop.load(Ordering::Relaxed) {
                match rx_socket.recv_from(&mut packets) {
                    Ok((size, source)) => {
                        let IpAddr::V4(ip) = source.ip() else { continue; };
                        if !valid_peers.lock().map(|p| p.contains(&ip)).unwrap_or(false) { continue; }
                        let Some((id, seq, part, total, payload)) = parse_fragment(&packets[..size]) else { continue; };
                        let entry = partial.entry(source.ip())
                            .or_insert_with(|| Assembly::new(seq, total));
                        if entry.seq != seq || entry.parts.len() != total {
                            *entry = Assembly::new(seq, total);
                        }
                        if let Some(jpeg) = entry.insert(part, payload) {
                            partial.remove(&source.ip());
                            if let Some(frame) = decode_jpeg(id, &jpeg) {
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
            }
        });
        Ok(Self {
            active: Arc::new(AtomicBool::new(false)),
            recv_stop,
            webcam_worker: None,
            recv_worker: Some(worker),
            events_tx, events_rx, local_id: id, targets: peers,
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
        // Stop condition is logically inverted; thread runs while active=true.
        self.webcam_worker = Some(thread::spawn(move || {
            webcam_loop(Arc::clone(&stop), id, peers, tx, count);
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
    fn fragments_roundtrip_and_out_of_order() {
        let jpg = vec![34u8; 10_987];
        let parts = make_fragments(7654, 99, &jpg).unwrap();
        assert!(parts.len()>1);
        let mut assembly = Assembly::new(99, parts.len());
        let mut result = None;
        for part in parts.iter().rev() {
            let (id,seq,idx,total,payload) = parse_fragment(part).unwrap();
            assert_eq!((id,seq,total),(7654,99,parts.len()));
            if let Some(done) = assembly.insert(idx,payload) { result = Some(done); }
        }
        assert_eq!(result.unwrap(),jpg);
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
