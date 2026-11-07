//! H.264 video transport used for the HD camera and for screen sharing.
//!
//! Pipeline (sender):  picture -> scale/crop -> I420 -> H.264 -> UDP packets
//! Pipeline (viewer):  UDP packets -> frame reassembly -> H.264 -> RGBA -> UI
//!
//! Frames are split into small packets with one XOR parity packet, so a single
//! lost packet per frame is repaired. When more is lost the viewer asks the
//! sender for a fresh key frame instead of showing a smeared picture.
use crate::i18n::{trf};
use crate::protocol::STREAM_PORT;
use crate::video::{VideoEvent, VideoFrame};
use fast_image_resize as fr;
use openh264::{
    decoder::Decoder,
    encoder::{Encoder, EncoderConfig, FrameType, RateControlMode, UsageType},
    formats::YUVSource,
    OpenH264API,
};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StreamKind {
    Camera,
    Screen,
}
impl StreamKind {
    fn code(self) -> u8 { match self { Self::Camera => 0, Self::Screen => 1 } }
    fn from_code(code: u8) -> Option<Self> {
        match code { 0 => Some(Self::Camera), 1 => Some(Self::Screen), _ => None }
    }
    pub fn index(self) -> usize { self.code() as usize }
}

// ── Colour conversion ──────────────────────────────────────────────────────
/// Planar YUV 4:2:0 picture (BT.601, limited range).
pub struct I420 {
    pub width: usize,
    pub height: usize,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl I420 {
    /// Width and height are rounded down to even numbers (H.264 needs them).
    pub fn new(width: usize, height: usize) -> Self {
        let (width, height) = (width & !1, height & !1);
        Self {
            width, height,
            y: vec![16; width * height],
            u: vec![128; width * height / 4],
            v: vec![128; width * height / 4],
        }
    }

    /// Fills the picture from packed pixels (3 or 4 bytes each, RGB or BGR order).
    pub fn fill_from_packed(&mut self, src: &[u8], stride: usize, channels: usize, bgr: bool) {
        let (width, height) = (self.width, self.height);
        assert!(channels == 3 || channels == 4);
        assert!(src.len() >= stride * (height - 1) + width * channels, "source too small");
        let (ro, bo) = if bgr { (2, 0) } else { (0, 2) };
        let bands = band_count(height);
        let rows_per_band = (height / 2).div_ceil(bands) * 2; // keep bands even
        let y_chunks = self.y.chunks_mut(rows_per_band * width);
        let u_chunks = self.u.chunks_mut(rows_per_band / 2 * (width / 2));
        let v_chunks = self.v.chunks_mut(rows_per_band / 2 * (width / 2));
        thread::scope(|scope| {
            for (index, ((y, u), v)) in y_chunks.zip(u_chunks).zip(v_chunks).enumerate() {
                let first_row = index * rows_per_band;
                scope.spawn(move || {
                    let rows = y.len() / width;
                    for pair in 0..rows / 2 {
                        let r0 = first_row + pair * 2;
                        let line0 = &src[r0 * stride..r0 * stride + width * channels];
                        let line1 = &src[(r0 + 1) * stride..(r0 + 1) * stride + width * channels];
                        for x in 0..width / 2 {
                            let mut sums = [0i32; 3];
                            for (line, row) in [(line0, pair * 2), (line1, pair * 2 + 1)] {
                                for dx in 0..2 {
                                    let px = &line[(x * 2 + dx) * channels..];
                                    let (r, g, b) = (px[ro] as i32, px[1] as i32, px[bo] as i32);
                                    y[row * width + x * 2 + dx] =
                                        (((66 * r + 129 * g + 25 * b + 128) >> 8) + 16) as u8;
                                    sums[0] += r; sums[1] += g; sums[2] += b;
                                }
                            }
                            let (r, g, b) = ((sums[0] + 2) >> 2, (sums[1] + 2) >> 2, (sums[2] + 2) >> 2);
                            u[pair * (width / 2) + x] =
                                (((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
                            v[pair * (width / 2) + x] =
                                (((112 * r - 94 * g - 18 * b + 128) >> 8) + 128).clamp(0, 255) as u8;
                        }
                    }
                });
            }
        });
    }
}

impl YUVSource for I420 {
    fn dimensions(&self) -> (usize, usize) { (self.width, self.height) }
    fn strides(&self) -> (usize, usize, usize) { (self.width, self.width / 2, self.width / 2) }
    fn y(&self) -> &[u8] { &self.y }
    fn u(&self) -> &[u8] { &self.u }
    fn v(&self) -> &[u8] { &self.v }
}

fn band_count(height: usize) -> usize {
    if height < 360 { return 1; }
    thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 4)
}

/// I420 -> RGBA (alpha 255), the exact inverse of [`I420::fill_from_packed`].
pub fn i420_to_rgba(
    y: &[u8], u: &[u8], v: &[u8],
    (width, height): (usize, usize), (sy, su, sv): (usize, usize, usize),
    out: &mut Vec<u8>,
) {
    out.resize(width * height * 4, 255);
    let bands = band_count(height);
    let rows_per_band = height.div_ceil(bands);
    thread::scope(|scope| {
        for (index, chunk) in out.chunks_mut(rows_per_band * width * 4).enumerate() {
            let first_row = index * rows_per_band;
            scope.spawn(move || {
                for (row_in_band, line) in chunk.chunks_exact_mut(width * 4).enumerate() {
                    let row = first_row + row_in_band;
                    let yrow = &y[row * sy..row * sy + width];
                    let urow = &u[(row / 2) * su..];
                    let vrow = &v[(row / 2) * sv..];
                    for x in 0..width {
                        let c = yrow[x] as i32 - 16;
                        let d = urow[x / 2] as i32 - 128;
                        let e = vrow[x / 2] as i32 - 128;
                        let base = 298 * c + 128;
                        line[x * 4] = ((base + 409 * e) >> 8).clamp(0, 255) as u8;
                        line[x * 4 + 1] = ((base - 100 * d - 208 * e) >> 8).clamp(0, 255) as u8;
                        line[x * 4 + 2] = ((base + 516 * d) >> 8).clamp(0, 255) as u8;
                        line[x * 4 + 3] = 255;
                    }
                }
            });
        }
    });
}

// ── Scaling ────────────────────────────────────────────────────────────────
/// Largest even size that fits `max_w × max_h`, keeps the aspect ratio and never upscales.
pub fn fit_size(src_w: usize, src_h: usize, max_w: usize, max_h: usize) -> (usize, usize) {
    if src_w == 0 || src_h == 0 { return (2, 2); }
    let scale = (max_w as f64 / src_w as f64).min(max_h as f64 / src_h as f64).min(1.0);
    let w = ((src_w as f64 * scale).round() as usize & !1).max(2);
    let h = ((src_h as f64 * scale).round() as usize & !1).max(2);
    (w, h)
}

/// Fast SIMD scaler for packed 8-bit pixels (3 or 4 channels).
pub struct Scaler {
    resizer: fr::Resizer,
}
impl Default for Scaler {
    fn default() -> Self { Self { resizer: fr::Resizer::new() } }
}
impl Scaler {
    /// Scales `src` (optionally cropping a region first) to `dst_w × dst_h`.
    pub fn scale(
        &mut self, src: &mut [u8], src_size: (usize, usize), channels: usize,
        crop: Option<(f64, f64, f64, f64)>, dst_size: (usize, usize), out: &mut Vec<u8>,
    ) -> Result<(), String> {
        let pixel = if channels == 4 { fr::PixelType::U8x4 } else { fr::PixelType::U8x3 };
        let source = fr::images::Image::from_slice_u8(src_size.0 as u32, src_size.1 as u32, src, pixel)
            .map_err(|e| e.to_string())?;
        out.resize(dst_size.0 * dst_size.1 * channels, 0);
        let mut target = fr::images::Image::from_slice_u8(dst_size.0 as u32, dst_size.1 as u32, out, pixel)
            .map_err(|e| e.to_string())?;
        let mut options = fr::ResizeOptions::new()
            .resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Bilinear))
            .use_alpha(false);
        if let Some((left, top, width, height)) = crop {
            options = options.crop(left, top, width, height);
        }
        self.resizer.resize(&source, &mut target, &options).map_err(|e| e.to_string())
    }
}

// ── H.264 encoder / decoder ────────────────────────────────────────────────
pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub key: bool,
}

