#![windows_subsystem = "windows"]
mod mesh;
mod protocol;
mod voice;
mod video;
mod ui;

use eframe::egui::{self, Color32};
use mesh::{Mesh, MeshEvent, PeerView};
use protocol::{CONTROL_PORT, MAX_NODES};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::PathBuf,
    sync::{atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering}, Arc},
    time::{Duration, Instant},
};
use voice::VoiceSession;
use video::{VideoSession, VideoEvent};

const BACKGROUND: Color32 = Color32::from_rgb(19, 21, 37);
const SIDEBAR: Color32 = Color32::from_rgb(15, 17, 33);
const PURPLE: Color32 = Color32::from_rgb(139, 93, 255);
const MUTED: Color32 = Color32::from_rgb(152, 159, 178);
const GREEN: Color32 = Color32::from_rgb(65, 214, 160);
const RED: Color32 = Color32::from_rgb(245, 98, 115);

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
}

impl Default for FriendsApp {
    fn default() -> Self {
        let settings = load_settings();
        let saved_ptt = settings.push_to_talk;
        let saved_ptt_key = settings.ptt_key;
        let saved_camera_index = settings.camera_index.min(3);
        let saved_video_quality = settings.video_quality.min(2);
        let saved_noise_gate = settings.noise_gate;
        let saved_gate_threshold = settings.gate_threshold.clamp(0.003, 0.08);
        let seeds = settings.known_ips.iter().filter_map(|ip| parse_known_ip(ip)).collect();
        let (mesh, network_error) = match Mesh::start(settings.nickname.clone(), seeds) {
            Ok(mesh) => (Some(mesh), None),
            Err(error) => (None, Some(error)),
        };
        Self {
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
            settings_open: true,
            settings_page: 0,
        }
    }
}

impl FriendsApp {
    fn notice(&mut self, text: impl Into<String>) {
        self.messages.push(ChatEntry { name: String::new(), text: text.into(), system: true });
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
                    self.messages.push(ChatEntry { name, text, system: false });
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
                mesh.voice(true);
                match VideoSession::start(
                    mesh.id, Arc::clone(&mesh.voice_targets),
                    Arc::clone(&self.camera_index), Arc::clone(&self.video_quality),
                ) {
                    Ok(video) => self.video = Some(video),
                    Err(error) => self.notice(format!("Відео недоступне: {error}. Голос працює.")),
                }
                self.notice("Голос підключено напряму до друзів. Камеру можна вмикати окремо.");
            }
            Err(error) => self.notice(format!("Не вдалося запустити голос: {error}")),
        }
    }

    fn toggle_camera(&mut self) {
        match self.video.as_mut() {
            Some(video) if video.enabled() => {
                video.disable();
                if let Some(mesh) = &self.mesh {
                    self.video_textures.remove(&mesh.id);
                    self.video_seen.remove(&mesh.id);
                }
                self.notice("Камеру вимкнено.");
            }
            Some(video) => {
                video.enable();
                self.notice("Запуск камери… Windows може запросити дозвіл.");
            }
            None => self.notice("Спочатку зайди у голосовий канал."),
        }
    }
    fn update_camera_device(&mut self, index: u32) {
        let index = index.min(3);
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
        for frame in latest.into_values() {
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
        self.poll_video(ctx);
        if self.voice.is_some() && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::M)) {
            let mic = self.mic_enabled.load(Ordering::Relaxed);
            self.mic_enabled.store(!mic, Ordering::Relaxed);
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
        egui::SidePanel::left("sidebar").resizable(false).exact_width(291.0)
            .frame(egui::Frame::none().fill(SIDEBAR).inner_margin(14.0))
            .show(ctx, |ui| self.neon_sidebar(ui));
        if self.settings_open {
            egui::SidePanel::right("settings").resizable(false).default_width(290.0)
                .width_range(250.0..=330.0)
                .frame(egui::Frame::none().fill(SIDEBAR).inner_margin(13.0))
                .show(ctx, |ui| self.neon_settings(ui));
        }
        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(BACKGROUND).inner_margin(16.0)
        ).show(ctx, |ui| self.neon_main(ui));
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
            .with_inner_size([1520.0, 865.0])
            .with_min_inner_size([1060.0, 660.0]),
        ..Default::default()
    };
    eframe::run_native("Friends Voice 0.8 — Stability", options, Box::new(|cc| {
        let mut visual = egui::Visuals::dark();
        visual.panel_fill = BACKGROUND;
        visual.window_fill = BACKGROUND;
        visual.widgets.inactive.bg_fill = Color32::from_rgb(39, 44, 69);
        visual.widgets.hovered.bg_fill = Color32::from_rgb(67, 69, 108);
        visual.widgets.active.bg_fill = Color32::from_rgb(88, 79, 171);
        visual.selection.bg_fill = PURPLE.linear_multiply(0.45);
        cc.egui_ctx.set_visuals(visual);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(9.0, 8.0);
            style.spacing.button_padding = egui::vec2(11.0, 7.0);
            style.visuals.widgets.inactive.rounding = egui::Rounding::same(9.0);
            style.visuals.widgets.hovered.rounding = egui::Rounding::same(9.0);
            style.visuals.widgets.active.rounding = egui::Rounding::same(9.0);
        });
        Ok(Box::new(FriendsApp::default()))
    }))
}
