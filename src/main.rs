#![windows_subsystem = "windows"]
mod mesh;
mod protocol;
mod voice;
mod video;
mod diagnostics;
mod i18n;
mod stream;
mod screen;
mod sounds;
mod ui;
mod branding;

use eframe::egui::{self, Color32};
use mesh::{Mesh, MeshEvent, PeerView};
use protocol::{CONTROL_PORT, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::PathBuf,
    sync::{atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering}, Arc, Mutex},
    time::{Duration, Instant},
};
use voice::VoiceSession;
use i18n::{decimal, tr, trf, Lang};
use screen::{ScreenShare, ShareQuality, ShareSource};
use sounds::{Sound, SoundPlayer};
use stream::StreamKind;
use video::{VideoSession, VideoEvent};


const MAX_CAMERA_INDEX: u32 = 15;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    nickname: String,
    known_ips: Vec<String>,
    input_device: String,
    output_device: String,
    volume: f32,
    push_to_talk: bool,
    ptt_key: i32,
    camera_index: u32,
    video_quality: u32,
    noise_gate: bool,
    gate_threshold: f32,
    /// Last used screen-share quality.
    share_height: usize,
    share_fps: u32,
    sounds_enabled: bool,
    sounds_volume: f32,
    /// Interface language code: "en" (default), "uk" or "ru".
    language: String,
    /// Interface scale factor (1.0 = 100%).
    ui_scale: f32,
    window_width: f32,
    window_height: f32,
    window_maximized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nickname: "Friend".into(),
            known_ips: vec![],
            input_device: String::new(),
            output_device: String::new(),
            volume: 1.0,
            push_to_talk: false,
            ptt_key: voice::PTT_VK,
            camera_index: 0,
            video_quality: 4,
            noise_gate: false, // Opt-in: never silently clip quiet microphones.
            gate_threshold: 0.018,
            share_height: 1080,
            share_fps: 30,
            sounds_enabled: true,
            sounds_volume: 0.6,
            language: "en".into(),
            ui_scale: 1.0,
            window_width: 0.0,
            window_height: 0.0,
            window_maximized: false,
        }
    }
}

/// Flips tightly packed pixel rows left to right.
fn mirror_horizontally(pixels: &mut [u8], width: usize, bytes_per_pixel: usize) {
    if width == 0 { return; }
    for row in pixels.chunks_exact_mut(width * bytes_per_pixel) {
        for x in 0..width / 2 {
            let (left, right) = (x * bytes_per_pixel, (width - 1 - x) * bytes_per_pixel);
            for c in 0..bytes_per_pixel { row.swap(left + c, right + c); }
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(|dir| PathBuf::from(dir).join("FriendsVoice").join("mesh-settings.json"))
}

fn load_settings() -> Settings {
    settings_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &Settings) {
    let Some(path) = settings_path() else { return; };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() { return; }
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        // A crash during write should not destroy the user's last known settings.
        // A same-directory temporary file allows an atomic rename on Windows.
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, &json).is_ok() && fs::rename(&tmp, &path).is_err() {
            let _ = fs::write(&path, json);
            let _ = fs::remove_file(&tmp);
        }
    }
}

fn parse_known_ip(text: &str) -> Option<SocketAddr> {
    let ip: Ipv4Addr = text.trim().parse().ok()?;
    if ip.octets()[0] == 26 || ip.is_loopback() {
        Some(SocketAddr::V4(SocketAddrV4::new(ip, CONTROL_PORT)))
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct WinSystemTime { year: u16, month: u16, weekday: u16, day: u16, hour: u16, minute: u16, second: u16, millis: u16 }
#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLocalTime(time: *mut WinSystemTime);
}
/// Wall-clock time as `HH:MM` in the user's own time zone.
fn clock_now() -> String {
    #[cfg(target_os = "windows")]
    {
        let mut time = WinSystemTime { year: 0, month: 0, weekday: 0, day: 0, hour: 0, minute: 0, second: 0, millis: 0 };
        unsafe { GetLocalTime(&mut time) };
        format!("{:02}:{:02}", time.hour, time.minute)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs()).unwrap_or(0);
        format!("{:02}:{:02}", seconds / 3600 % 24, seconds / 60 % 60)
    }
}

struct ChatEntry {
    /// `HH:MM` when the message arrived.
    time: String,
    name: String,
    text: String,
    system: bool,
    /// How many identical consecutive system notices were folded into this one.
    repeat: u32,
}

