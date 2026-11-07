//! Lightweight native Windows webcam video for small, trusted Radmin VPN rooms.
//! Camera is ON ONLY when explicitly enabled by the user. JPEG uses low resolution
//! and a strict bandwidth cap; this is a beta transport, not a low-latency H.264 stack.
use crate::i18n::{tr, trf};
use crate::protocol::VIDEO_PORT;
use crate::stream::{fit_size, I420, Scaler, StreamEncoder, StreamHub, StreamKind};
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, ExtendedColorType, ImageFormat, RgbImage};
use nokhwa::{
    pixel_format::RgbFormat,
    Buffer,
    utils::{CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution},
    Camera,
};
use std::{
    borrow::Cow,
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

// Three practical presets: stability first, then smoothness, then detail.
fn video_preset(mode: u32) -> (u32, u32, u64, u8) {
    match mode {
        0 => (320, 180, 6, 42),  // Saver
        2 => (640, 360, 12, 48), // Sharp
        3 => (960, 540, 15, 56), // HD: ~6 Mbit/s per friend, needs a good upload
        _ => (480, 270, 10, 50), // Balanced (default)
    }
}
/// H.264 camera presets (modes 4 and 5): `(width, height, fps, bitrate in bit/s)`.
fn h264_preset(mode: u32) -> Option<(usize, usize, u32, u32)> {
    match mode {
        4 => Some((1280, 720, 30, 3_000_000)),
        5 => Some((1920, 1080, 30, 6_000_000)),
        _ => None,
    }
}
/// Resolution requested from the camera for a quality mode.
fn capture_size(mode: u32) -> (u32, u32) { if mode == 5 { (1920, 1080) } else { (1280, 720) } }
const MAX_JPEG_BYTES: usize = 72_000;
const CHUNK: usize = 960;
const HEADER: usize = 23; // marker + sender u64 + camera session u64 + seq u32 + indexes
const MAX_CHUNKS: usize = 80;
const MAGIC: u8 = 0xF6; // v0.8, requires same version on all peers

#[derive(Debug)]
pub struct VideoFrame {
    pub sender_id: u64,
    pub kind: StreamKind,
    pub width: usize,
    pub height: usize,
    /// Opaque RGBA pixels, ready to become a texture.
    pub rgba: Vec<u8>,
}

fn rgb_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for pixel in rgb.chunks_exact(3) {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
    rgba
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
/// `(sender, camera session, sequence, part, total parts, payload)`
type Fragment<'a> = (u64, u64, u32, usize, usize, &'a [u8]);

fn parse_fragment(datagram: &[u8]) -> Option<Fragment<'_>> {
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
    if !matches!((image.width(), image.height()), (320, 180) | (480, 270) | (640, 360) | (960, 540)) { return None; }
    Some(VideoFrame {
        sender_id: id, kind: StreamKind::Camera,
        width: image.width() as usize, height: image.height() as usize,
        rgba: rgb_to_rgba(image.as_raw()),
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

/// Walks the JPEG header (up to the start-of-scan marker) and returns
/// `(offset_of_first_SOS, concatenated DHT segments, has_dht)`.
fn scan_jpeg_header(data: &[u8]) -> Option<(usize, Vec<u8>, bool)> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 { return None; }
    let mut i = 2;
    let mut dht = Vec::new();
    while i + 4 <= data.len() {
        if data[i] != 0xFF { return None; }
        let marker = data[i + 1];
        if marker == 0xFF { i += 1; continue; } // fill byte
        if marker == 0xDA { return Some((i, dht.clone(), !dht.is_empty())); }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) { i += 2; continue; }
        let length = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if length < 2 || i + 2 + length > data.len() { return None; }
        if marker == 0xC4 { dht.extend_from_slice(&data[i..i + 2 + length]); }
        i += 2 + length;
    }
    None
}

/// The standard JPEG Huffman tables. Taken from our own encoder's output so
/// they are exactly the Annex K tables, not typed in by hand.
fn standard_huffman_tables() -> &'static [u8] {
    static TABLES: OnceLock<Vec<u8>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let image = RgbImage::from_fn(16, 16, |x, y| image::Rgb([(x * 15) as u8, (y * 15) as u8, 128]));
        let mut jpeg = Vec::new();
        let _ = JpegEncoder::new_with_quality(&mut jpeg, 80)
            .encode(image.as_raw(), 16, 16, ExtendedColorType::Rgb8);
        scan_jpeg_header(&jpeg).map(|(_, dht, _)| dht).unwrap_or_default()
    })
}

