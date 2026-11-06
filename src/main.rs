#![windows_subsystem = "windows"]
mod mesh;
mod protocol;
mod voice;
mod video;
mod ui;
mod branding;

use eframe::egui::{self, Color32};
use mesh::{Mesh, MeshEvent, PeerView};
use protocol::{CONTROL_PORT, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::PathBuf,
    sync::{atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering}, Arc, Mutex},
    time::{Duration, Instant},
};
use voice::VoiceSession;
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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nickname: "Друг".into(),
            known_ips: vec![],
            input_device: String::new(),
            output_device: String::new(),
            volume: 1.0,
            push_to_talk: false,
            ptt_key: voice::PTT_VK,
            camera_index: 0,
            video_quality: 1,
            noise_gate: false, // Opt-in: never silently clip quiet microphones.
            gate_threshold: 0.018,
        }
    }
}

/// Flips tightly packed RGB rows left to right.
fn mirror_horizontally(rgb: &mut [u8], width: usize) {
    if width == 0 { return; }
    for row in rgb.chunks_exact_mut(width * 3) {
        for x in 0..width / 2 {
            let (left, right) = (x * 3, (width - 1 - x) * 3);
            for c in 0..3 { row.swap(left + c, right + c); }
        }
    }
}

#[cfg(test)]
mod mirror_tests {
    use super::mirror_horizontally;
    #[test]
    fn mirror_swaps_pixels_in_each_row_only() {
        // Two rows of three pixels: [A B C] [D E F]
        let mut data: Vec<u8> = (0..18).collect();
        mirror_horizontally(&mut data, 3);
        assert_eq!(&data[..9], &[6, 7, 8, 3, 4, 5, 0, 1, 2]);
        assert_eq!(&data[9..], &[15, 16, 17, 12, 13, 14, 9, 10, 11]);
        mirror_horizontally(&mut data, 3);
        assert_eq!(data, (0..18).collect::<Vec<u8>>());
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
        if fs::write(&tmp, &json).is_ok() {
            if fs::rename(&tmp, &path).is_err() {
                let _ = fs::write(&path, json);
                let _ = fs::remove_file(&tmp);
            }
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

struct ChatEntry {
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
    draft: String,
    messages: Vec<ChatEntry>,
    voice: Option<VoiceSession>,
    video: Option<VideoSession>,
    video_textures: HashMap<u64, egui::TextureHandle>,
    video_seen: HashMap<u64, Instant>,
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
    ip_copied_at: Option<Instant>,
    /// Cameras reported by Windows as `(index, name)`; filled by a background thread.
    cameras: Arc<Mutex<Vec<(u32, String)>>>,
    cameras_loading: Arc<AtomicBool>,
}

impl Default for FriendsApp {
    fn default() -> Self {
        let settings = load_settings();
        let saved_ptt = settings.push_to_talk;
        let saved_ptt_key = settings.ptt_key;
        let saved_camera_index = settings.camera_index.min(MAX_CAMERA_INDEX);
        let saved_video_quality = settings.video_quality.min(2);
        let saved_noise_gate = settings.noise_gate;
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
            draft: String::new(),
            messages: vec![ChatEntry {
                name: String::new(),
                text: "Friends Voice працює без сервера! Друзі з однієї мережі Radmin VPN можуть знайти одне одного автоматично. Якщо ні — додай IP друга один раз.".into(),
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
            settings_page: 0,
            sidebar_open: true,
            focus_mode: false,
            chat_search_open: false,
            chat_search_focus: false,
            chat_search: String::new(),
            chat_unread: 0,
            voice_started_at: None,
            last_notice: None,
            add_friend_open: false,
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
        self.messages.push(ChatEntry { name: String::new(), text, system: true, repeat: 1 });
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
                MeshEvent::Peers(peers) => self.peers = peers,
                MeshEvent::Notice(text) => self.notice(text),
                MeshEvent::Chat { name, text } => {
                    if self.focus_mode {self.chat_unread=self.chat_unread.saturating_add(1);}
                    self.messages.push(ChatEntry { name, text, system: false, repeat: 1 });
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
                self.notice(format!("Пробуємо знайти друга за адресою {text}."));
            }
            None => self.notice("Введи IPv4-адресу друга з Radmin VPN, наприклад 26.12.34.56."),
        }
    }

    fn retry_network(&mut self) {
        let seeds = self.settings.known_ips.iter().filter_map(|ip| parse_known_ip(ip)).collect();
        match Mesh::start(self.settings.nickname.clone(), seeds) {
            Ok(mesh) => {
                self.mesh = Some(mesh);
                self.network_error = None;
                self.notice("P2P-мережу запущено.");
            }
            Err(error) => self.network_error = Some(error),
        }
    }

    fn send_chat(&mut self) {
        let text = self.draft.trim().to_string();
        if text.is_empty() { return; }
        if text.chars().count() > 1000 {
            self.notice("Повідомлення не може бути довшим за 1000 символів.");
            return;
        }
        if let Some(mesh) = &self.mesh {
            mesh.send_chat(text);
            self.draft.clear();
        }
    }

    fn toggle_voice(&mut self) {
        if self.voice.is_some() {
            self.video.take();
            self.video_textures.clear();
            self.video_seen.clear();
            self.voice.take();
            self.voice_started_at = None;
            if let Some(mesh) = &self.mesh { mesh.voice(false); }
            self.notice("Ти вийшов(-ла) з голосового каналу.");
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
                    Err(error) => self.notice(format!("Відео недоступне: {error}. Голос працює.")),
                }
                self.notice("Голос підключено напряму до друзів. Камеру можна вмикати окремо.");
            }
            Err(error) => self.notice(format!("Не вдалося запустити голос: {error}")),
        }
    }

    /// Makes sure a video session exists. Outside a call it is a private preview:
    /// it has no send targets, so nothing leaves this computer.
    fn ensure_video(&mut self) -> bool {
        if self.video.is_some() { return true; }
        let Some(mesh) = &self.mesh else {
            self.notice("Немає з'єднання з мережею, камера недоступна.");
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
                self.notice(format!("Відео недоступне: {error}"));
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
                self.video_textures.remove(&id);
                self.video_seen.remove(&id);
            }
            self.notice("Камеру вимкнено.");
        } else {
            video.enable();
            self.notice(if in_call { "Запуск камери. Windows може попросити дозвіл." }
                else { "Запуск камери для перегляду. Друзі її не бачать." });
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
        let watching = self.settings_open && self.settings_page == 1;
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
        let quality = quality.min(2);
        self.video_quality.store(quality, Ordering::Relaxed);
        self.settings.video_quality = quality;
        save_settings(&self.settings);
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
                    if Some(frame.sender_id) == local_id && !camera_on {
                        continue; // discard queued previews after switching camera off
                    }
                    latest.insert(frame.sender_id, frame);
                }
                VideoEvent::Error(message) => self.notice(message),
            }
        }
        for mut frame in latest.into_values() {
            // Your own preview is mirrored, like looking into a mirror. What is
            // sent to friends is not mirrored.
            if Some(frame.sender_id) == local_id {
                mirror_horizontally(&mut frame.rgb, frame.width);
            }
            let image = egui::ColorImage::from_rgb([frame.width, frame.height], &frame.rgb);
            if let Some(texture) = self.video_textures.get_mut(&frame.sender_id) {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                let texture = ctx.load_texture(
                    format!("friends-camera-{}", frame.sender_id),
                    image,
                    egui::TextureOptions::LINEAR,
                );
                self.video_textures.insert(frame.sender_id, texture);
            }
            self.video_seen.insert(frame.sender_id, Instant::now());
        }
        self.video_seen.retain(|_, time| time.elapsed() < Duration::from_secs(2));
        self.video_textures.retain(|id, _| self.video_seen.contains_key(id));
    }

}

impl eframe::App for FriendsApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll_events();
        self.stop_orphan_preview();
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
        if shortcuts.4 {
            if self.chat_search_open {
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
        let repaint_ms = if self.video.as_ref().is_some_and(|v| v.enabled()) {
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

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([960.0, 620.0])
            .with_icon(branding::icon()),
        ..Default::default()
    };
    eframe::run_native("Friends Voice", options, Box::new(|cc| {
        ui::install_theme(&cc.egui_ctx);
        Ok(Box::new(FriendsApp::default()))
    }))
}