pub struct FriendsApp {
    settings: Settings,
    mesh: Option<Mesh>,
    peers: Vec<PeerView>,
    network_error: Option<String>,
    friend_ip: String,
    friend_ip_error: Option<String>,
    nickname_draft: String,
    draft: String,
    messages: Vec<ChatEntry>,
    voice: Option<VoiceSession>,
    video: Option<VideoSession>,
    video_textures: HashMap<(u64, StreamKind), egui::TextureHandle>,
    video_seen: HashMap<(u64, StreamKind), Instant>,
    mic_enabled: Arc<AtomicBool>,
    push_to_talk: Arc<AtomicBool>,
    ptt_key: Arc<AtomicI32>,
    noise_gate: Arc<AtomicBool>,
    gate_threshold: Arc<AtomicU32>,
    camera_index: Arc<AtomicU32>,
    video_quality: Arc<AtomicU32>,
    deafened: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    input_devices: Vec<String>,
    output_devices: Vec<String>,
    settings_open: bool,
    settings_page: usize,
    sidebar_open: bool,
    focus_mode: bool,
    chat_search_open: bool,
    chat_search_focus: bool,
    chat_search: String,
    chat_unread: usize,
    voice_started_at: Option<Instant>,
    last_notice: Option<(String, Instant)>,
    add_friend_open: bool,
    /// While `Some`, the next key press becomes the Push-to-Talk key. Holds keys that were already down.
    ptt_capture: Option<HashSet<i32>>,
    camera_stall_reported: bool,
    screen: Option<ScreenShare>,
    share_picker_open: bool,
    share_sources: Vec<ShareSource>,
    share_tab: usize,
    share_selected: Option<usize>,
    share_quality: ShareQuality,
    /// (when sampled, bytes at that time, megabits per second)
    share_rate: (Instant, u64, f32),
    /// Tile the user pinned as the big one.
    spotlight: Option<(u64, StreamKind)>,
    /// Tile shown on the whole window ("full view"), optionally on the whole monitor.
    fullview: Option<(u64, StreamKind)>,
    fullview_zoom: f32,
    fullview_pan: egui::Vec2,
    fullview_missing_since: Option<Instant>,
    last_pointer_move: Instant,
    sounds: SoundPlayer,
    window_focused: bool,
    /// A chat message arrived while the window was in the background.
    attention_wanted: bool,
    /// States seen on the previous frame, to play a sound exactly when one changes.
    prev_mic: bool,
    prev_deaf: bool,
    prev_camera: bool,
    prev_sharing: bool,
    prev_voice_peers: HashSet<u64>,
    ip_copied_at: Option<Instant>,
    /// Cameras reported by Windows as `(index, name)`; filled by a background thread.
    cameras: Arc<Mutex<Vec<(u32, String)>>>,
    cameras_loading: Arc<AtomicBool>,
}

