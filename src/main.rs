#![windows_subsystem = "windows"]
mod protocol;
mod voice;

use eframe::egui::{self, Color32, RichText};
use protocol::{Packet, UserInfo};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};
use voice::VoiceSession;

const BACKGROUND: Color32 = Color32::from_rgb(22, 24, 34);
const SIDEBAR: Color32 = Color32::from_rgb(17, 19, 29);
const ACCENT: Color32 = Color32::from_rgb(105, 118, 244);
const MUTED: Color32 = Color32::from_rgb(151, 158, 177);
const SUCCESS: Color32 = Color32::from_rgb(70, 213, 158);
const DANGER: Color32 = Color32::from_rgb(244, 102, 113);

enum Event {
    Connected { users: Vec<UserInfo>, token: u128, addr: SocketAddr },
    Packet(Packet),
    Disconnected(String),
}

struct ChatEntry {
    name: String,
    text: String,
    system: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct Settings {
    host: String,
    nickname: String,
    input_device: String,
    output_device: String,
    volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1:45888".into(),
            nickname: "Друг".into(),
            input_device: String::new(),
            output_device: String::new(),
            volume: 1.0,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|path| PathBuf::from(path).join("FriendsVoice").join("settings.json"))
}

fn read_settings() -> Settings {
    settings_path().and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn persist_settings(settings: &Settings) {
    let Some(path) = settings_path() else { return; };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() { return; }
    }
    if let Ok(data) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, data);
    }
}