pub struct StreamEncoder {
    inner: Encoder,
    frames: u64,
    key_every: u64,
}
impl StreamEncoder {
    pub fn new(kind: StreamKind, fps: u32, bitrate_bps: u32) -> Result<Self, String> {
        let config = EncoderConfig::new()
            .set_bitrate_bps(bitrate_bps)
            .max_frame_rate(fps as f32)
            .rate_control_mode(RateControlMode::Bitrate)
            .usage_type(match kind {
                StreamKind::Camera => UsageType::CameraVideoRealTime,
                StreamKind::Screen => UsageType::ScreenContentRealTime,
            })
            // The rate controller only honours the bitrate when it may skip a frame
            // that does not fit; skipped frames are simply not sent.
            .enable_skip_frame(true);
        let inner = Encoder::with_api_config(OpenH264API::from_source(), config)
            .map_err(|e| trf("H.264 encoder: {error}", &[("error", &e)]))?;
        // A regular key frame lets late joiners and lossy links recover on their own.
        Ok(Self { inner, frames: 0, key_every: (fps as u64 * 4).max(30) })
    }

    pub fn encode(&mut self, picture: &I420, force_key: bool) -> Result<Option<EncodedFrame>, String> {
        if force_key || self.frames.is_multiple_of(self.key_every) {
            self.inner.force_intra_frame();
        }
        self.frames += 1;
        let stream = self.inner.encode(picture).map_err(|e| trf("H.264 encoding: {error}", &[("error", &e)]))?;
        let key = matches!(stream.frame_type(), FrameType::IDR | FrameType::I);
        if matches!(stream.frame_type(), FrameType::Skip | FrameType::Invalid) { return Ok(None); }
        let data = stream.to_vec();
        if data.is_empty() { return Ok(None); }
        Ok(Some(EncodedFrame { data, key }))
    }
}