impl Default for FriendsApp {
    fn default() -> Self {
        let settings = load_settings();
        i18n::set_language(Lang::from_code(&settings.language));
        let saved_ptt = settings.push_to_talk;
        let saved_ptt_key = settings.ptt_key;
        let saved_camera_index = settings.camera_index.min(MAX_CAMERA_INDEX);
        let saved_video_quality = settings.video_quality.min(5);
        let saved_noise_gate = settings.noise_gate;
        let saved_nickname = settings.nickname.clone();
        let saved_output_device = settings.output_device.clone();
        let saved_sounds_enabled = settings.sounds_enabled;
        let saved_sounds_volume = settings.sounds_volume.clamp(0.0, 1.0);
        let saved_share_height = settings.share_height;
        let saved_share_fps = settings.share_fps;
        let saved_gate_threshold = settings.gate_threshold.clamp(0.003, 0.08);
        let seeds = settings.known_ips.iter().filter_map(|ip| parse_known_ip(ip)).collect();
        let (mesh, network_error) = match Mesh::start(settings.nickname.clone(), seeds) {
            Ok(mesh) => (Some(mesh), None),
            Err(error) => (None, Some(error)),
        };
        let app = Self {
            volume: Arc::new(AtomicU32::new(settings.volume.clamp(0.0, 2.0).to_bits())),
            settings,
            mesh,
            peers: vec![],
            network_error,
            friend_ip: String::new(),
            friend_ip_error: None,
            nickname_draft: saved_nickname,
            draft: String::new(),
            messages: vec![ChatEntry {
                time: clock_now(),
                name: String::new(),
                text: i18n::english("This is the room chat: everyone on the network can see it. Messages are not saved when you close the app.").into(),
                system: true,
                repeat: 1,
            }],
            voice: None,
            video: None,
            video_textures: HashMap::new(),
            video_seen: HashMap::new(),
            mic_enabled: Arc::new(AtomicBool::new(true)),
            push_to_talk: Arc::new(AtomicBool::new(saved_ptt)),
            ptt_key: Arc::new(AtomicI32::new(saved_ptt_key)),
            noise_gate: Arc::new(AtomicBool::new(saved_noise_gate)),
            gate_threshold: Arc::new(AtomicU32::new(saved_gate_threshold.to_bits())),
            camera_index: Arc::new(AtomicU32::new(saved_camera_index)),
            video_quality: Arc::new(AtomicU32::new(saved_video_quality)),
            deafened: Arc::new(AtomicBool::new(false)),
            input_devices: voice::list_input_devices(),
            output_devices: voice::list_output_devices(),
            settings_open: false,
            settings_page: 1,
            sidebar_open: true,
            focus_mode: false,
            chat_search_open: false,
            chat_search_focus: false,
            chat_search: String::new(),
            chat_unread: 0,
            voice_started_at: None,
            last_notice: None,
            add_friend_open: false,
            ptt_capture: None,
            camera_stall_reported: false,
            screen: None,
            share_picker_open: false,
            share_sources: Vec::new(),
            share_tab: 0,
            share_selected: None,
            share_quality: ShareQuality {
                max_height: if screen::RESOLUTIONS.contains(&saved_share_height) { saved_share_height } else { 1080 },
                fps: if screen::FRAME_RATES.contains(&saved_share_fps) { saved_share_fps } else { 30 },
            },
            share_rate: (Instant::now(), 0, 0.0),
            spotlight: None,
            fullview: None,
            fullview_zoom: 1.0,
            fullview_pan: egui::Vec2::ZERO,
            fullview_missing_since: None,
            last_pointer_move: Instant::now(),
            window_focused: true,
            attention_wanted: false,
            sounds: SoundPlayer::new(&saved_output_device, saved_sounds_enabled, saved_sounds_volume),
            prev_mic: true,
            prev_deaf: false,
            prev_camera: false,
            prev_sharing: false,
            prev_voice_peers: HashSet::new(),
            ip_copied_at: None,
            cameras: Arc::new(Mutex::new(Vec::new())),
            cameras_loading: Arc::new(AtomicBool::new(false)),
        };
        app.refresh_cameras();
        app
    }
}

impl FriendsApp {
    fn notice(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.last_notice = Some((text.clone(), Instant::now()));
        // Fold identical consecutive notices instead of flooding the chat.
        if let Some(last) = self.messages.last_mut() {
            if last.system && last.text == text {
                last.repeat = last.repeat.saturating_add(1);
                return;
            }
        }
        self.messages.push(ChatEntry { time: clock_now(), name: String::new(), text, system: true, repeat: 1 });
        if self.messages.len() > 600 { self.messages.remove(0); }
    }

    fn remember_ip(&mut self, address: SocketAddr) {
        let ip = address.ip().to_string();
        if !self.settings.known_ips.contains(&ip) && self.settings.known_ips.len() < 32 {
            self.settings.known_ips.push(ip);
            save_settings(&self.settings);
        }
    }

