//! Screen sharing: capture a whole monitor or a single window and stream it as H.264.
//!
//! Capture uses the Windows Graphics Capture API (the same one Windows' own
//! screen recorder uses): it can grab a window even when it is partly covered,
//! includes the mouse cursor, and is hardware accelerated.
use crate::stream::{fit_size, EncodedFrame, I420, Scaler, StreamEncoder, StreamHub, StreamKind};
use crate::video::{VideoEvent, VideoFrame};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::SyncSender,
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

// ── What can be shared ─────────────────────────────────────────────────────
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum ShareTarget {
    Monitor { handle: usize },
    Window { handle: usize },
}

#[derive(Clone, Debug)]
pub struct ShareSource {
    pub target: ShareTarget,
    /// Main line in the picker: "Екран 1" or the window title.
    pub title: String,
    /// Second line: resolution and refresh rate, or the program name.
    pub detail: String,
    /// Size of the source in pixels, `(0, 0)` when unknown.
    pub size: (usize, usize),
}

impl ShareSource {
    pub fn is_window(&self) -> bool { matches!(self.target, ShareTarget::Window { .. }) }
}

/// Monitors and windows that can be shared right now.
pub fn list_sources() -> Vec<ShareSource> {
    #[cfg(windows)]
    { windows_impl::list_sources() }
    #[cfg(not(windows))]
    { Vec::new() }
}

// ── Quality ────────────────────────────────────────────────────────────────
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareQuality {
    /// Maximum height of the sent picture. The source is only ever scaled down.
    pub max_height: usize,
    pub fps: u32,
}

pub const RESOLUTIONS: [(usize, &str); 4] = [
    (720, "720p"),
    (1080, "1080p"),
    (1440, "1440p"),
    (2160, "Оригінал (до 4K)"),
];
pub const FRAME_RATES: [u32; 3] = [15, 30, 60];

impl Default for ShareQuality {
    fn default() -> Self { Self { max_height: 1080, fps: 30 } }
}

impl ShareQuality {
    /// Size actually sent for a source of the given size.
    pub fn output_size(&self, source_w: usize, source_h: usize) -> (usize, usize) {
        fit_size(source_w, source_h, 3840, self.max_height)
    }
    /// Target bit rate: about 0.1 bit per pixel per frame suits text and UI,
    /// and stays watchable for games and video.
    pub fn bitrate(&self, width: usize, height: usize) -> u32 {
        let bits = width as f64 * height as f64 * self.fps as f64 * 0.1;
        (bits as u32).clamp(1_000_000, 24_000_000)
    }
}

// ── Frames handed from the capture API to the encoder ──────────────────────
pub struct RawFrame {
    pub width: usize,
    pub height: usize,
    /// BGRA, tightly packed.
    pub bgra: Vec<u8>,
}

/// Single-slot mailbox: the encoder always gets the newest frame; older ones are dropped.
#[derive(Default)]
pub struct Mailbox {
    slot: Mutex<Option<RawFrame>>,
    arrived: Condvar,
    spare: Mutex<Vec<Vec<u8>>>,
    closed: AtomicBool,
    reason: Mutex<Option<String>>,
}
impl Mailbox {
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn push(&self, frame: RawFrame) {
        if let Ok(mut slot) = self.slot.lock() {
            if let Some(old) = slot.replace(frame) { self.recycle(old.bgra); }
        }
        self.arrived.notify_one();
    }
    pub fn take(&self, timeout: Duration) -> Option<RawFrame> {
        let mut slot = self.slot.lock().ok()?;
        if slot.is_none() {
            slot = self.arrived.wait_timeout(slot, timeout).ok()?.0;
        }
        slot.take()
    }
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn buffer(&self) -> Vec<u8> { self.spare.lock().ok().and_then(|mut s| s.pop()).unwrap_or_default() }
    pub fn recycle(&self, buffer: Vec<u8>) {
        if let Ok(mut spare) = self.spare.lock() { if spare.len() < 3 { spare.push(buffer); } }
    }
    pub fn close(&self, reason: &str) {
        if let Ok(mut r) = self.reason.lock() { r.get_or_insert_with(|| reason.to_string()); }
        self.closed.store(true, Ordering::Relaxed);
        self.arrived.notify_all();
    }
    pub fn is_closed(&self) -> bool { self.closed.load(Ordering::Relaxed) }
    pub fn reason(&self) -> Option<String> { self.reason.lock().ok().and_then(|r| r.clone()) }
}