/// Many webcams send "MJPEG" frames without Huffman tables (the receiver is
/// expected to know the standard ones). Our JPEG decoder is strict, so add them.
fn with_standard_tables(jpeg: &[u8]) -> Cow<'_, [u8]> {
    match scan_jpeg_header(jpeg) {
        Some((sos, _, false)) => {
            let tables = standard_huffman_tables();
            if tables.is_empty() { return Cow::Borrowed(jpeg); }
            let mut fixed = Vec::with_capacity(jpeg.len() + tables.len());
            fixed.extend_from_slice(&jpeg[..sos]);
            fixed.extend_from_slice(tables);
            fixed.extend_from_slice(&jpeg[sos..]);
            Cow::Owned(fixed)
        }
        _ => Cow::Borrowed(jpeg),
    }
}

/// Turns a camera buffer into RGB. nokhwa is built without its optional MJPEG
/// decoder (it would need a C toolchain), so MJPEG goes through the `image` crate.
/// Raw formats (YUY2, NV12) are converted by nokhwa itself.
fn decode_camera_frame(frame: &Buffer) -> Result<RgbImage, String> {
    match frame.source_frame_format() {
        FrameFormat::MJPEG => {
            let data = with_standard_tables(frame.buffer());
            image::load_from_memory_with_format(&data, ImageFormat::Jpeg)
                .map(|decoded| decoded.to_rgb8())
                .map_err(|error| error.to_string())
        }
        _ => frame.decode_image::<RgbFormat>().map_err(|error| error.to_string()),
    }
}

/// Cameras differ a lot: many laptop webcams offer MJPEG only at 720p, others only
/// raw YUY2/NV12. Try the formats we want most first and fall back gracefully
/// instead of failing with "format not found".
fn open_camera(index: u32, wanted: (u32, u32)) -> Result<Camera, String> {
    let attempts = [
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(wanted.0, wanted.1), FrameFormat::MJPEG, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(1280, 720), FrameFormat::MJPEG, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(640, 480), FrameFormat::MJPEG, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(1280, 720), FrameFormat::YUYV, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(640, 480), FrameFormat::YUYV, 30)),
        RequestedFormatType::Closest(CameraFormat::new(Resolution::new(640, 480), FrameFormat::NV12, 30)),
        RequestedFormatType::AbsoluteHighestFrameRate,
    ];
    let mut last_error = String::from(tr("the camera supports none of the known formats"));
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