    fn poll_events(&mut self) {
        let mut events = Vec::new();
        if let Some(mesh) = &self.mesh {
            while let Ok(event) = mesh.events.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                MeshEvent::Peers(peers) => {
                    let in_voice_now: HashSet<u64> = peers.iter().filter(|p| p.voice).map(|p| p.id).collect();
                    // Only tell the user about people arriving or leaving while they are in the call themselves.
                    if self.voice.is_some() && !self.deafened.load(Ordering::Relaxed) {
                        if in_voice_now.difference(&self.prev_voice_peers).next().is_some() {
                            self.sounds.play(Sound::PeerJoin);
                        } else if self.prev_voice_peers.difference(&in_voice_now).next().is_some() {
                            self.sounds.play(Sound::PeerLeave);
                        }
                    }
                    self.prev_voice_peers = in_voice_now;
                    self.peers = peers;
                }
                MeshEvent::Notice(text) => self.notice(text),
                MeshEvent::Chat { name, text } => {
                    if self.focus_mode || self.fullview.is_some() {self.chat_unread=self.chat_unread.saturating_add(1);}
                    // Somebody else wrote while the chat is out of sight: tell the user.
                    let chat_hidden = self.focus_mode || self.fullview.is_some() || !self.window_focused;
                    if name != self.settings.nickname && chat_hidden && !self.deafened.load(Ordering::Relaxed) {
                        self.sounds.play(Sound::Message);
                        if !self.window_focused { self.attention_wanted = true; }
                    }
                    self.messages.push(ChatEntry { time: clock_now(), name, text, system: false, repeat: 1 });
                    if self.messages.len() > 600 { self.messages.remove(0); }
                }
                MeshEvent::LearnedAddress(address) => self.remember_ip(address),
            }
        }
    }

    fn add_friend(&mut self) {
        let text = self.friend_ip.trim().to_string();
        match parse_known_ip(&text) {
            Some(address) => {
                self.remember_ip(address);
                if let Some(mesh) = &self.mesh {
                    mesh.add_peer(address);
                }
                self.friend_ip.clear();
                self.friend_ip_error = None;
                self.notice(trf("Looking for a friend at {address}. They will appear in the list as soon as they answer.", &[("address", &text)]));
            }
            None => {
                self.friend_ip_error = Some(if text.is_empty() {
                    tr("Enter your friend's address, for example 26.12.34.56").to_string()
                } else {
                    trf("“{text}” does not look like a Radmin VPN address. It looks like 26.12.34.56", &[("text", &text)])
                });
            }
        }
    }

    fn retry_network(&mut self) {
        let seeds = self.settings.known_ips.iter().filter_map(|ip| parse_known_ip(ip)).collect();
        match Mesh::start(self.settings.nickname.clone(), seeds) {
            Ok(mesh) => {
                self.mesh = Some(mesh);
                self.network_error = None;
                self.notice(tr("Network started."));
            }
            Err(error) => self.network_error = Some(error),
        }
    }

    fn send_chat(&mut self) {
        let text = self.draft.trim().to_string();
        if text.is_empty() { return; }
        if text.chars().count() > 1000 {
            self.notice(tr("A message cannot be longer than 1000 characters."));
            return;
        }
        if let Some(mesh) = &self.mesh {
            mesh.send_chat(text);
            self.draft.clear();
        }
    }

    fn toggle_voice(&mut self) {
        if self.voice.is_some() {
            self.stop_screen_share(None);
            self.fullview = None;
            self.sounds.play(Sound::LeaveSelf);
            self.video.take();
            self.video_textures.clear();
            self.video_seen.clear();
            self.voice.take();
            self.voice_started_at = None;
            if let Some(mesh) = &self.mesh { mesh.voice(false); }
            self.notice(tr("You left the voice channel."));
            return;
        }
        let Some(mesh) = &self.mesh else { return; };
        match VoiceSession::start(
            mesh.id,
            Arc::clone(&mesh.voice_targets),
            &self.settings.input_device,
            &self.settings.output_device,
            Arc::clone(&self.mic_enabled),
            Arc::clone(&self.push_to_talk),
            Arc::clone(&self.ptt_key),
            Arc::clone(&self.noise_gate),
            Arc::clone(&self.gate_threshold),
            Arc::clone(&self.deafened),
            Arc::clone(&self.volume),
        ) {
            Ok(voice) => {
                self.voice = Some(voice);
                self.sounds.play(Sound::JoinSelf);
                self.voice_started_at = Some(Instant::now());
                mesh.voice(true);
                // A camera preview opened from Settings has no send targets; replace
                // it with a real session and carry the camera state over.
                let camera_was_on = self.video.as_ref().is_some_and(|v| v.enabled());
                self.video = None;
                match VideoSession::start(
                    mesh.id, Arc::clone(&mesh.voice_targets),
                    Arc::clone(&self.camera_index), Arc::clone(&self.video_quality),
                ) {
                    Ok(mut video) => {
                        if camera_was_on { video.enable(); }
                        self.video = Some(video);
                    }
                    Err(error) => self.notice(trf("Video is unavailable: {error}. Voice still works.", &[("error", &error)])),
                }
                self.notice(tr("Voice is connected directly to your friends. You can turn the camera on separately."));
            }
            Err(error) => {
                self.notice(trf("Could not join voice: {error}. Check your microphone and headphones in the sound settings.", &[("error", &error)]));
                self.settings_open = true;
                self.settings_page = 1;
            }
        }
    }

    /// Makes sure a video session exists. Outside a call it is a private preview:
    /// it has no send targets, so nothing leaves this computer.
    fn ensure_video(&mut self) -> bool {
        if self.video.is_some() { return true; }
        let Some(mesh) = &self.mesh else {
            self.notice(tr("No network connection, the camera is unavailable."));
            return false;
        };
        let targets = if self.voice.is_some() {
            Arc::clone(&mesh.voice_targets)
        } else {
            Arc::new(Mutex::new(Vec::new()))
        };
        let id = mesh.id;
        match VideoSession::start(id, targets,
            Arc::clone(&self.camera_index), Arc::clone(&self.video_quality)) {
            Ok(video) => { self.video = Some(video); true }
            Err(error) => {
                self.notice(trf("Video is unavailable: {error}", &[("error", &error)]));
                false
            }
        }
    }

    fn toggle_camera(&mut self) {
        if !self.ensure_video() { return; }
        let in_call = self.voice.is_some();
        let local_id = self.mesh.as_ref().map(|mesh| mesh.id);
        let Some(video) = self.video.as_mut() else { return; };
        if video.enabled() {
            video.disable();
            if let Some(id) = local_id {
                self.video_textures.remove(&(id, StreamKind::Camera));
                self.video_seen.remove(&(id, StreamKind::Camera));
            }
            self.notice(tr("Camera turned off."));
        } else {
            video.enable();
            self.notice(if in_call { tr("Starting the camera. Windows may ask for permission.") }
                else { tr("Starting the camera for preview. Your friends cannot see it.") });
        }
    }

    /// Opens the "what do you want to show" window with a fresh list of screens and windows.
    fn open_share_picker(&mut self) {
        if self.voice.is_none() {
            self.notice(tr("To share your screen, join the voice channel first."));
            return;
        }
        self.share_sources = screen::list_sources();
        self.share_tab = 0;
        self.share_selected = self.share_sources.iter().position(|s| !s.is_window());
        self.share_picker_open = true;
    }

    fn start_screen_share(&mut self) {
        let Some(source) = self.share_selected.and_then(|i| self.share_sources.get(i)).cloned() else { return; };
        if !self.ensure_video() { return; }
        let Some(video) = self.video.as_ref() else { return; };
        let (hub, events, local_id) = (video.hub(), video.events_sender(), video.local_id());
        self.settings.share_height = self.share_quality.max_height;
        self.settings.share_fps = self.share_quality.fps;
        save_settings(&self.settings);
        match ScreenShare::start(&source, self.share_quality, hub, events, local_id) {
            Ok(share) => {
                self.screen = Some(share);
                self.share_picker_open = false;
                self.share_rate = (Instant::now(), 0, 0.0);
                self.notice(trf("You are sharing: {title}.", &[("title", &source.title)]));
            }
            Err(error) => self.notice(error),
        }
    }

    fn stop_screen_share(&mut self, reason: Option<String>) {
        if self.screen.take().is_none() { return; }
        if let Some(mesh) = &self.mesh {
            self.video_textures.remove(&(mesh.id, StreamKind::Screen));
            self.video_seen.remove(&(mesh.id, StreamKind::Screen));
        }
        self.notice(reason.unwrap_or_else(|| tr("Screen sharing stopped.").into()));
    }

    /// Notices a share that ended by itself and refreshes the bit-rate readout.
    fn watch_screen_share(&mut self) {
        let Some(share) = self.screen.as_ref() else { return; };
        if !share.is_running() {
            let reason = share.end_reason().unwrap_or_else(|| tr("Screen sharing stopped.").into());
            self.stop_screen_share(Some(reason));
            return;
        }
        let bytes = share.stats.bytes.load(Ordering::Relaxed);
        let (since, before, _) = self.share_rate;
        let seconds = since.elapsed().as_secs_f32();
        if seconds >= 1.0 {
            let mbps = (bytes.saturating_sub(before)) as f32 * 8.0 / seconds / 1_000_000.0;
            self.share_rate = (Instant::now(), bytes, mbps);
        }
    }

    /// Plays a sound at the moment mic, sound, camera or screen sharing changes,
    /// no matter what caused the change (button, Ctrl+M or the global F9 key).
    fn track_sounds(&mut self) {
        let mic = self.mic_enabled.load(Ordering::Relaxed);
        let deaf = self.deafened.load(Ordering::Relaxed);
        let camera = self.video.as_ref().is_some_and(|v| v.enabled());
        let sharing = self.screen.is_some();
        if self.voice.is_some() {
            if deaf != self.prev_deaf {
                self.sounds.play(if deaf { Sound::DeafenOn } else { Sound::DeafenOff });
            } else if mic != self.prev_mic {
                self.sounds.play(if mic { Sound::MicOn } else { Sound::MicOff });
            }
        }
        if camera != self.prev_camera {
            self.sounds.play(if camera { Sound::CameraOn } else { Sound::CameraOff });
        }
        if sharing != self.prev_sharing {
            self.sounds.play(if sharing { Sound::ScreenOn } else { Sound::ScreenOff });
        }
        self.prev_mic = mic;
        self.prev_deaf = deaf;
        self.prev_camera = camera;
        self.prev_sharing = sharing;
    }

    /// Shows one stream on the whole window.
    fn open_fullview(&mut self, key: (u64, StreamKind)) {
        self.fullview = Some(key);
        self.fullview_zoom = 1.0;
        self.fullview_pan = egui::Vec2::ZERO;
        self.fullview_missing_since = None;
        self.last_pointer_move = Instant::now();
    }

    fn close_fullview(&mut self, ctx: &egui::Context) {
        self.fullview = None;
        if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
    }

    /// Switches the interface language immediately and remembers it.
    fn set_language(&mut self, lang: Lang) {
        if i18n::language() == lang { return; }
        i18n::set_language(lang);
        self.settings.language = lang.code().to_string();
        // The default name follows the language; a name the user chose is never touched.
        if ["Friend", "Друг"].contains(&self.settings.nickname.as_str()) {
            let name = tr("Friend").to_string();
            self.settings.nickname = name.clone();
            self.nickname_draft = name.clone();
            if let Some(mesh) = &self.mesh { mesh.rename(name); }
        }
        save_settings(&self.settings);
    }

    /// Remembers the window size (restored on the next start) and applies the interface scale.
    fn track_window(&mut self, ctx: &egui::Context) {
        let scale = self.settings.ui_scale.clamp(0.8, 2.0);
        if (ctx.zoom_factor() - scale).abs() > 0.001 { ctx.set_zoom_factor(scale); }
        let (rect, maximized, fullscreen, focused) = ctx.input(|i| {
            let viewport = i.viewport();
            (viewport.inner_rect, viewport.maximized.unwrap_or(false),
             viewport.fullscreen.unwrap_or(false), viewport.focused.unwrap_or(true))
        });
        self.window_focused = focused;
        self.settings.window_maximized = maximized;
        // The size of a maximised or full-screen window is not the size to come back to.
        if let (Some(rect), false, false) = (rect, maximized, fullscreen) {
            self.settings.window_width = rect.width() * scale;
            self.settings.window_height = rect.height() * scale;
        }
    }

    fn start_ptt_capture(&mut self) {
        self.ptt_capture = Some(voice::pressed_keys().into_iter().collect());
    }

    /// Waits for a new key or mouse button and stores it as the Push-to-Talk key.
    fn poll_ptt_capture(&mut self) {
        let Some(held) = self.ptt_capture.as_mut() else { return; };
        let down: HashSet<i32> = voice::pressed_keys().into_iter().collect();
        // Keys held when capture started only count again after being released.
        held.retain(|vk| down.contains(vk));
        let Some(vk) = down.iter().copied().find(|vk| !held.contains(vk)) else { return; };
        self.ptt_capture = None;
        if vk == 0x1B { return; } // Esc cancels
        if vk == voice::MUTE_VK {
            self.notice(tr("F9 already toggles the microphone. Choose another key."));
            return;
        }
        self.ptt_key.store(vk, Ordering::Relaxed);
        self.settings.ptt_key = vk;
        save_settings(&self.settings);
        self.notice(trf("Push-to-Talk key: {key}.", &[("key", &voice::vk_name(vk))]));
    }

    fn warn_if_camera_stalled(&mut self) {
        let stalled = self.video.as_ref().is_some_and(|v| v.stalled());
        if stalled && !self.camera_stall_reported {
            self.camera_stall_reported = true;
            self.notice(tr("The camera is on but no frames arrive. Try another camera or check the permission in Windows settings."));
        } else if !stalled && self.video.as_ref().is_none_or(|v| !v.starting()) {
            self.camera_stall_reported = false;
        }
    }

    fn refresh_cameras(&self) {
        if self.cameras_loading.swap(true, Ordering::AcqRel) { return; }
        let cameras = Arc::clone(&self.cameras);
        let loading = Arc::clone(&self.cameras_loading);
        std::thread::spawn(move || {
            let found = video::list_cameras();
            if let Ok(mut shared) = cameras.lock() { *shared = found; }
            loading.store(false, Ordering::Release);
        });
    }

    /// The preview camera must never stay on unnoticed: switch it off as soon as
    /// the user is neither in a call nor looking at the camera settings.
    fn stop_orphan_preview(&mut self) {
        let watching = self.settings_open && self.settings_page == 2;
        if self.voice.is_none() && !watching && self.video.is_some() {
            self.video = None;
            self.video_textures.clear();
            self.video_seen.clear();
        }
    }
    fn update_camera_device(&mut self, index: u32) {
        let index = index.min(MAX_CAMERA_INDEX);
        self.camera_index.store(index, Ordering::Relaxed);
        self.settings.camera_index = index;
        save_settings(&self.settings);
        if let Some(video) = self.video.as_mut() {
            if video.enabled() {
                video.disable();
                video.enable();
                self.video_textures.clear();
                self.video_seen.clear();
            }
        }
    }
    fn update_video_quality(&mut self, quality: u32) {
        let quality = quality.min(5);
        self.video_quality.store(quality, Ordering::Relaxed);
        self.settings.video_quality = quality;
        save_settings(&self.settings);
        // The camera is asked for a different resolution in the HD modes, so restart it.
        if let Some(video) = self.video.as_mut() {
            if video.enabled() {
                video.disable();
                video.enable();
            }
        }
    }

    fn poll_video(&mut self, ctx: &egui::Context) {
        let Some(video) = self.video.as_mut() else { return; };
        let events = video.poll_events();
        let camera_on = video.enabled();
        let local_id = self.mesh.as_ref().map(|mesh| mesh.id);
        // A burst can contain several queued frames per user. Upload only the
        // latest frame to the GPU once per UI tick rather than all old frames.
        let mut latest = HashMap::new();
        for event in events {
            match event {
                VideoEvent::Frame(frame) => {
                    if Some(frame.sender_id) == local_id && frame.kind == StreamKind::Camera && !camera_on {
                        continue; // discard queued previews after switching camera off
                    }
                    latest.insert((frame.sender_id, frame.kind), frame);
                }
                VideoEvent::Error(message) => self.notice(message),
            }
        }
        for (key, mut frame) in latest {
            // Your own camera preview is mirrored, like looking into a mirror.
            // What friends receive is not mirrored.
            if Some(frame.sender_id) == local_id && frame.kind == StreamKind::Camera {
                mirror_horizontally(&mut frame.rgba, frame.width, 4);
            }
            let image = egui::ColorImage::from_rgba_premultiplied([frame.width, frame.height], &frame.rgba);
            if let Some(texture) = self.video_textures.get_mut(&key) {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                let texture = ctx.load_texture(
                    format!("friends-video-{}-{:?}", frame.sender_id, frame.kind),
                    image,
                    egui::TextureOptions::LINEAR,
                );
                self.video_textures.insert(key, texture);
            }
            self.video_seen.insert(key, Instant::now());
        }
        self.video_seen.retain(|_, time| time.elapsed() < Duration::from_secs(2));
        self.video_textures.retain(|id, _| self.video_seen.contains_key(id));
    }

}