pub struct StreamDecoder {
    inner: Decoder,
}
impl StreamDecoder {
    pub fn new() -> Result<Self, String> {
        Ok(Self { inner: Decoder::new().map_err(|e| trf("H.264 decoder: {error}", &[("error", &e)]))? })
    }
    /// Decodes one access unit into `out` (RGBA) and returns the picture size.
    pub fn decode(&mut self, data: &[u8], out: &mut Vec<u8>) -> Result<Option<(usize, usize)>, String> {
        match self.inner.decode(data).map_err(|e| e.to_string())? {
            Some(yuv) => {
                let size = yuv.dimensions();
                i420_to_rgba(yuv.y(), yuv.u(), yuv.v(), size, yuv.strides(), out);
                Ok(Some(size))
            }
            None => Ok(None),
        }
    }
}

// ── Packets ────────────────────────────────────────────────────────────────
const STREAM_MAGIC: u8 = 0xE7;
const REQUEST_MAGIC: u8 = 0xE8;
const HEADER: usize = 27; // magic, kind, sender u64, session u64, seq u32, flags, part u16, total u16
const CHUNK: usize = 1100;
const MAX_PARTS: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    pub kind: StreamKind,
    pub sender: u64,
    pub session: u64,
    pub seq: u32,
    pub key: bool,
    pub part: u16,
    pub total: u16,
}

pub fn seq_newer(candidate: u32, previous: u32) -> bool {
    let difference = candidate.wrapping_sub(previous);
    difference != 0 && difference < (1 << 31)
}

/// Splits one encoded frame into data packets plus one parity packet.
pub fn make_packets(kind: StreamKind, sender: u64, session: u64, seq: u32, key: bool, data: &[u8]) -> Option<Vec<Vec<u8>>> {
    if data.is_empty() { return None; }
    let total = data.len().div_ceil(CHUNK);
    if total > MAX_PARTS { return None; }
    let head = |part: usize| {
        let mut packet = Vec::with_capacity(HEADER + CHUNK + 4);
        packet.push(STREAM_MAGIC);
        packet.push(kind.code());
        packet.extend_from_slice(&sender.to_be_bytes());
        packet.extend_from_slice(&session.to_be_bytes());
        packet.extend_from_slice(&seq.to_be_bytes());
        packet.push(u8::from(key));
        packet.extend_from_slice(&(part as u16).to_be_bytes());
        packet.extend_from_slice(&(total as u16).to_be_bytes());
        packet
    };
    let mut packets = Vec::with_capacity(total + 1);
    let mut parity = [0u8; CHUNK];
    for (part, bytes) in data.chunks(CHUNK).enumerate() {
        for (i, byte) in bytes.iter().enumerate() { parity[i] ^= byte; }
        let mut packet = head(part);
        packet.extend_from_slice(bytes);
        packets.push(packet);
    }
    {
        let mut packet = head(total);
        packet.extend_from_slice(&(data.len() as u32).to_be_bytes());
        packet.extend_from_slice(&parity);
        packets.push(packet);
    }
    Some(packets)
}

pub fn parse_packet(buf: &[u8]) -> Option<(PacketHeader, &[u8])> {
    if buf.len() <= HEADER || buf.len() > HEADER + CHUNK + 4 || buf[0] != STREAM_MAGIC { return None; }
    let kind = StreamKind::from_code(buf[1])?;
    let header = PacketHeader {
        kind,
        sender: u64::from_be_bytes(buf[2..10].try_into().ok()?),
        session: u64::from_be_bytes(buf[10..18].try_into().ok()?),
        seq: u32::from_be_bytes(buf[18..22].try_into().ok()?),
        key: buf[22] != 0,
        part: u16::from_be_bytes(buf[23..25].try_into().ok()?),
        total: u16::from_be_bytes(buf[25..27].try_into().ok()?),
    };
    let (part, total) = (header.part as usize, header.total as usize);
    if total == 0 || total > MAX_PARTS || part > total { return None; }
    let payload = &buf[HEADER..];
    if part == total {
        if payload.len() != CHUNK + 4 { return None; }
    } else if payload.is_empty() || payload.len() > CHUNK || (part + 1 < total && payload.len() != CHUNK) {
        return None;
    }
    Some((header, payload))
}

