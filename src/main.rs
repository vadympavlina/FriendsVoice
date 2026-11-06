#![windows_subsystem = "windows"]
mod protocol;
use protocol::Packet;
use eframe::egui;
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

enum Event {
    Connected,
    Packet(Packet),
    Disconnected(String),
}

struct App {
    host: String,
    nickname: String,
    password: String,
    draft: String,
    connected: bool,
    connecting: bool,
    users: Vec<String>,
    messages: Vec<String>,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
    outbound: Option<Sender<Packet>>,
}

impl Default for App {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            host: "127.0.0.1:45888".into(),
            nickname: "Друг".into(),
            password: String::new(),
            draft: String::new(),
            connected: false,
            connecting: false,
            users: vec![],
            messages: vec!["Ласкаво просимо до Friends Voice!".into()],
            events_tx: tx,
            events_rx: rx,
            outbound: None,
        }
    }
}

impl App {
    fn connect(&mut self) {
        if self.connecting || self.connected { return; }
        let host = self.host.trim().to_string();
        let nickname = self.nickname.trim().to_string();
        let password = self.password.clone();
        if host.is_empty() || nickname.is_empty() || password.is_empty() {
            self.messages.push("⚠ Введіть адресу сервера, нікнейм і пароль".into());
            return;
        }
        self.connecting = true;
        let (tx, rx) = mpsc::channel::<Packet>();
        self.outbound = Some(tx);
        let events = self.events_tx.clone();

        thread::spawn(move || {
            let address = match host.to_socket_addrs().ok().and_then(|mut a| a.next()) {
                Some(a) => a,
                None => {
                    let _ = events.send(Event::Disconnected("Неправильна IP-адреса або порт".into()));
                    return;
                }
            };
            let mut stream = match TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
                Ok(s) => s,
                Err(e) => {
                    let _ = events.send(Event::Disconnected(format!("Не вдалося підключитися: {e}")));
                    return;
                }
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
            let hello = Packet::Hello { nickname, password };
            if writeln!(stream, "{}", serde_json::to_string(&hello).unwrap()).is_err() {
                let _ = events.send(Event::Disconnected("Не вдалося авторизуватися".into()));
                return;
            }
            let read_stream = match stream.try_clone() {
                Ok(s) => s,
                Err(e) => {
                    let _ = events.send(Event::Disconnected(format!("Помилка з'єднання: {e}")));
                    return;
                }
            };
            let mut reader = BufReader::new(read_stream);
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                let _ = events.send(Event::Disconnected("Сервер закрив з'єднання".into()));
                return;
            }
            match serde_json::from_str::<Packet>(&line) {
                Ok(Packet::Welcome { users }) => {
                    let _ = events.send(Event::Connected);
                    let _ = events.send(Event::Packet(Packet::Users { users }));
                }
                Ok(Packet::Error { text }) => {
                    let _ = events.send(Event::Disconnected(text));
                    return;
                }
                _ => {
                    let _ = events.send(Event::Disconnected("Невідома відповідь сервера".into()));
                    return;
                }
            }
            let _ = stream.set_read_timeout(None);
            let write_events = events.clone();
            thread::spawn(move || {
                for packet in rx {
                    if writeln!(stream, "{}", serde_json::to_string(&packet).unwrap()).is_err() {
                        let _ = write_events.send(Event::Disconnected("З'єднання втрачено".into()));
                        break;
                    }
                }
            });
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(n) if n > 8192 => break,
                    Ok(_) => {
                        if let Ok(packet) = serde_json::from_str::<Packet>(&line) {
                            let _ = events.send(Event::Packet(packet));
                        }
                    }
                }
            }
            let _ = events.send(Event::Disconnected("З'єднання із сервером завершено".into()));
        });
    }

    fn send_chat(&mut self) {
        let text = self.draft.trim().to_string();
        if text.is_empty() || text.chars().count() > 1000 { return; }
        if let Some(tx) = &self.outbound {
            if tx.send(Packet::SendChat { text }).is_ok() {
                self.draft.clear();
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        while let Ok(event) = self.events_rx.try_recv() {
            match event {
                Event::Connected => {
                    self.connected = true;
                    self.connecting = false;
                    self.messages.push("Підключено до сервера".into());
                }
                Event::Disconnected(text) => {
                    self.connected = false;
                    self.connecting = false;
                    self.outbound = None;
                    self.users.clear();
                    self.messages.push(format!("⚠ {text}"));
                }
                Event::Packet(packet) => match packet {
                    Packet::Chat { from, text } => self.messages.push(format!("{from}: {text}")),
                    Packet::Users { users } | Packet::Welcome { users } => self.users = users,
                    Packet::Notice { text } | Packet::Error { text } => self.messages.push(format!("• {text}")),
                    _ => {}
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        egui::SidePanel::left("sidebar").min_width(210.0).show(ctx, |ui| {
            ui.heading("🎧 Friends Voice");
            ui.label("Приватна мережа друзів");
            ui.separator();
            ui.label("АДРЕСА СЕРВЕРА");
            ui.add_enabled(!self.connected && !self.connecting, egui::TextEdit::singleline(&mut self.host));
            ui.label("НІКНЕЙМ");
            ui.add_enabled(!self.connected && !self.connecting, egui::TextEdit::singleline(&mut self.nickname));
            ui.label("ПАРОЛЬ КІМНАТИ");
            ui.add_enabled(!self.connected && !self.connecting, egui::TextEdit::singleline(&mut self.password).password(true));
            ui.add_space(8.0);
            if ui.add_enabled(!self.connected && !self.connecting, egui::Button::new("Підключитися")).clicked() {
                self.connect();
            }
            if self.connecting {
                ui.spinner();
                ui.label("Підключення…");
            }
            if self.connected {
                ui.colored_label(egui::Color32::LIGHT_GREEN, "● У мережі");
            }
            ui.separator();
            ui.heading("👥 Учасники");
            for name in &self.users { ui.label(format!("● {name}")); }
            ui.label(format!("{} / 5", self.users.len()));
            ui.separator();
            ui.label("🔊 Голосові канали — у версії 0.2");
            ui.label("📹 Камера — у наступних версіях");
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("# загальний");
            ui.separator();
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                for message in &self.messages {
                    ui.label(message);
                    ui.add_space(5.0);
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let edit = ui.add_enabled(
                    self.connected,
                    egui::TextEdit::singleline(&mut self.draft)
                        .hint_text("Написати повідомлення…")
                        .desired_width(f32::INFINITY)
                );
                if self.connected && edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.send_chat();
                }
                if ui.add_enabled(self.connected, egui::Button::new("Надіслати")).clicked() {
                    self.send_chat();
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 620.0])
            .with_min_inner_size([720.0, 440.0]),
        ..Default::default()
    };
    eframe::run_native("Friends Voice 0.1", options, Box::new(|cc| {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Ok(Box::new(App::default()))
    }))
}