impl eframe::App for FriendsApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.track_window(ctx);
        if self.attention_wanted && !self.window_focused {
            // Makes the taskbar button flash until the window is focused.
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
        }
        self.attention_wanted = false;
        self.poll_events();
        self.stop_orphan_preview();
        self.poll_ptt_capture();
        self.warn_if_camera_stalled();
        self.watch_screen_share();
        self.track_sounds();
        self.poll_video(ctx);
        let shortcuts = ctx.input(|i| {
            let ctrl = i.modifiers.ctrl && !i.modifiers.alt;
            (
                ctrl && i.key_pressed(egui::Key::M),
                ctrl && i.key_pressed(egui::Key::B),
                ctrl && i.key_pressed(egui::Key::E),
                ctrl && i.key_pressed(egui::Key::F),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if shortcuts.0 && self.voice.is_some() {
            let mic = self.mic_enabled.load(Ordering::Relaxed);
            self.mic_enabled.store(!mic, Ordering::Relaxed);
        }
        if shortcuts.1 { self.sidebar_open = !self.sidebar_open; }
        if shortcuts.2 {
            self.focus_mode = !self.focus_mode;
            if !self.focus_mode {self.chat_unread=0;}
        }
        if shortcuts.3 {
            self.chat_search_open = true;
            self.chat_search_focus = true;
            self.focus_mode = false;
            self.chat_unread = 0;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            let on = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!on));
        }
        if shortcuts.4 && self.ptt_capture.is_none() {
            if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
                // First Esc leaves full screen, the next one leaves the full view.
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            } else if self.fullview.is_some() {
                self.fullview = None;
            } else if self.share_picker_open {
                self.share_picker_open = false;
            } else if self.chat_search_open {
                self.chat_search_open = false;
                self.chat_search_focus = false;
                self.chat_search.clear();
            } else if self.settings_open {
                self.settings_open = false;
            } else {
                self.focus_mode = false;
                self.chat_unread=0;
            }
        }
        // Lower idle CPU/GPU load, but repaint quickly while in a live call.
        // No network worker depends on the UI frame rate.
        let repaint_ms = if self.ptt_capture.is_some() {
            25
        } else if self.screen.is_some() || self.video_textures.keys().any(|k| k.1 == StreamKind::Screen) {
            33
        } else if self.video.as_ref().is_some_and(|v| v.enabled()) {
            60
        } else if self.voice.is_some() {
            70
        } else {
            180
        };
        ctx.request_repaint_after(Duration::from_millis(repaint_ms));
        self.render(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.video.take();
        self.voice.take();
        self.mesh.take();
        self.settings.volume = f32::from_bits(self.volume.load(Ordering::Relaxed));
        save_settings(&self.settings);
    }
}