/// "Please send a key frame" request from a viewer to the sender.
pub fn make_request(kind: StreamKind, requester: u64, session: u64) -> Vec<u8> {
    let mut packet = vec![REQUEST_MAGIC, kind.code()];
    packet.extend_from_slice(&requester.to_be_bytes());
    packet.extend_from_slice(&session.to_be_bytes());
    packet
}
pub fn parse_request(buf: &[u8]) -> Option<(StreamKind, u64, u64)> {
    if buf.len() != 18 || buf[0] != REQUEST_MAGIC { return None; }
    Some((
        StreamKind::from_code(buf[1])?,
        u64::from_be_bytes(buf[2..10].try_into().ok()?),
        u64::from_be_bytes(buf[10..18].try_into().ok()?),
    ))
}

// ── Reassembly ─────────────────────────────────────────────────────────────
struct Partial {
    seq: u32,
    key: bool,
    parts: Vec<Option<Vec<u8>>>,
    parity: Option<(usize, Vec<u8>)>,
    started: Instant,
}
impl Partial {
    fn new(seq: u32, key: bool, total: usize, now: Instant) -> Self {
        Self { seq, key, parts: vec![None; total], parity: None, started: now }
    }
    /// Returns the whole frame once every part is present (or repaired).
    fn insert(&mut self, part: usize, bytes: &[u8]) -> Option<Vec<u8>> {
        let count = self.parts.len();
        if part == count {
            let total_len = u32::from_be_bytes(bytes[..4].try_into().ok()?) as usize;
            if total_len <= (count - 1) * CHUNK || total_len > count * CHUNK { return None; }
            self.parity = Some((total_len, bytes[4..].to_vec()));
        } else {
            self.parts[part].get_or_insert_with(|| bytes.to_vec());
        }
        let missing: Vec<usize> = (0..count).filter(|i| self.parts[*i].is_none()).collect();
        if missing.len() == 1 {
            if let Some((total_len, xor)) = self.parity.as_ref() {
                let index = missing[0];
                let mut repaired = xor.clone();
                for piece in self.parts.iter().flatten() {
                    for (i, byte) in piece.iter().enumerate() { repaired[i] ^= byte; }
                }
                let expected = if index == count - 1 { total_len - (count - 1) * CHUNK } else { CHUNK };
                repaired.truncate(expected);
                self.parts[index] = Some(repaired);
            }
        }
        if self.parts.iter().any(Option::is_none) { return None; }
        let mut frame = Vec::with_capacity(count * CHUNK);
        for part in self.parts.iter() { frame.extend_from_slice(part.as_ref()?); }
        Some(frame)
    }
}

struct Slot {
    session: u64,
    addr: IpAddr,
    last_done: Option<u32>,
    partial: Option<Partial>,
    need_key: bool,
    last_request: Option<Instant>,
    last_seen: Instant,
}

pub struct CompleteFrame {
    pub sender: u64,
    pub kind: StreamKind,
    pub data: Vec<u8>,
}
pub struct KeyRequest {
    pub addr: IpAddr,
    pub kind: StreamKind,
    pub session: u64,
}

#[derive(Default)]
pub struct Reassembler {
    slots: HashMap<(u64, StreamKind), Slot>,
}
impl Reassembler {
    pub fn push(&mut self, header: &PacketHeader, payload: &[u8], from: IpAddr, now: Instant) -> Option<CompleteFrame> {
        let slot = self.slots.entry((header.sender, header.kind)).or_insert_with(|| Slot {
            session: header.session, addr: from, last_done: None, partial: None,
            need_key: true, last_request: None, last_seen: now,
        });
        if slot.session != header.session {
            // The sender restarted its stream: start from scratch.
            *slot = Slot {
                session: header.session, addr: from, last_done: None, partial: None,
                need_key: true, last_request: None, last_seen: now,
            };
        }
        slot.addr = from;
        slot.last_seen = now;
        if slot.last_done.is_some_and(|last| !seq_newer(header.seq, last)) { return None; }
        match slot.partial.as_ref() {
            Some(partial) if partial.seq == header.seq => {}
            Some(partial) if !seq_newer(header.seq, partial.seq) => return None,
            Some(_) => {
                // A newer frame started before the old one completed: it is lost,
                // and every later frame depends on it until the next key frame.
                slot.need_key = true;
                slot.partial = Some(Partial::new(header.seq, header.key, header.total as usize, now));
            }
            None => slot.partial = Some(Partial::new(header.seq, header.key, header.total as usize, now)),
        }
        let partial = slot.partial.as_mut()?;
        if partial.parts.len() != header.total as usize { return None; }
        let data = partial.insert(header.part as usize, payload)?;
        let key = partial.key;
        slot.partial = None;
        let contiguous = slot.last_done.is_some_and(|last| header.seq == last.wrapping_add(1));
        slot.last_done = Some(header.seq);
        if key {
            slot.need_key = false;
        } else if !contiguous {
            slot.need_key = true;
        }
        if slot.need_key { return None; }
        Some(CompleteFrame { sender: header.sender, kind: header.kind, data })
    }

