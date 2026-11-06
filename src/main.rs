#![windows_subsystem = "windows"]
mod mesh;
mod protocol;
mod voice;
mod ui;

use eframe::egui::{self, Color32};
use mesh::{Mesh, MeshEvent, PeerView};
use protocol::{CONTROL_PORT, MAX_NODES};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::PathBuf,
    sync::{atomic::{AtomicBool, AtomicU32, Ordering}, Arc},
    time::Duration,
};
use voice::VoiceSession;

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
        let _ = fs::write(path, json);
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
    mic_enabled: Arc<AtomicBool>,
    push_to_talk: Arc<AtomicBool>,
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
            mic_enabled: Arc::new(AtomicBool::new(true)),
            push_to_talk: Arc::new(AtomicBool::new(settings.push_to_talk)),
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
        if self.voice.take().is_some() {
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
            Arc::clone(&self.deafened),
            Arc::clone(&self.volume),
        ) {
            Ok(voice) => {
                self.voice = Some(voice);
                mesh.voice(true);
                self.notice("Голос підключено напряму до друзів. Можна виходити з каналу незалежно одне від одного.");
            }
            Err(error) => self.notice(format!("Не вдалося запустити голос: {error}")),
        }
    }


}

impl eframe::App for FriendsApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll_events();
        if self.voice.is_some() && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::M)) {
            let mic = self.mic_enabled.load(Ordering::Relaxed);
            self.mic_enabled.store(!mic, Ordering::Relaxed);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
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
    eframe::run_native("Friends Voice 0.4 — Neon P2P", options, Box::new(|cc| {
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