// ── Statistics shown in the UI ─────────────────────────────────────────────
#[derive(Default)]
pub struct ShareStats {
    pub frames: AtomicU64,
    pub bytes: AtomicU64,
    pub width: AtomicU32,
    pub height: AtomicU32,
    /// Smoothed time to scale + convert + encode one frame, in microseconds.
    pub encode_us: AtomicU64,
    pub overloaded: AtomicBool,
}

pub fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for pixel in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }
    rgba
}

/// Everything the encoding loop needs from the outside world.
pub struct LoopIo<'a> {
    /// Number of friends receiving the stream.
    pub viewers: &'a dyn Fn() -> usize,
    /// True once if a viewer asked for a key frame.
    pub key_requested: &'a dyn Fn() -> bool,
    /// Sends one encoded frame; returns bytes put on the wire.
    pub send: &'a mut dyn FnMut(&EncodedFrame) -> usize,
    /// Receives a small RGBA picture for the presenter's own preview.
    pub preview: &'a mut dyn FnMut(usize, usize, Vec<u8>),
}

/// Takes frames from the mailbox, scales, encodes and sends them until stopped.
pub fn run_share_loop(
    mailbox: &Mailbox, stop: &AtomicBool, quality: ShareQuality, stats: &ShareStats,
    io: &mut LoopIo<'_>,
) -> Result<(), String> {
    let mut scaler = Scaler::default();
    let mut scaled: Vec<u8> = Vec::new();
    let mut small: Vec<u8> = Vec::new();
    let mut picture: Option<I420> = None;
    let mut encoder: Option<(StreamEncoder, (usize, usize))> = None;
    let mut last: Option<RawFrame> = None;
    let mut last_encode = Instant::now() - Duration::from_secs(1);
    let mut last_preview = Instant::now() - Duration::from_secs(1);
    let mut was_sending = false;
    let frame_budget = Duration::from_micros(1_000_000 / quality.fps.max(1) as u64);

    while !stop.load(Ordering::Relaxed) && !mailbox.is_closed() {
        match mailbox.take(Duration::from_millis(250)) {
            Some(frame) => {
                if let Some(old) = last.replace(frame) { mailbox.recycle(old.bgra); }
            }
            None => {
                // A still screen produces no new frames. Send the last one again from
                // time to time: it costs almost nothing and keeps viewers' pictures alive.
                if last.is_none() || last_encode.elapsed() < Duration::from_millis(500) { continue; }
            }
        }
        let Some(frame) = last.as_mut() else { continue; };
        let started = Instant::now();
        let (out_w, out_h) = quality.output_size(frame.width, frame.height);

        // Presenter's own preview, about 8 times per second.
        if last_preview.elapsed() >= Duration::from_millis(125) {
            last_preview = Instant::now();
            let (pw, ph) = fit_size(frame.width, frame.height, 480, 270);
            if scaler.scale(&mut frame.bgra, (frame.width, frame.height), 4, None, (pw, ph), &mut small).is_ok() {
                (io.preview)(pw, ph, bgra_to_rgba(&small));
            }
        }

        if (io.viewers)() == 0 {
            was_sending = false;
            let _ = (io.key_requested)();
            last_encode = Instant::now();
            continue;
        }

        let (pixels, stride): (&[u8], usize) = if (out_w, out_h) == (frame.width, frame.height) {
            (&frame.bgra, frame.width * 4)
        } else {
            scaler.scale(&mut frame.bgra, (frame.width, frame.height), 4, None, (out_w, out_h), &mut scaled)?;
            (&scaled, out_w * 4)
        };
        if picture.as_ref().is_none_or(|p| p.width != out_w || p.height != out_h) {
            picture = Some(I420::new(out_w, out_h));
        }
        let Some(yuv) = picture.as_mut() else { continue; };
        yuv.fill_from_packed(pixels, stride, 4, true);

        if encoder.as_ref().is_none_or(|(_, size)| *size != (out_w, out_h)) {
            encoder = Some((
                StreamEncoder::new(StreamKind::Screen, quality.fps, quality.bitrate(out_w, out_h))?,
                (out_w, out_h),
            ));
            was_sending = false;
        }
        let Some((encoder, _)) = encoder.as_mut() else { continue; };
        let force_key = (io.key_requested)() || !was_sending;
        was_sending = true;
        if let Some(encoded) = encoder.encode(yuv, force_key)? {
            let bytes = (io.send)(&encoded);
            stats.bytes.fetch_add(bytes as u64, Ordering::Relaxed);
            stats.frames.fetch_add(1, Ordering::Relaxed);
        }
        stats.width.store(out_w as u32, Ordering::Relaxed);
        stats.height.store(out_h as u32, Ordering::Relaxed);
        last_encode = Instant::now();

        let took = started.elapsed().as_micros() as u64;
        let smoothed = (stats.encode_us.load(Ordering::Relaxed) * 7 + took) / 8;
        stats.encode_us.store(smoothed, Ordering::Relaxed);
        stats.overloaded.store(Duration::from_micros(smoothed) > frame_budget.mul_f32(0.95), Ordering::Relaxed);
    }
    Ok(())
}