/// The saved window size if it is sensible, otherwise the default. A window that was
/// closed on a bigger monitor must not open off-screen or tiny on a smaller one.
fn restored_window_size(width: f32, height: f32) -> [f32; 2] {
    const DEFAULT: [f32; 2] = [1400.0, 860.0];
    let ok = |v: f32, min: f32, max: f32| v.is_finite() && (min..=max).contains(&v);
    if ok(width, 960.0, 7680.0) && ok(height, 620.0, 4320.0) { [width, height] } else { DEFAULT }
}

fn main() -> eframe::Result<()> {
    diagnostics::install();
    let saved = load_settings();
    let [width, height] = restored_window_size(saved.window_width, saved.window_height);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([width, height])
            .with_min_inner_size([960.0, 620.0])
            .with_maximized(saved.window_maximized)
            .with_icon(branding::icon()),
        ..Default::default()
    };
    eframe::run_native("Friends Voice", options, Box::new(|cc| {
        ui::install_theme(&cc.egui_ctx);
        Ok(Box::new(FriendsApp::default()))
    }))
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
        fn mirror_swaps_pixels_in_each_row_only() {
            // Two rows of three pixels: [A B C] [D E F]
            let mut data: Vec<u8> = (0..18).collect();
            mirror_horizontally(&mut data, 3, 3);
            assert_eq!(&data[..9], &[6, 7, 8, 3, 4, 5, 0, 1, 2]);
            assert_eq!(&data[9..], &[15, 16, 17, 12, 13, 14, 9, 10, 11]);
            mirror_horizontally(&mut data, 3, 3);
            assert_eq!(data, (0..18).collect::<Vec<u8>>());
        }

    #[test]
    fn saved_window_size_is_used_only_when_it_is_sensible() {
        assert_eq!(restored_window_size(1600.0, 900.0), [1600.0, 900.0]);
        assert_eq!(restored_window_size(0.0, 0.0), [1400.0, 860.0], "never saved");
        assert_eq!(restored_window_size(300.0, 200.0), [1400.0, 860.0], "too small");
        assert_eq!(restored_window_size(50_000.0, 900.0), [1400.0, 860.0], "absurd");
        assert_eq!(restored_window_size(f32::NAN, 900.0), [1400.0, 860.0]);
    }

    #[test]
    fn settings_from_an_older_version_still_load() {
        // A file written before the window, language and scale options existed.
        let old = r#"{"nickname":"Олена","volume":1.0,"push_to_talk":true}"#;
        let settings: Settings = serde_json::from_str(old).expect("old settings must load");
        assert_eq!(settings.nickname, "Олена");
        assert!(settings.push_to_talk);
        assert_eq!(settings.language, "en", "English is the default");
        assert_eq!(settings.ui_scale, 1.0);
        assert_eq!(settings.window_width, 0.0);
    }
}