    /// A decoder rejected a frame: ask for a key frame.
    pub fn mark_broken(&mut self, sender: u64, kind: StreamKind) {
        if let Some(slot) = self.slots.get_mut(&(sender, kind)) { slot.need_key = true; }
    }

    /// Drops frames that never completed and forgets silent streams.
    pub fn expire(&mut self, now: Instant) {
        for slot in self.slots.values_mut() {
            if slot.partial.as_ref().is_some_and(|p| now.duration_since(p.started) > Duration::from_millis(400)) {
                slot.partial = None;
                slot.need_key = true;
            }
        }
        self.slots.retain(|_, slot| now.duration_since(slot.last_seen) < Duration::from_secs(6));
    }

    /// Streams that need a key frame, at most one request per stream every 600 ms.
    pub fn key_requests(&mut self, now: Instant) -> Vec<KeyRequest> {
        let mut requests = Vec::new();
        for ((_, kind), slot) in self.slots.iter_mut() {
            let due = slot.last_request.is_none_or(|t| now.duration_since(t) > Duration::from_millis(600));
            if slot.need_key && due {
                slot.last_request = Some(now);
                requests.push(KeyRequest { addr: slot.addr, kind: *kind, session: slot.session });
            }
        }
        requests
    }
}

// ── Network hub ────────────────────────────────────────────────────────────
fn bind_socket() -> Result<UdpSocket, String> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).map_err(|e| e.to_string())?;
    // Key frames arrive as bursts of hundreds of packets: give the OS room for them.
    let _ = socket.set_recv_buffer_size(8 << 20);
    let _ = socket.set_send_buffer_size(4 << 20);
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, STREAM_PORT));
    socket.bind(&address.into()).map_err(|e| trf("could not open UDP port {port}: {error}", &[("port", &STREAM_PORT), ("error", &e)]))?;
    let socket: UdpSocket = socket.into();
    socket.set_read_timeout(Some(Duration::from_millis(50))).map_err(|e| e.to_string())?;
    Ok(socket)
}

fn is_ignorable(error: &std::io::Error) -> bool {
    matches!(error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        | std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::Interrupted)
        || error.raw_os_error() == Some(10054)
}

/// Sends and receives H.264 streams on one UDP port.
pub struct StreamHub {
    socket: Arc<UdpSocket>,
    local_id: u64,
    targets: Arc<Mutex<Vec<Ipv4Addr>>>,
    key_wanted: [Arc<AtomicBool>; 2],
    stop: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
    pub frames_received: Arc<AtomicU64>,
    pub packets_sent: Arc<AtomicU64>,
}