// ── A running share ────────────────────────────────────────────────────────
pub struct ScreenShare {
    stop: Arc<AtomicBool>,
    mailbox: Arc<Mailbox>,
    worker: Option<JoinHandle<()>>,
    stop_capture: Option<Box<dyn FnOnce() + Send>>,
    pub stats: Arc<ShareStats>,
    pub quality: ShareQuality,
    pub title: String,
    failure: Arc<Mutex<Option<String>>>,
}

impl ScreenShare {
    #[cfg(windows)]
    pub fn start(
        source: &ShareSource, quality: ShareQuality, hub: Arc<StreamHub>,
        events: SyncSender<VideoEvent>, local_id: u64,
    ) -> Result<Self, String> {
        let mailbox = Arc::new(Mailbox::default());
        let stop_capture = windows_impl::start_capture(&source.target, Arc::clone(&mailbox), quality.fps)?;
        Ok(Self::spawn(mailbox, Some(stop_capture), source.title.clone(), quality, hub, events, local_id))
    }

    #[cfg(not(windows))]
    pub fn start(
        _source: &ShareSource, _quality: ShareQuality, _hub: Arc<StreamHub>,
        _events: SyncSender<VideoEvent>, _local_id: u64,
    ) -> Result<Self, String> {
        Err("Показ екрана працює лише у Windows.".into())
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn spawn(
        mailbox: Arc<Mailbox>, stop_capture: Option<Box<dyn FnOnce() + Send>>, title: String,
        quality: ShareQuality, hub: Arc<StreamHub>, events: SyncSender<VideoEvent>, local_id: u64,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(ShareStats::default());
        let failure = Arc::new(Mutex::new(None));
        let worker = {
            let (stop, mailbox, stats, failure) =
                (Arc::clone(&stop), Arc::clone(&mailbox), Arc::clone(&stats), Arc::clone(&failure));
            thread::spawn(move || {
                let session = rand::random::<u64>().max(1);
                let mut seq = 0u32;
                let viewers = || hub.viewers();
                let key_requested = || hub.take_key_request(StreamKind::Screen);
                let mut send = |frame: &EncodedFrame| {
                    let bytes = hub.send(StreamKind::Screen, session, seq, frame.key, &frame.data);
                    seq = seq.wrapping_add(1);
                    bytes
                };
                let mut preview = |width: usize, height: usize, rgba: Vec<u8>| {
                    let _ = events.try_send(VideoEvent::Frame(VideoFrame {
                        sender_id: local_id, kind: StreamKind::Screen, width, height, rgba,
                    }));
                };
                let mut io = LoopIo { viewers: &viewers, key_requested: &key_requested, send: &mut send, preview: &mut preview };
                if let Err(error) = run_share_loop(&mailbox, &stop, quality, &stats, &mut io) {
                    if let Ok(mut f) = failure.lock() { *f = Some(error); }
                }
                mailbox.close("stopped");
            })
        };
        Self { stop, mailbox, worker: Some(worker), stop_capture, stats, quality, title, failure }
    }

    /// False once the share ended by itself (window closed, error) or was stopped.
    pub fn is_running(&self) -> bool {
        !self.mailbox.is_closed() && self.worker.as_ref().is_some_and(|w| !w.is_finished())
    }
    /// Why the share ended on its own, if it did.
    pub fn end_reason(&self) -> Option<String> {
        if let Some(error) = self.failure.lock().ok().and_then(|f| f.clone()) { return Some(error); }
        self.mailbox.reason().filter(|reason| reason != "stopped" && reason != "manual")
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.mailbox.close("manual");
        if let Some(stop_capture) = self.stop_capture.take() { stop_capture(); }
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
impl Drop for ScreenShare {
    fn drop(&mut self) { self.stop(); }
}

// ── Windows: capture through Windows Graphics Capture ──────────────────────
#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::ffi::c_void;
    use windows_capture::{
        capture::{Context, GraphicsCaptureApiHandler},
        frame::Frame,
        graphics_capture_api::InternalCaptureControl,
        monitor::Monitor,
        settings::{
            ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
            MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
        },
        window::Window,
    };

    type BoxError = Box<dyn std::error::Error + Send + Sync>;

    pub fn list_sources() -> Vec<ShareSource> {
        let mut sources = Vec::new();
        let primary = Monitor::primary().ok().map(|m| m.as_raw_hmonitor() as usize);
        if let Ok(monitors) = Monitor::enumerate() {
            for (index, monitor) in monitors.iter().enumerate() {
                let (width, height) = (monitor.width().unwrap_or(0), monitor.height().unwrap_or(0));
                let hz = monitor.refresh_rate().unwrap_or(0);
                let handle = monitor.as_raw_hmonitor() as usize;
                let mut detail = format!("{width}×{height}");
                if hz > 0 { detail.push_str(&format!(", {hz} Гц")); }
                if Some(handle) == primary { detail.push_str(", основний"); }
                sources.push(ShareSource {
                    target: ShareTarget::Monitor { handle },
                    title: format!("Екран {}", index + 1),
                    detail,
                    size: (width as usize, height as usize),
                });
            }
        }
        if let Ok(windows) = Window::enumerate() {
            let mut found: Vec<ShareSource> = windows.iter().filter_map(|window| {
                let title = window.title().ok()?.trim().to_string();
                if title.is_empty() || title == "Friends Voice" || title == "Program Manager" { return None; }
                let process = window.process_name().unwrap_or_default();
                let size = window.rect().map(|r| ((r.right - r.left).max(0) as usize, (r.bottom - r.top).max(0) as usize)).unwrap_or((0, 0));
                Some(ShareSource {
                    target: ShareTarget::Window { handle: window.as_raw_hwnd() as usize },
                    title,
                    detail: process,
                    size,
                })
            }).collect();
            found.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
            sources.extend(found);
        }
        sources
    }

    struct Handler {
        mailbox: Arc<Mailbox>,
        interval: Duration,
        last: Instant,
    }

    impl GraphicsCaptureApiHandler for Handler {
        type Flags = (Arc<Mailbox>, u32);
        type Error = BoxError;

        fn new(context: Context<Self::Flags>) -> Result<Self, Self::Error> {
            let (mailbox, fps) = context.flags;
            Ok(Self { mailbox, interval: Duration::from_micros(1_000_000 / fps.max(1) as u64), last: Instant::now() - Duration::from_secs(1) })
        }

        fn on_frame_arrived(&mut self, frame: &mut Frame, control: InternalCaptureControl) -> Result<(), Self::Error> {
            if self.mailbox.is_closed() {
                control.stop();
                return Ok(());
            }
            // Windows may deliver more frames than we want to send; skip them
            // before paying for the copy.
            if self.last.elapsed() < self.interval { return Ok(()); }
            self.last = Instant::now();
            let (width, height) = (frame.width() as usize, frame.height() as usize);
            let mut buffer = frame.buffer()?;
            let data = buffer.as_nopadding_buffer()?;
            let needed = width * height * 4;
            if data.len() < needed || width == 0 || height == 0 { return Ok(()); }
            let mut bgra = self.mailbox.buffer();
            bgra.clear();
            bgra.extend_from_slice(&data[..needed]);
            self.mailbox.push(RawFrame { width, height, bgra });
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            self.mailbox.close("Показ зупинено: вікно або екран закрито.");
            Ok(())
        }
    }

    fn settings<T: windows_capture::settings::TryIntoCaptureItemWithType>(
        item: T, mailbox: Arc<Mailbox>, fps: u32,
    ) -> Settings<(Arc<Mailbox>, u32), T> {
        Settings::new(
            item,
            CursorCaptureSettings::WithCursor,
            DrawBorderSettings::Default,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Bgra8,
            (mailbox, fps),
        )
    }

    /// Starts capturing on its own thread and returns a function that stops it.
    pub fn start_capture(target: &ShareTarget, mailbox: Arc<Mailbox>, fps: u32) -> Result<Box<dyn FnOnce() + Send>, String> {
        let explain = |e: String| format!(
            "Не вдалося почати показ: {e}. Потрібна Windows 10 версії 1903 або новіша.");
        match target {
            ShareTarget::Monitor { handle } => {
                let monitor = Monitor::from_raw_hmonitor(*handle as *mut c_void);
                let control = Handler::start_free_threaded(settings(monitor, mailbox, fps))
                    .map_err(|e| explain(e.to_string()))?;
                Ok(Box::new(move || { let _ = control.stop(); }))
            }
            ShareTarget::Window { handle } => {
                let window = Window::from_raw_hwnd(*handle as *mut c_void);
                if !window.is_valid() { return Err("Це вікно вже закрите або його не можна показувати.".into()); }
                let control = Handler::start_free_threaded(settings(window, mailbox, fps))
                    .map_err(|e| explain(e.to_string()))?;
                Ok(Box::new(move || { let _ = control.stop(); }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{parse_packet, make_packets, Reassembler, StreamDecoder};
    use std::net::IpAddr;

    fn desktop(width: usize, height: usize, t: usize) -> RawFrame {
        // Window-like content: flat background, a "text" block and a moving square.
        let mut bgra = vec![0u8; width * height * 4];
        let travel = width.saturating_sub(80).max(1);
        for y in 0..height {
            for x in 0..width {
                let i = (y * width + x) * 4;
                let in_text = y > height / 6 && y < height / 3 && x > width / 8 && x < width * 3 / 4 && (x / 3 + y / 4) % 7 < 2;
                let left = (t * 20) % travel;
                let in_box = x > left && x < left + 80 && y > height / 2 && y < height / 2 + 80;
                let (r, g, b) = if in_box { (230, 60, 60) } else if in_text { (20, 20, 20) } else { (240, 240, 235) };
                bgra[i] = b; bgra[i + 1] = g; bgra[i + 2] = r; bgra[i + 3] = 255;
            }
        }
        RawFrame { width, height, bgra }
    }

    #[test]
    fn quality_math_is_sane() {
        let quality = ShareQuality { max_height: 1080, fps: 30 };
        assert_eq!(quality.output_size(2560, 1440), (1920, 1080));
        assert_eq!(quality.output_size(1280, 720), (1280, 720), "never upscaled");
        assert_eq!(ShareQuality { max_height: 720, fps: 30 }.output_size(1920, 1080), (1280, 720));
        let b1080p30 = quality.bitrate(1920, 1080);
        assert!((5_000_000..8_000_000).contains(&b1080p30), "{b1080p30}");
        assert!(ShareQuality { max_height: 1080, fps: 60 }.bitrate(1920, 1080) > b1080p30);
        assert_eq!(ShareQuality { max_height: 720, fps: 15 }.bitrate(160, 90), 1_000_000, "lower clamp");
        assert_eq!(ShareQuality { max_height: 2160, fps: 60 }.bitrate(3840, 2160), 24_000_000, "upper clamp");
        assert_eq!(bgra_to_rgba(&[1, 2, 3, 0]), vec![3, 2, 1, 255]);
    }

    #[test]
    fn mailbox_keeps_only_the_newest_frame_and_wakes_waiters() {
        let mailbox = Arc::new(Mailbox::default());
        assert!(mailbox.take(Duration::from_millis(10)).is_none());
        mailbox.push(desktop(8, 8, 0));
        mailbox.push(desktop(16, 16, 0));
        assert_eq!(mailbox.take(Duration::from_millis(10)).unwrap().width, 16);
        let waiter = { let m = Arc::clone(&mailbox); thread::spawn(move || m.take(Duration::from_secs(2)).map(|f| f.width)) };
        thread::sleep(Duration::from_millis(50));
        mailbox.push(desktop(32, 32, 0));
        assert_eq!(waiter.join().unwrap(), Some(32));
        mailbox.close("Вікно закрито");
        assert!(mailbox.is_closed());
        assert_eq!(mailbox.reason().as_deref(), Some("Вікно закрито"));
    }

    #[test]
    fn full_pipeline_captures_scales_encodes_loses_packets_and_decodes() {
        let mailbox = Arc::new(Mailbox::default());
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(ShareStats::default());
        let quality = ShareQuality { max_height: 360, fps: 30 }; // 1280x720 source -> 640x360
        let sent: Arc<Mutex<Vec<(bool, Vec<u8>)>>> = Arc::new(Mutex::new(Vec::new()));
        let previews = Arc::new(AtomicU64::new(0));

        let worker = {
            let (mailbox, stop, stats, sent, previews) =
                (Arc::clone(&mailbox), Arc::clone(&stop), Arc::clone(&stats), Arc::clone(&sent), Arc::clone(&previews));
            thread::spawn(move || {
                let viewers = || 1usize;
                let key_requested = || false;
                let mut send = |frame: &EncodedFrame| { sent.lock().unwrap().push((frame.key, frame.data.clone())); frame.data.len() };
                let mut preview = |w: usize, h: usize, rgba: Vec<u8>| {
                    assert_eq!((w, h), (480, 270));
                    assert_eq!(rgba.len(), w * h * 4);
                    previews.fetch_add(1, Ordering::Relaxed);
                };
                let mut io = LoopIo { viewers: &viewers, key_requested: &key_requested, send: &mut send, preview: &mut preview };
                run_share_loop(&mailbox, &stop, quality, &stats, &mut io).unwrap();
            })
        };
        for t in 0..12 {
            mailbox.push(desktop(1280, 720, t));
            // Wait for the encoder, so the test does not depend on machine speed.
            let deadline = Instant::now() + Duration::from_secs(20);
            while sent.lock().unwrap().len() <= t && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
        }
        stop.store(true, Ordering::Relaxed);
        worker.join().unwrap();

        let frames = sent.lock().unwrap().clone();
        assert!(frames.len() >= 12, "only {} frames sent", frames.len());
        assert!(frames[0].0, "stream starts with a key frame");
        assert_eq!((stats.width.load(Ordering::Relaxed), stats.height.load(Ordering::Relaxed)), (640, 360));
        assert!(stats.bytes.load(Ordering::Relaxed) > 0 && previews.load(Ordering::Relaxed) >= 1);

        // The viewer side: packets with one lost packet per frame -> decode.
        let mut reassembler = Reassembler::default();
        let mut decoder = StreamDecoder::new().unwrap();
        let ip: IpAddr = "26.1.1.1".parse().unwrap();
        let mut rgba = Vec::new();
        let mut shown = 0;
        for (seq, (key, data)) in frames.iter().enumerate() {
            let packets = make_packets(StreamKind::Screen, 9, 1, seq as u32, *key, data).unwrap();
            let skip = seq % packets.len();
            let mut complete = None;
            for (i, packet) in packets.iter().enumerate() {
                if i == skip { continue; }
                let (header, payload) = parse_packet(packet).unwrap();
                if let Some(frame) = reassembler.push(&header, payload, ip, Instant::now()) { complete = Some(frame); }
            }
            let frame = complete.expect("frame rebuilt despite a lost packet");
            if decoder.decode(&frame.data, &mut rgba).unwrap() == Some((640, 360)) { shown += 1; }
        }
        assert_eq!(shown, frames.len());
        // Background colour survives scaling + H.264 (240,240,235 in the top-left corner).
        let corner = &rgba[(10 * 640 + 10) * 4..(10 * 640 + 10) * 4 + 3];
        for (got, want) in corner.iter().zip([240u8, 240, 235]) {
            assert!((*got as i32 - want as i32).abs() <= 8, "corner colour {corner:?}");
        }
    }

    #[test]
    fn a_still_screen_is_kept_alive_and_no_viewers_means_no_encoding() {
        let mailbox = Arc::new(Mailbox::default());
        let stop = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(ShareStats::default());
        let count = Arc::new(AtomicU64::new(0));
        let audience = Arc::new(AtomicU32::new(0));
        let worker = {
            let (mailbox, stop, stats, count, audience) =
                (Arc::clone(&mailbox), Arc::clone(&stop), Arc::clone(&stats), Arc::clone(&count), Arc::clone(&audience));
            thread::spawn(move || {
                let viewers = || audience.load(Ordering::Relaxed) as usize;
                let key_requested = || false;
                let mut send = |frame: &EncodedFrame| { count.fetch_add(1, Ordering::Relaxed); frame.data.len() };
                let mut preview = |_: usize, _: usize, _: Vec<u8>| {};
                let mut io = LoopIo { viewers: &viewers, key_requested: &key_requested, send: &mut send, preview: &mut preview };
                run_share_loop(&mailbox, &stop, ShareQuality { max_height: 360, fps: 15 }, &stats, &mut io).unwrap();
            })
        };
        mailbox.push(desktop(640, 360, 0));
        thread::sleep(Duration::from_millis(1400));
        assert_eq!(count.load(Ordering::Relaxed), 0, "nobody is watching: nothing is encoded");
        audience.store(1, Ordering::Relaxed);
        thread::sleep(Duration::from_millis(1800));
        let frames = count.load(Ordering::Relaxed);
        assert!((2..=6).contains(&frames), "keep-alive should repeat ~2 frames per second, got {frames}");
        stop.store(true, Ordering::Relaxed);
        worker.join().unwrap();
    }
}