fn network_worker(
    host: String,
    nickname: String,
    password: String,
    send_events: Sender<Event>,
    receive_packets: Receiver<Packet>,
    awake: Arc<AtomicBool>,
) {
    let addr = match host.to_socket_addrs().ok().and_then(|mut it| it.next()) {
        Some(addr) => addr,
        None => {
            let _ = send_events.send(Event::Disconnected("Неправильна адреса сервера".into()));
            return;
        }
    };
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(6)) {
        Ok(stream) => stream,
        Err(e) => {
            let _ = send_events.send(Event::Disconnected(format!("Не вдалося підключитися: {e}")));
            return;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let packet = Packet::Hello { nickname, password };
    if writeln!(stream, "{}", serde_json::to_string(&packet).unwrap()).is_err() {
        let _ = send_events.send(Event::Disconnected("Помилка авторизації".into()));
        return;
    }
    let read_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = send_events.send(Event::Disconnected(format!("Помилка сокета: {e}")));
            return;
        }
    };
    let mut reader = BufReader::new(read_stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        let _ = send_events.send(Event::Disconnected("Сервер закрив підключення".into()));
        return;
    }
    if line.len() > 8192 {
        let _ = send_events.send(Event::Disconnected("Завелика відповідь сервера".into()));
        return;
    }
    match serde_json::from_str::<Packet>(&line) {
        Ok(Packet::Welcome { users, voice_token, .. }) => {
            match u128::from_str_radix(&voice_token, 16) {
                Ok(token) => {
                    let _ = send_events.send(Event::Connected { users, token, addr });
                }
                Err(_) => {
                    let _ = send_events.send(Event::Disconnected("Недопустимий токен від сервера".into()));
                    return;
                }
            }
        }
        Ok(Packet::Error { text }) => {
            let _ = send_events.send(Event::Disconnected(text));
            return;
        }
        _ => {
            let _ = send_events.send(Event::Disconnected("Невідома відповідь сервера".into()));
            return;
        }
    }
    let write_events = send_events.clone();
    let write_awake = Arc::clone(&awake);
    thread::spawn(move || {
        while write_awake.load(Ordering::Relaxed) {
            match receive_packets.recv_timeout(Duration::from_millis(100)) {
                Ok(packet) => {
                    if writeln!(stream, "{}", serde_json::to_string(&packet).unwrap()).is_err() {
                        let _ = write_events.send(Event::Disconnected("Не вдалося надіслати повідомлення".into()));
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let _ = stream.shutdown(Shutdown::Both);
    });
    // A short read timeout on Windows may abort a quiet connection, even though it is alive.
    // Keep the established TCP socket in blocking mode. The writer thread explicitly shuts
    // down its clone when the user disconnects, so this read unblocks without polling timeouts.
    if let Err(error) = reader.get_ref().set_read_timeout(None) {
        let _ = send_events.send(Event::Disconnected(format!("Помилка TCP: {error}")));
        awake.store(false, Ordering::Relaxed);
        return;
    }
    let reason = loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break "Сервер закрив TCP-з'єднання".to_string(),
            Ok(n) if n > 8192 => break "Сервер надіслав завеликий пакет".to_string(),
            Ok(_) => {
                match serde_json::from_str::<Packet>(&line) {
                    Ok(packet) => {
                        let _ = send_events.send(Event::Packet(packet));
                    }
                    Err(error) => {
                        break format!("Некоректний пакет від сервера: {error}");
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => break format!("Помилка читання TCP: {error}"),
        }
    };
    if awake.swap(false, Ordering::Relaxed) {
        let _ = send_events.send(Event::Disconnected(reason));
    }
}

struct App {
    settings: Settings,
    password: String,
    draft: String,
    messages: Vec<ChatEntry>,
    users: Vec<UserInfo>,
    connecting: bool,
    connected: bool,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    outbound: Option<Sender<Packet>>,
    connection_alive: Option<Arc<AtomicBool>>,
    voice: Option<VoiceSession>,
    session: Option<(SocketAddr, u128)>,
    mic_enabled: Arc<AtomicBool>,
    deafened: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    input_devices: Vec<String>,
    output_devices: Vec<String>,
    settings_open: bool,
}

impl Default for App {
    fn default() -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        let settings = read_settings();
        let volume = Arc::new(AtomicU32::new(settings.volume.clamp(0.0, 2.0).to_bits()));
        Self {
            settings,
            password: String::new(),
            draft: String::new(),
            messages: vec![ChatEntry {
                name: String::new(),
                text: "Ласкаво просимо! Підключись до сервера, щоб спілкуватися з друзями.".into(),
                system: true,
            }],
            users: vec![],
            connecting: false,
            connected: false,
            events_tx,
            events_rx,
            outbound: None,
            connection_alive: None,
            voice: None,
            session: None,
            mic_enabled: Arc::new(AtomicBool::new(true)),
            deafened: Arc::new(AtomicBool::new(false)),
            volume,
            input_devices: voice::list_input_devices(),
            output_devices: voice::list_output_devices(),
            settings_open: false,
        }
    }
}

impl App {
    fn notice(&mut self, text: impl Into<String>) {
        self.messages.push(ChatEntry { name: String::new(), text: text.into(), system: true });
        if self.messages.len() > 500 { self.messages.remove(0); }
    }

    fn connect(&mut self) {
        if self.connected || self.connecting { return; }
        let host = self.settings.host.trim().to_string();
        let nickname = self.settings.nickname.trim().to_string();
        if host.is_empty() || self.password.len() < 8 || nickname.is_empty() || nickname.chars().count() > 24 {
            self.notice("Перевір адресу, нікнейм (1–24 символи) і пароль (від 8 символів).");
            return;
        }
        persist_settings(&self.settings);
        let (tx, rx) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        self.outbound = Some(tx);
        self.connection_alive = Some(Arc::clone(&alive));
        self.connecting = true;
        let events = self.events_tx.clone();
        let password = self.password.clone();
        thread::spawn(move || network_worker(host, nickname, password, events, rx, alive));
    }

    fn disconnect(&mut self) {
        if let Some(alive) = self.connection_alive.take() {
            alive.store(false, Ordering::Relaxed);
        }
        self.voice.take();
        self.outbound = None;
        self.connected = false;
        self.connecting = false;
        self.session = None;
        self.users.clear();
    }

    fn poll(&mut self) {
        while let Ok(event) = self.events_rx.try_recv() {
            match event {
                Event::Connected { users, token, addr } => {
                    if self.connection_alive.as_ref().is_none_or(|a| !a.load(Ordering::Relaxed)) {
                        continue;
                    }
                    self.users = users;
                    self.session = Some((addr, token));
                    self.connected = true;
                    self.connecting = false;
                    self.notice("З'єднання встановлено! Можна писати в чат або приєднатися до голосу.");
                }
                Event::Disconnected(text) => {
                    if self.connected || self.connecting {
                        self.disconnect();
                        self.notice(format!("Помилка / вихід: {text}"));
                    }
                }
                Event::Packet(packet) => match packet {
                    Packet::Users { users } => self.users = users,
                    Packet::Chat { from, text } => {
                        self.messages.push(ChatEntry { name: from, text, system: false });
                        if self.messages.len() > 500 { self.messages.remove(0); }
                    }
                    Packet::Notice { text } | Packet::Error { text } => self.notice(text),
                    _ => {}
                }
            }
        }
    }

    fn toggle_voice(&mut self) {
        if self.voice.is_some() {
            self.voice.take();
            self.notice("Ти залишив(-ла) голосовий канал.");
            return;
        }
        let Some((addr, token)) = self.session else { return; };
        match VoiceSession::start(
            addr, token, &self.settings.input_device, &self.settings.output_device,
            Arc::clone(&self.mic_enabled), Arc::clone(&self.deafened), Arc::clone(&self.volume),
        ) {
            Ok(voice) => {
                self.voice = Some(voice);
                self.notice("Голосовий канал підключено. Рекомендуємо використовувати навушники.");
            }
            Err(err) => self.notice(format!("Голос не запустився: {err}")),
        }
    }

    fn send_message(&mut self) {
        if !self.connected { return; }
        let text = self.draft.trim().to_owned();
        if text.is_empty() { return; }
        if text.chars().count() > 1000 {
            self.notice("Максимальна довжина повідомлення — 1000 символів.");
            return;
        }
        if let Some(outbound) = &self.outbound {
            if outbound.send(Packet::SendChat { text }).is_ok() {
                self.draft.clear();
            }
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("# загальний").size(21.0).color(Color32::WHITE));
            ui.label(RichText::new("Друзі · Чат · Голос").color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status = if self.connected { "● У мережі" } else if self.connecting { "◌ Підключення" } else { "○ Офлайн" };
                ui.label(RichText::new(status).color(if self.connected { SUCCESS } else { MUTED }));
                ui.separator();
                ui.label("Friends Voice 0.2");
            });
        });
        ui.add_space(8.0);
        ui.separator();
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.heading(RichText::new("◉ Friends Voice").size(20.0).color(Color32::WHITE));
        ui.label(RichText::new("PRIVATE SPACE / RADMIN VPN").size(10.0).color(MUTED));
        ui.add_space(20.0);
        ui.label(RichText::new("ПІДКЛЮЧЕННЯ").size(11.0).strong().color(MUTED));
        ui.add_space(6.0);
        ui.label("Адреса сервера");
        ui.add_enabled(!self.connected && !self.connecting,
            egui::TextEdit::singleline(&mut self.settings.host).desired_width(f32::INFINITY));
        ui.label("Нікнейм");
        ui.add_enabled(!self.connected && !self.connecting,
            egui::TextEdit::singleline(&mut self.settings.nickname).desired_width(f32::INFINITY));
        ui.label("Пароль кімнати");
        ui.add_enabled(!self.connected && !self.connecting,
            egui::TextEdit::singleline(&mut self.password).password(true).desired_width(f32::INFINITY));
        ui.add_space(10.0);
        if self.connected {
            if ui.add_sized([ui.available_width(), 30.0],
                egui::Button::new("Відключитися").fill(Color32::from_rgb(86, 43, 56))).clicked() {
                self.disconnect();
                self.notice("Відключено вручну.");
            }
        } else if ui.add_enabled_ui(!self.connecting, |ui| {
            ui.add_sized([ui.available_width(), 32.0],
                egui::Button::new("Підключитися →").fill(ACCENT))
        }).inner.clicked() {
            self.connect();
        }
        if self.connecting { ui.spinner(); }
        ui.add_space(18.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label(RichText::new("ГОЛОСОВІ КАНАЛИ").size(11.0).strong().color(MUTED));
        ui.add_space(6.0);
        let joined = self.voice.is_some();
        ui.label(RichText::new(if joined { "● Загальний · підключено" } else { "◌ Загальний" })
            .color(if joined { SUCCESS } else { Color32::LIGHT_GRAY }));
        if ui.add_enabled(self.connected, egui::Button::new(
            if joined { "Залишити голос" } else { "Приєднатися до голосу" }
        )).clicked() {
            self.toggle_voice();
        }
        ui.add_space(13.0);
        ui.separator();
        ui.label(RichText::new(format!("УЧАСНИКИ · {} / 5", self.users.len())).size(11.0).strong().color(MUTED));
        for user in &self.users {
            ui.horizontal(|ui| {
                ui.colored_label(SUCCESS, "●");
                ui.label(&user.nickname);
            });
        }
        ui.add_space(10.0);
        ui.separator();
        if ui.button("⚙ Налаштування звуку").clicked() {
            self.settings_open = !self.settings_open;
        }
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(12.0);
        ui.heading(RichText::new("Налаштування").size(18.0));
        ui.separator();
        ui.label(RichText::new("МІКРОФОН").size(11.0).color(MUTED));
        egui::ComboBox::from_id_salt("mic_device")
            .width(ui.available_width() - 8.0)
            .selected_text(if self.settings.input_device.is_empty() {
                "За замовчуванням" } else { &self.settings.input_device })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.input_device, String::new(), "За замовчуванням");
                for name in &self.input_devices {
                    ui.selectable_value(&mut self.settings.input_device, name.clone(), name);
                }
            });
        ui.add_space(10.0);
        ui.label(RichText::new("ДИНАМІКИ / НАВУШНИКИ").size(11.0).color(MUTED));
        egui::ComboBox::from_id_salt("speaker_device")
            .width(ui.available_width() - 8.0)
            .selected_text(if self.settings.output_device.is_empty() {
                "За замовчуванням" } else { &self.settings.output_device })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.output_device, String::new(), "За замовчуванням");
                for name in &self.output_devices {
                    ui.selectable_value(&mut self.settings.output_device, name.clone(), name);
                }
            });
        if ui.button("↻ Оновити список пристроїв").clicked() {
            self.input_devices = voice::list_input_devices();
            self.output_devices = voice::list_output_devices();
        }
        ui.add_space(10.0);
        let mut volume = f32::from_bits(self.volume.load(Ordering::Relaxed)).clamp(0.0, 2.0);
        if ui.add(egui::Slider::new(&mut volume, 0.0..=2.0).text("Гучність")).changed() {
            self.volume.store(volume.to_bits(), Ordering::Relaxed);
        }
        ui.add_space(12.0);
        ui.label(RichText::new("РІВЕНЬ МІКРОФОНА").size(11.0).color(MUTED));
        let level = self.voice.as_ref()
            .map(|v| f32::from_bits(v.microphone_level.load(Ordering::Relaxed)))
            .unwrap_or(0.0).clamp(0.0, 1.0);
        ui.add(egui::ProgressBar::new((level * 7.0).min(1.0)).desired_width(ui.available_width()));
        ui.add_space(7.0);
        ui.label(RichText::new("Після зміни аудіопристрою вийди з голосового каналу та зайди знову.")
            .size(11.0).color(MUTED));
        ui.add_space(9.0);
        if ui.button("Зберегти налаштування").clicked() {
            self.settings.volume = volume;
            persist_settings(&self.settings);
            self.notice("Налаштування збережено на цьому комп'ютері.");
        }
        ui.add_space(20.0);
        ui.separator();
        ui.label(RichText::new("ПОКИ НЕ ПІДТРИМУЄТЬСЯ").size(10.0).color(MUTED));
        ui.label(RichText::new("Відео й демонстрація екрана з'являться в майбутніх версіях.")
            .size(12.0).color(MUTED));
    }

    fn chat_panel(&mut self, ui: &mut egui::Ui) {
        self.header(ui);
        let chat_height = (ui.available_height() - 84.0).max(120.0);
        egui::ScrollArea::vertical()
            .max_height(chat_height)
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_height(chat_height);
                for message in &self.messages {
                    if message.system {
                        ui.add_space(5.0);
                        ui.label(RichText::new(format!("◆ {}", message.text)).size(12.0).color(MUTED));
                    } else {
                        ui.add_space(9.0);
                        let own = message.name == self.settings.nickname;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("●").color(if own { ACCENT } else { SUCCESS }));
                            ui.label(RichText::new(&message.name).strong().color(if own { ACCENT } else { Color32::WHITE }));
                        });
                        ui.indent(&message.name, |ui| {
                            ui.label(RichText::new(&message.text).size(14.0));
                        });
                    }
                }
            });
        ui.separator();
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            let send_width = (ui.available_width() - 100.0).max(120.0);
            let edit = ui.add_enabled(self.connected, egui::TextEdit::singleline(&mut self.draft)
                .hint_text(if self.connected { "Повідомлення для друзів…" } else { "Підключися, щоб написати…" })
                .desired_width(send_width));
            if self.connected && edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                self.send_message();
            }
            if ui.add_enabled(self.connected, egui::Button::new("Надіслати").fill(ACCENT)).clicked() {
                self.send_message();
            }
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            if self.voice.is_some() {
                let mic = self.mic_enabled.load(Ordering::Relaxed);
                if ui.add(egui::Button::new(if mic { "🎙 Мікрофон увімкнено" } else { "🔇 Мікрофон вимкнено" })
                    .fill(if mic { Color32::from_rgb(34, 100, 77) } else { DANGER })).clicked() {
                    self.mic_enabled.store(!mic, Ordering::Relaxed);
                }
                let deaf = self.deafened.load(Ordering::Relaxed);
                if ui.button(if deaf { "Увімкнути звук" } else { "Вимкнути звук" }).clicked() {
                    self.deafened.store(!deaf, Ordering::Relaxed);
                }
                ui.label(RichText::new("Ctrl+M — мікрофон").size(11.0).color(MUTED));
            } else {
                ui.label(RichText::new("🎧 Для голосу натисни «Приєднатися до голосу» ліворуч.")
                    .size(12.0).color(MUTED));
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll();
        if self.voice.is_some() && ctx.input(|input| input.modifiers.ctrl && input.key_pressed(egui::Key::M)) {
            let old = self.mic_enabled.load(Ordering::Relaxed);
            self.mic_enabled.store(!old, Ordering::Relaxed);
        }
        ctx.request_repaint_after(Duration::from_millis(85));
        egui::SidePanel::left("friends_sidebar")
            .resizable(false).exact_width(244.0)
            .frame(egui::Frame::none().fill(SIDEBAR).inner_margin(12.0))
            .show(ctx, |ui| self.sidebar(ui));
        if self.settings_open {
            egui::SidePanel::right("audio_settings")
                .resizable(false).exact_width(270.0)
                .frame(egui::Frame::none().fill(SIDEBAR).inner_margin(13.0))
                .show(ctx, |ui| self.settings_panel(ui));
        }
        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(BACKGROUND).inner_margin(17.0)
        ).show(ctx, |ui| self.chat_panel(ui));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.disconnect();
        self.settings.volume = f32::from_bits(self.volume.load(Ordering::Relaxed));
        persist_settings(&self.settings);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 710.0])
            .with_min_inner_size([810.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native("Friends Voice 0.2", options, Box::new(|cc| {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BACKGROUND;
        visuals.window_fill = BACKGROUND;
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(43, 47, 63);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(65, 69, 93);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 7.0);
        });
        Ok(Box::new(App::default()))
    }))
}