impl StreamHub {
    pub fn start(local_id: u64, targets: Arc<Mutex<Vec<Ipv4Addr>>>, events: SyncSender<VideoEvent>) -> Result<Self, String> {
        let socket = Arc::new(bind_socket()?);
        let stop = Arc::new(AtomicBool::new(false));
        let key_wanted = [Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false))];
        let frames_received = Arc::new(AtomicU64::new(0));
        let (frame_tx, frame_rx) = mpsc::sync_channel::<CompleteFrame>(6);
        let (broken_tx, broken_rx) = mpsc::channel::<(u64, StreamKind)>();

        // Decoding is CPU heavy, so it never runs on the network thread.
        let decode_worker = {
            let stop = Arc::clone(&stop);
            let counter = Arc::clone(&frames_received);
            thread::spawn(move || {
                let mut decoders: HashMap<(u64, StreamKind), StreamDecoder> = HashMap::new();
                let mut rgba = Vec::new();
                while !stop.load(Ordering::Relaxed) {
                    let frame = match frame_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(frame) => frame,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let id = (frame.sender, frame.kind);
                    let decoder = match decoders.entry(id) {
                        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                        std::collections::hash_map::Entry::Vacant(entry) => match StreamDecoder::new() {
                            Ok(decoder) => entry.insert(decoder),
                            Err(_) => continue,
                        },
                    };
                    match decoder.decode(&frame.data, &mut rgba) {
                        Ok(Some((width, height))) => {
                            counter.fetch_add(1, Ordering::Relaxed);
                            let _ = events.try_send(VideoEvent::Frame(VideoFrame {
                                sender_id: frame.sender, kind: frame.kind, width, height,
                                rgba: std::mem::take(&mut rgba),
                            }));
                        }
                        Ok(None) => {}
                        Err(_) => {
                            decoders.remove(&id);
                            let _ = broken_tx.send(id);
                        }
                    }
                }
            })
        };

        let network_worker = {
            let socket = Arc::clone(&socket);
            let stop = Arc::clone(&stop);
            let targets = Arc::clone(&targets);
            let key_wanted = [Arc::clone(&key_wanted[0]), Arc::clone(&key_wanted[1])];
            thread::spawn(move || {
                let mut reassembler = Reassembler::default();
                let mut buffer = [0u8; 2048];
                let mut last_key_grant = [Instant::now() - Duration::from_secs(1); 2];
                while !stop.load(Ordering::Relaxed) {
                    match socket.recv_from(&mut buffer) {
                        Ok((size, source)) => {
                            let IpAddr::V4(ip) = source.ip() else { continue; };
                            if !targets.lock().map(|t| t.contains(&ip)).unwrap_or(false) { continue; }
                            let now = Instant::now();
                            if let Some((kind, _requester, _session)) = parse_request(&buffer[..size]) {
                                // At most one forced key frame per 400 ms keeps a flapping link from flooding.
                                if now.duration_since(last_key_grant[kind.index()]) > Duration::from_millis(400) {
                                    last_key_grant[kind.index()] = now;
                                    key_wanted[kind.index()].store(true, Ordering::Relaxed);
                                }
                            } else if let Some((header, payload)) = parse_packet(&buffer[..size]) {
                                if header.sender == local_id { continue; }
                                if let Some(frame) = reassembler.push(&header, payload, source.ip(), now) {
                                    let (sender, kind) = (frame.sender, frame.kind);
                                    if frame_tx.try_send(frame).is_err() {
                                        reassembler.mark_broken(sender, kind); // decoder is behind
                                    }
                                }
                            }
                        }
                        Err(error) if is_ignorable(&error) => {}
                        Err(_) => thread::sleep(Duration::from_millis(50)),
                    }
                    let now = Instant::now();
                    while let Ok((sender, kind)) = broken_rx.try_recv() { reassembler.mark_broken(sender, kind); }
                    reassembler.expire(now);
                    for request in reassembler.key_requests(now) {
                        let packet = make_request(request.kind, local_id, request.session);
                        let _ = socket.send_to(&packet, SocketAddr::new(request.addr, STREAM_PORT));
                    }
                }
            })
        };

        Ok(Self {
            socket, local_id, targets, key_wanted, stop,
            workers: vec![decode_worker, network_worker],
            frames_received,
            packets_sent: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Number of friends currently in the call (the people who receive the stream).
    pub fn viewers(&self) -> usize { self.targets.lock().map(|t| t.len()).unwrap_or(0) }

    /// True once if a viewer asked for a key frame since the last call.
    pub fn take_key_request(&self, kind: StreamKind) -> bool {
        self.key_wanted[kind.index()].swap(false, Ordering::Relaxed)
    }

    /// Sends one frame to everyone in the call. Returns the number of bytes handed to the network.
    pub fn send(&self, kind: StreamKind, session: u64, seq: u32, key: bool, data: &[u8]) -> usize {
        let Some(packets) = make_packets(kind, self.local_id, session, seq, key, data) else { return 0; };
        let ips = self.targets.lock().map(|t| t.clone()).unwrap_or_default();
        let mut bytes = 0;
        for ip in ips {
            let address = SocketAddr::V4(SocketAddrV4::new(ip, STREAM_PORT));
            for packet in &packets {
                if self.socket.send_to(packet, address).is_ok() {
                    bytes += packet.len();
                    self.packets_sent.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        bytes
    }
}

impl Drop for StreamHub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for worker in self.workers.drain(..) { let _ = worker.join(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: usize, height: usize, rgb: [u8; 3]) -> Vec<u8> {
        (0..width * height).flat_map(|_| rgb).collect()
    }

    fn roundtrip_colour(rgb: [u8; 3], bgr: bool, channels: usize) -> [u8; 3] {
        let (w, h) = (16, 16);
        let mut pixel = vec![];
        for _ in 0..w * h {
            if bgr { pixel.extend_from_slice(&[rgb[2], rgb[1], rgb[0]]); } else { pixel.extend_from_slice(&rgb); }
            if channels == 4 { pixel.push(255); }
        }
        let mut yuv = I420::new(w, h);
        yuv.fill_from_packed(&pixel, w * channels, channels, bgr);
        let mut out = Vec::new();
        i420_to_rgba(yuv.y(), yuv.u(), yuv.v(), (w, h), yuv.strides(), &mut out);
        [out[8 * 16 * 4 + 8 * 4], out[8 * 16 * 4 + 8 * 4 + 1], out[8 * 16 * 4 + 8 * 4 + 2]]
    }

    #[test]
    fn colours_survive_the_yuv_roundtrip() {
        for rgb in [[0, 0, 0], [255, 255, 255], [255, 0, 0], [0, 255, 0], [0, 0, 255], [128, 128, 128], [200, 120, 40]] {
            for (bgr, channels) in [(false, 3), (true, 3), (true, 4), (false, 4)] {
                let got = roundtrip_colour(rgb, bgr, channels);
                for c in 0..3 {
                    assert!((got[c] as i32 - rgb[c] as i32).abs() <= 6,
                        "{rgb:?} bgr={bgr} ch={channels} came back as {got:?}");
                }
            }
        }
    }

    #[test]
    fn fit_size_keeps_aspect_never_upscales_and_is_even() {
        assert_eq!(fit_size(2560, 1440, 1920, 1080), (1920, 1080));
        assert_eq!(fit_size(1280, 720, 1920, 1080), (1280, 720));
        assert_eq!(fit_size(3440, 1440, 1920, 1080), (1920, 804));
        let (w, h) = fit_size(1366, 768, 1280, 720);
        assert!(w % 2 == 0 && h % 2 == 0 && w <= 1280 && h <= 720);
        assert_eq!(fit_size(0, 0, 100, 100), (2, 2));
    }

    #[test]
    fn scaler_crops_and_scales() {
        let mut scaler = Scaler::default();
        let mut src = solid(640, 480, [10, 200, 30]);
        let mut out = Vec::new();
        scaler.scale(&mut src, (640, 480), 3, Some((0.0, 60.0, 640.0, 360.0)), (320, 180), &mut out).unwrap();
        assert_eq!(out.len(), 320 * 180 * 3);
        assert_eq!(&out[..3], &[10, 200, 30]);
        let mut bgra = vec![255u8; 100 * 50 * 4];
        scaler.scale(&mut bgra, (100, 50), 4, None, (50, 24), &mut out).unwrap();
        assert_eq!(out.len(), 50 * 24 * 4);
    }

    #[test]
    fn packets_roundtrip_and_validate() {
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let packets = make_packets(StreamKind::Screen, 7, 99, 12, true, &data).unwrap();
        assert_eq!(packets.len(), 5 + 1); // 5 data parts + parity
        for packet in &packets {
            assert!(packet.len() <= HEADER + CHUNK + 4);
            let (header, _) = parse_packet(packet).unwrap();
            assert_eq!((header.sender, header.session, header.seq, header.key, header.kind),
                (7, 99, 12, true, StreamKind::Screen));
        }
        assert!(parse_packet(&[STREAM_MAGIC, 0]).is_none());
        // Even a tiny single-packet frame gets a repair packet.
        let tiny = make_packets(StreamKind::Camera, 1, 1, 1, false, &[7; 300]).unwrap();
        assert_eq!(tiny.len(), 2);
        let mut reassembler = Reassembler::default();
        let key = make_packets(StreamKind::Camera, 1, 1, 0, true, &[1; 300]).unwrap();
        assert!(feed(&mut reassembler, &key, &[]).is_some());
        let frame = feed(&mut reassembler, &tiny, &[0]).expect("single-packet frame repaired from parity");
        assert_eq!(frame.data, vec![7; 300]);
        assert!(make_packets(StreamKind::Camera, 1, 1, 1, false, &[]).is_none());
        assert!(make_packets(StreamKind::Camera, 1, 1, 1, false, &vec![0; MAX_PARTS * CHUNK + 1]).is_none());
        let request = make_request(StreamKind::Camera, 5, 6);
        assert_eq!(parse_request(&request), Some((StreamKind::Camera, 5, 6)));
        assert!(parse_request(&request[..10]).is_none());
    }

    fn feed(reassembler: &mut Reassembler, packets: &[Vec<u8>], skip: &[usize]) -> Option<CompleteFrame> {
        let ip: IpAddr = "26.0.0.2".parse().unwrap();
        let mut result = None;
        for (i, packet) in packets.iter().enumerate().rev() {
            if skip.contains(&i) { continue; }
            let (header, payload) = parse_packet(packet).unwrap();
            if let Some(frame) = reassembler.push(&header, payload, ip, Instant::now()) { result = Some(frame); }
        }
        result
    }

    #[test]
    fn one_lost_packet_is_repaired_two_are_not() {
        let data: Vec<u8> = (0..7000u32).map(|i| (i * 7 % 253) as u8).collect();
        for lost in [0usize, 3, 6] {
            let mut reassembler = Reassembler::default();
            let packets = make_packets(StreamKind::Camera, 1, 1, 0, true, &data).unwrap();
            let frame = feed(&mut reassembler, &packets, &[lost]).expect("one loss must be repaired");
            assert_eq!(frame.data, data, "lost part {lost}");
        }
        let mut reassembler = Reassembler::default();
        let packets = make_packets(StreamKind::Camera, 1, 1, 0, true, &data).unwrap();
        assert!(feed(&mut reassembler, &packets, &[1, 4]).is_none());
    }

    #[test]
    fn loss_requests_a_key_frame_and_the_next_one_recovers() {
        let mut reassembler = Reassembler::default();
        let key = vec![1u8; 4000];
        let delta = vec![2u8; 3000];
        // Frame 0 (key) arrives, frame 1 is lost, frame 2 is a delta: it must be refused.
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Screen, 1, 5, 0, true, &key).unwrap(), &[]).is_some());
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Screen, 1, 5, 2, false, &delta).unwrap(), &[]).is_none());
        let requests = reassembler.key_requests(Instant::now());
        assert_eq!(requests.len(), 1);
        assert_eq!((requests[0].kind, requests[0].session), (StreamKind::Screen, 5));
        assert!(reassembler.key_requests(Instant::now()).is_empty(), "requests are rate limited");
        // More deltas are still refused, then a key frame restores playback.
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Screen, 1, 5, 3, false, &delta).unwrap(), &[]).is_none());
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Screen, 1, 5, 4, true, &key).unwrap(), &[]).is_some());
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Screen, 1, 5, 5, false, &delta).unwrap(), &[]).is_some());
    }

    #[test]
    fn a_stream_cannot_start_on_a_delta_frame_and_duplicates_are_ignored() {
        let mut reassembler = Reassembler::default();
        let delta = vec![9u8; 2500];
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Camera, 3, 1, 10, false, &delta).unwrap(), &[]).is_none());
        let key = make_packets(StreamKind::Camera, 3, 1, 11, true, &delta).unwrap();
        assert!(feed(&mut reassembler, &key, &[]).is_some());
        assert!(feed(&mut reassembler, &key, &[]).is_none(), "duplicate frame");
    }

    #[test]
    fn restarted_stream_session_resets_state() {
        let mut reassembler = Reassembler::default();
        let key = vec![1u8; 2200];
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Screen, 1, 1, 500, true, &key).unwrap(), &[]).is_some());
        // New session, sequence numbers start over.
        assert!(feed(&mut reassembler, &make_packets(StreamKind::Screen, 1, 2, 0, true, &key).unwrap(), &[]).is_some());
    }

    #[test]
    fn sequence_numbers_wrap_around() {
        assert!(seq_newer(0, u32::MAX));
        assert!(!seq_newer(u32::MAX, 0));
        assert!(!seq_newer(5, 5));
    }

    fn moving_frame(width: usize, height: usize, t: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                data.extend_from_slice(&[((x + t * 4) % 256) as u8, (y * 255 / height) as u8, 90]);
            }
        }
        data
    }

    #[test]
    fn real_h264_roundtrip_through_lossy_packets() {
        let (w, h) = (640, 360);
        let mut encoder = StreamEncoder::new(StreamKind::Camera, 30, 1_500_000).unwrap();
        let mut decoder = StreamDecoder::new().unwrap();
        let mut reassembler = Reassembler::default();
        let mut yuv = I420::new(w, h);
        let mut rgba = Vec::new();
        let mut decoded_frames = 0;
        let mut key_frames = 0;
        for t in 0..20usize {
            let source = moving_frame(w, h, t);
            yuv.fill_from_packed(&source, w * 3, 3, false);
            let Some(encoded) = encoder.encode(&yuv, false).unwrap() else { continue };
            if t == 0 { assert!(encoded.key, "first frame must be a key frame"); }
            key_frames += usize::from(encoded.key);
            let packets = make_packets(StreamKind::Camera, 1, 1, t as u32, encoded.key, &encoded.data).unwrap();
            // Drop exactly one packet of every frame: parity must repair it.
            let frame = feed(&mut reassembler, &packets, &[t % packets.len()]).expect("frame must be rebuilt");
            assert_eq!(frame.data, encoded.data);
            let size = decoder.decode(&frame.data, &mut rgba).unwrap().expect("decoded picture");
            assert_eq!(size, (w, h));
            decoded_frames += 1;
            // Picture quality: mean absolute error against the source stays small.
            let mut error = 0u64;
            for i in 0..w * h {
                for c in 0..3 { error += (rgba[i * 4 + c] as i32 - source[i * 3 + c] as i32).unsigned_abs() as u64; }
            }
            let mean = error as f64 / (w * h * 3) as f64;
            assert!(mean < 12.0, "frame {t}: mean error {mean:.1}");
        }
        assert_eq!(decoded_frames, 20);
        assert!(key_frames >= 1);
    }

    #[test]
    fn encoder_honours_forced_key_frames_and_handles_resolution_1080p() {
        let (w, h) = (1920, 1080);
        let mut encoder = StreamEncoder::new(StreamKind::Screen, 30, 6_000_000).unwrap();
        let mut yuv = I420::new(w, h);
        let source = moving_frame(w, h, 0);
        yuv.fill_from_packed(&source, w * 3, 3, false);
        let first = encoder.encode(&yuv, false).unwrap().unwrap();
        assert!(first.key);
        let second = encoder.encode(&yuv, false).unwrap().unwrap();
        assert!(!second.key);
        let forced = encoder.encode(&yuv, true).unwrap().unwrap();
        assert!(forced.key, "forced key frame");
        assert!(first.data.len() <= MAX_PARTS * CHUNK, "a 1080p key frame must fit the packet limit");
    }
}