#[allow(clippy::too_many_arguments)]
fn webcam_loop(
    active: Arc<AtomicBool>,
    live: Arc<AtomicBool>,
    id: u64,
    targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    events: SyncSender<VideoEvent>,
    frames_sent: Arc<AtomicU64>,
    camera_index: u32,
    quality: Arc<AtomicU32>,
    diagnostics: Arc<CameraDiagnostics>,
    hub: Arc<StreamHub>,
) {
    if !active.load(Ordering::Relaxed) { return; }
    let mut camera = match open_camera(camera_index, capture_size(quality.load(Ordering::Relaxed))) {
        Ok(camera) => camera,
        Err(error) => {
            let _ = events.try_send(VideoEvent::Error(trf("The camera did not start: {error}. Check that Windows settings (Privacy → Camera) allow desktop apps to use the camera and that no other app is using it.", &[("error", &error)])));
            return;
        }
    };
    let format = camera.camera_format();
    if let Ok(mut info) = diagnostics.format.lock() {
        *info = trf("{format} {width}×{height}, {fps} fps", &[("format", &format.format()),
            ("width", &format.resolution().width()), ("height", &format.resolution().height()), ("fps", &format.frame_rate())]);
    }
    let socket = match UdpSocket::bind(("0.0.0.0", 0)) {
        Ok(socket) => socket,
        Err(error) => {
            let _ = events.try_send(VideoEvent::Error(trf("Camera send port: {error}", &[("error", &error)])));
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
    let mut decode_failures = 0u32;
    // H.264 state (modes 4 and 5).
    let mut h264: Option<(u32, StreamEncoder)> = None;
    let mut picture: Option<I420> = None;
    let mut scaler = Scaler::default();
    let mut scaled: Vec<u8> = Vec::new();
    let mut preview: Vec<u8> = Vec::new();
    let stream_session = rand::random::<u64>().max(1);
    let mut stream_seq = 0u32;
    let mut was_sending = false;
    let mut preview_tick = 0u32;
    while active.load(Ordering::Relaxed) {
        let mode = quality.load(Ordering::Relaxed);
        let (width, height, jpeg_fps, jpeg_quality) = video_preset(mode);
        let fps = h264_preset(mode).map_or(jpeg_fps, |preset| preset.2 as u64);
        // Always pull frames at the camera's own pace so the driver queue never
        // fills up (that would add seconds of delay); only *process* at `fps`.
        let frame = match camera.frame() {
            Ok(frame) => { failures = 0; frame }
            Err(error) => {
                failures += 1;
                if failures >= 8 {
                    let _ = events.try_send(VideoEvent::Error(trf("The camera stopped responding: {error}", &[("error", &error)])));
                    break;
                }
                thread::sleep(Duration::from_millis(40));
                continue;
            }
        };
        if !active.load(Ordering::Relaxed) { break; }
        diagnostics.captured.fetch_add(1, Ordering::Relaxed);
        if last_processed.elapsed() < Duration::from_millis(1000 / fps) { continue; }
        last_processed = Instant::now();
        let decoded = match decode_camera_frame(&frame) {
            Ok(image) => { decode_failures = 0; image }
            Err(error) => {
                diagnostics.decode_errors.fetch_add(1, Ordering::Relaxed);
                decode_failures += 1;
                // One corrupt frame is normal. A long streak means this camera
                // format cannot be decoded: tell the user instead of showing nothing.
                if decode_failures == 20 {
                    let _ = events.try_send(VideoEvent::Error(trf("Camera frames ({format}) cannot be decoded: {error}. Try another camera or quality.",
                        &[("format", &frame.source_frame_format()), ("error", &error)])));
                }
                continue;
            }
        };
        if let Some((target_w, target_h, target_fps, bitrate)) = h264_preset(mode) {
            if h264.as_ref().map(|state| state.0) != Some(mode) {
                match StreamEncoder::new(StreamKind::Camera, target_fps, bitrate) {
                    Ok(encoder) => h264 = Some((mode, encoder)),
                    Err(error) => {
                        let _ = events.try_send(VideoEvent::Error(trf("Camera: {error}", &[("error", &error)])));
                        break;
                    }
                }
            }
            let (source_w, source_h) = (decoded.width() as usize, decoded.height() as usize);
            let (crop_w, crop_h) = if source_w * 9 > source_h * 16 { (source_h * 16 / 9, source_h) } else { (source_w, source_w * 9 / 16) };
            let crop = (((source_w - crop_w) / 2) as f64, ((source_h - crop_h) / 2) as f64, crop_w as f64, crop_h as f64);
            let (out_w, out_h) = fit_size(crop_w, crop_h, target_w, target_h);
            let mut raw = decoded.into_raw();
            if scaler.scale(&mut raw, (source_w, source_h), 3, Some(crop), (out_w, out_h), &mut scaled).is_err() { continue; }
            live.store(true, Ordering::Relaxed);

            // Local preview (every other frame is plenty for your own tile).
            preview_tick = preview_tick.wrapping_add(1);
            if preview_tick.is_multiple_of(2) {
                let (pw, ph) = fit_size(out_w, out_h, 640, 360);
                let rgba = if (pw, ph) == (out_w, out_h) { rgb_to_rgba(&scaled) } else {
                    let mut copy = scaled.clone();
                    if scaler.scale(&mut copy, (out_w, out_h), 3, None, (pw, ph), &mut preview).is_ok() { rgb_to_rgba(&preview) } else { Vec::new() }
                };
                if !rgba.is_empty() {
                    let _ = events.try_send(VideoEvent::Frame(VideoFrame {
                        sender_id: id, kind: StreamKind::Camera, width: pw, height: ph, rgba,
                    }));
                }
            }

            // Nobody to send to: skip the expensive encoding entirely.
            if hub.viewers() == 0 {
                was_sending = false;
                let _ = hub.take_key_request(StreamKind::Camera);
                continue;
            }
            if picture.as_ref().is_none_or(|p| p.width != out_w || p.height != out_h) {
                picture = Some(I420::new(out_w, out_h));
            }
            let Some(yuv) = picture.as_mut() else { continue; };
            yuv.fill_from_packed(&scaled, out_w * 3, 3, false);
            let force_key = hub.take_key_request(StreamKind::Camera) || !was_sending;
            was_sending = true;
            let Some((_, encoder)) = h264.as_mut() else { continue; };
            match encoder.encode(yuv, force_key) {
                Ok(Some(frame)) => {
                    hub.send(StreamKind::Camera, stream_session, stream_seq, frame.key, &frame.data);
                    stream_seq = stream_seq.wrapping_add(1);
                    frames_sent.fetch_add(1, Ordering::Relaxed);
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = events.try_send(VideoEvent::Error(trf("Camera: {error}", &[("error", &error)])));
                    break;
                }
            }
            continue;
        }
        // Crop to 16:9 first, then shrink *before* encoding.
        let cropped = crop_to_16x9(&decoded);
        let small: RgbImage = image::imageops::resize(&cropped, width, height, FilterType::CatmullRom);
        live.store(true, Ordering::Relaxed);
        // Keep the preview responsive, but reduce traffic when almost nothing moves.
        let _ = events.try_send(VideoEvent::Frame(VideoFrame {
            sender_id: id, kind: StreamKind::Camera,
            width: width as usize, height: height as usize,
            rgba: rgb_to_rgba(small.as_raw()),
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
            for jpeg_quality in [jpeg_quality, 46, 38, 32, 26] {
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
/// What the camera thread is doing; shown in Settings so a silent camera can be diagnosed.
#[derive(Default)]
pub struct CameraDiagnostics {
    pub format: Mutex<String>,
    pub captured: AtomicU64,
    pub decode_errors: AtomicU64,
}

pub struct VideoSession {
    active: Arc<AtomicBool>,
    /// True once the camera has produced its first frame.
    live: Arc<AtomicBool>,
    enabled_at: Option<Instant>,
    pub diagnostics: Arc<CameraDiagnostics>,
    hub: Arc<StreamHub>,
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
            .map_err(|e| trf("Could not open the camera UDP port {port}: {error}", &[("port", &VIDEO_PORT), ("error", &e)]))?;
        rx_socket.set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|e| e.to_string())?;
        let (events_tx, events_rx) = mpsc::sync_channel(12);
        let hub = Arc::new(StreamHub::start(id, Arc::clone(&peers), events_tx.clone())?);
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
            enabled_at: None,
            diagnostics: Arc::new(CameraDiagnostics::default()),
            hub,
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
    /// Channel for pictures that should appear in the UI (used by screen sharing).
    pub fn events_sender(&self) -> SyncSender<VideoEvent> { self.events_tx.clone() }
    pub fn local_id(&self) -> u64 { self.local_id }
    /// The H.264 network hub, shared with screen sharing.
    pub fn hub(&self) -> Arc<StreamHub> { Arc::clone(&self.hub) }
    /// H.264 frames received from friends (camera and screens).
    pub fn stream_frames_received(&self) -> u64 { self.hub.frames_received.load(Ordering::Relaxed) }
    pub fn enabled(&self) -> bool { self.active.load(Ordering::Relaxed) }
    /// Camera was switched on but has not delivered a frame yet.
    pub fn starting(&self) -> bool { self.enabled() && !self.live.load(Ordering::Relaxed) }
    /// Switched on for several seconds but no usable frame has arrived.
    pub fn stalled(&self) -> bool {
        self.starting() && self.enabled_at.is_some_and(|t| t.elapsed() > Duration::from_secs(6))
    }
    pub fn enable(&mut self) {
        if self.enabled() { return; }
        self.active = Arc::new(AtomicBool::new(true));
        self.live = Arc::new(AtomicBool::new(false));
        self.enabled_at = Some(Instant::now());
        self.diagnostics = Arc::new(CameraDiagnostics::default());
        let diagnostics = Arc::clone(&self.diagnostics);
        let hub = Arc::clone(&self.hub);
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
            webcam_loop(Arc::clone(&stop), live, id, peers, tx, count, camera_index, quality, diagnostics, hub);
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
        assert_eq!(video_preset(2), (640, 360, 12, 48));
        assert_eq!(video_preset(3), (960, 540, 15, 56));
        assert_eq!(video_preset(99), (480, 270, 10, 50));
    }
    fn sample_jpeg(width: u32, height: u32) -> (RgbImage, Vec<u8>) {
        let image = RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x * 255 / width) as u8, (y * 255 / height) as u8, ((x + y) % 200) as u8])
        });
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 85)
            .encode(image.as_raw(), width, height, ExtendedColorType::Rgb8).unwrap();
        (image, jpeg)
    }
    /// Removes every DHT segment the way many UVC webcams ship their frames.
    fn strip_huffman_tables(jpeg: &[u8]) -> Vec<u8> {
        let (sos, _, _) = scan_jpeg_header(jpeg).unwrap();
        let mut out = vec![0xFF, 0xD8];
        let mut i = 2;
        while i < sos {
            let length = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
            if jpeg[i + 1] != 0xC4 { out.extend_from_slice(&jpeg[i..i + 2 + length]); }
            i += 2 + length;
        }
        out.extend_from_slice(&jpeg[sos..]);
        out
    }
    #[test]
    fn mjpeg_without_huffman_tables_is_repaired_and_decodes() {
        let (_, jpeg) = sample_jpeg(320, 240);
        let stripped = strip_huffman_tables(&jpeg);
        assert!(stripped.len() < jpeg.len());
        assert!(image::load_from_memory_with_format(&stripped, ImageFormat::Jpeg).is_err(),
            "the strict decoder is expected to reject tables-less JPEG");
        let repaired = with_standard_tables(&stripped);
        let original = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg).unwrap().to_rgb8();
        let fixed = image::load_from_memory_with_format(&repaired, ImageFormat::Jpeg).unwrap().to_rgb8();
        assert_eq!(original.as_raw(), fixed.as_raw(), "repair must not change pixels");
    }
    #[test]
    fn complete_jpeg_is_left_untouched() {
        let (_, jpeg) = sample_jpeg(64, 48);
        assert!(matches!(with_standard_tables(&jpeg), Cow::Borrowed(_)));
        assert!(matches!(with_standard_tables(&[1, 2, 3]), Cow::Borrowed(_)));
    }
    #[test]
    fn camera_buffer_with_mjpeg_decodes_without_nokhwa_decoder() {
        let (_, jpeg) = sample_jpeg(640, 480);
        let buffer = Buffer::new(Resolution::new(640, 480), &strip_huffman_tables(&jpeg), FrameFormat::MJPEG);
        let decoded = decode_camera_frame(&buffer).expect("MJPEG frame must decode");
        assert_eq!(decoded.dimensions(), (640, 480));
    }
    #[test]
    fn camera_buffer_with_yuyv_decodes() {
        let (w, h) = (4usize, 2usize);
        let raw = vec![128u8; w * h * 2];
        let buffer = Buffer::new(Resolution::new(w as u32, h as u32), &raw, FrameFormat::YUYV);
        assert_eq!(decode_camera_frame(&buffer).unwrap().dimensions(), (4, 2));
    }
    #[test]
    fn garbage_frames_report_an_error_instead_of_panicking() {
        let buffer = Buffer::new(Resolution::new(640, 480), &[0u8; 100], FrameFormat::MJPEG);
        assert!(decode_camera_frame(&buffer).is_err());
    }
    #[test]
    fn every_preset_resolution_is_accepted_by_the_receiver() {
        for mode in 0..4 {
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
        assert_eq!(frame.rgba.len(),320*180*4);
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
