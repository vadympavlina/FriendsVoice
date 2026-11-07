//! Left rail: channel, people, network block and user bar.
use super::*;

impl FriendsApp {
    pub(super) fn rail(&mut self, ui: &mut Ui) {
        egui::TopBottomPanel::bottom("userbar").exact_height(58.0)
            .frame(egui::Frame::none().fill(USERBAR).inner_margin(Margin::symmetric(10.0, 0.0)))
            .show_inside(ui, |ui| self.user_bar(ui));

        // Brand header.
        let (header, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::hover());
        ui.allocate_new_ui(
            egui::UiBuilder::new().max_rect(header.shrink2(Vec2::new(14.0, 0.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                brand_mark(ui, 28.0);
                ui.add_space(2.0);
                ui.label(strong("Friends Voice", 15.5, TEXT));
            });
        ui.painter().hline(header.x_range(), header.bottom(), Stroke::new(1.0_f32, LINE));

        egui::ScrollArea::vertical().id_salt("rail_scroll").auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Frame::none().inner_margin(Margin::symmetric(8.0, 0.0)).show(ui, |ui| {
                    let in_voice = self.voice.is_some();
                    let online = self.mesh.is_some();

                    section_label(ui, "Канали", None);
                    if nav_row(ui, ph::SPEAKER_HIGH, "Загальний", in_voice, online && !in_voice, 0)
                        .on_hover_text(if in_voice { "Ти в цьому каналі" } else { "Приєднатися до голосу" })
                        .clicked() {
                        self.toggle_voice();
                    }
                    // People currently in the voice channel are nested under it.
                    if in_voice {
                        let nickname = self.settings.nickname.clone();
                        let transmitting = self.voice.as_ref()
                            .is_some_and(|v| v.transmitting.load(Ordering::Relaxed));
                        let camera = self.video.as_ref().is_some_and(|v| v.enabled());
                        person_row(ui, 18.0, &nickname, None, transmitting,
                            camera.then_some(ph::VIDEO_CAMERA));
                    }
                    for peer in self.peers.iter().filter(|p| p.voice) {
                        let speaking = self.voice.as_ref().is_some_and(|v| v.is_speaking(peer.id));
                        let outdated = peer.protocol != PROTOCOL_VERSION;
                        person_row(ui, 18.0, &peer.name,
                            outdated.then_some(("Потрібне оновлення", RED)), speaking,
                            if self.video_textures.contains_key(&(peer.id, StreamKind::Screen)) { Some(ph::MONITOR_ARROW_UP) }
                                else if self.video_textures.contains_key(&(peer.id, StreamKind::Camera)) { Some(ph::VIDEO_CAMERA) } else { None });
                    }
                    if nav_row(ui, ph::HASH, "Чат", !self.focus_mode, true, self.chat_unread).clicked() {
                        self.focus_mode = false;
                        self.chat_unread = 0;
                    }

                    let idle: Vec<_> = self.peers.iter().filter(|p| !p.voice).collect();
                    let idle_count = idle.len() + usize::from(!in_voice);
                    section_label(ui, "У мережі", Some(format!("{}", idle_count)));
                    if !in_voice {
                        let nickname = self.settings.nickname.clone();
                        person_row(ui, 0.0, &format!("{nickname} (ти)"), None, false, None);
                    }
                    for peer in idle {
                        let outdated = peer.protocol != PROTOCOL_VERSION;
                        person_row(ui, 0.0, &peer.name,
                            outdated.then_some(("Потрібне оновлення", RED)), false, None);
                    }
                    if self.peers.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.add(egui::Label::new(text(
                                "Поки нікого. Додай друга за його адресою Radmin VPN.", 12.5, TEXT3)).wrap());
                        });
                    }

                    section_label(ui, "Мережа", None);
                    self.network_block(ui);
                });
            });
    }

    pub(super) fn network_block(&mut self, ui: &mut Ui) {
        let ip = self.mesh.as_ref().and_then(|m| m.local_radmin_ips.first().copied());
        let connected = self.mesh.is_some();
        // One plain sentence about the connection, then what to do about it.
        let (color, status) = if !connected { (RED, "Немає з'єднання") }
            else if ip.is_none() { (WARN, "Radmin VPN не знайдено") }
            else { (GREEN, "Підключено до Radmin VPN") };
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 3.5, color);
            ui.label(text(status, 13.0, TEXT));
        });
        if !connected {
            if let Some(error) = &self.network_error {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(text(error.clone(), 12.0, RED)).wrap());
                });
            }
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                if primary_button(ui, "Спробувати знову", true) { self.retry_network(); }
            });
        } else if ip.is_none() {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.add(egui::Label::new(text(
                    "Запусти Radmin VPN і зайди в мережу з друзями. Тоді тут з'явиться твоя адреса.", 12.5, TEXT3)).wrap());
            });
        }
        if let Some(ip) = ip {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(text("Твоя адреса, її треба дати друзям", 12.0, TEXT3));
            });
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(mono(ip.to_string(), 14.0, TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    let copied = self.ip_copied_at.is_some_and(|t| t.elapsed().as_secs() < 2);
                    if icon_button(ui, if copied { ph::CHECK } else { ph::COPY },
                        if copied { "Скопійовано" } else { "Скопіювати свою адресу" }, false, 28.0) {
                        ui.output_mut(|o| o.copied_text = ip.to_string());
                        self.ip_copied_at = Some(Instant::now());
                    }
                });
            });
        }
        ui.add_space(4.0);
        if nav_row(ui, ph::USER_PLUS, "Додати друга за адресою", self.add_friend_open, true, 0).clicked() {
            self.add_friend_open = !self.add_friend_open;
        }
        if self.add_friend_open {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() - 8.0);
                    ui.add(egui::Label::new(text(
                        "Друзі зазвичай знаходяться самі. Якщо ні, введи адресу друга з його Radmin VPN: вона починається з 26. Програма запам'ятає її.",
                        12.0, TEXT3)).wrap());
                    ui.add_space(2.0);
                    let entry = ui.add(egui::TextEdit::singleline(&mut self.friend_ip)
                        .hint_text("26.12.34.56")
                        .font(FontId::monospace(13.0))
                        .margin(Margin::symmetric(10.0, 8.0))
                        .desired_width(f32::INFINITY));
                    if entry.changed() { self.friend_ip_error = None; }
                    let submit = entry.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if let Some(error) = &self.friend_ip_error {
                        ui.add(egui::Label::new(text(error.clone(), 12.0, RED)).wrap());
                    }
                    ui.add_space(2.0);
                    if (primary_button(ui, "Знайти друга", self.mesh.is_some()) || submit)
                        && self.mesh.is_some() {
                        self.add_friend();
                    }
                });
            });
        }
    }

    pub(super) fn user_bar(&mut self, ui: &mut Ui) {
        let voice = self.voice.is_some();
        let mic = self.mic_enabled.load(Ordering::Relaxed);
        let deaf = self.deafened.load(Ordering::Relaxed);
        ui.horizontal_centered(|ui| {
            avatar(ui, &self.settings.nickname, 32.0, false);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(12.0);
                let name = truncated(ui, &self.settings.nickname, semibold(13.5), TEXT, 84.0);
                ui.label(egui::WidgetText::Galley(name));
                ui.label(text(if voice { "У голосі" } else { "У мережі" }, 11.5, TEXT3));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if icon_button(ui, ph::GEAR, "Налаштування", self.settings_open, 32.0) {
                    self.settings_open = !self.settings_open;
                }
                let tint = |on: bool| if on { RED } else { TEXT2 };
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0),
                    if voice { Sense::click() } else { Sense::hover() });
                let resp = resp.on_hover_text(if !voice { "Зайди в голосовий канал, щоб керувати звуком" } else if deaf { "Увімкнути звук" } else { "Вимкнути звук" });
                if voice && resp.hovered() {
                    ui.painter().rect_filled(rect, Rounding::same(8.0), HOVER);
                }
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, ph::HEADPHONES,
                    FontId::proportional(18.0), if voice { tint(deaf) } else { TEXT3 });
                if voice && resp.clicked() { self.deafened.store(!deaf, Ordering::Relaxed); }
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0),
                    if voice { Sense::click() } else { Sense::hover() });
                let resp = resp.on_hover_text(if !voice { "Зайди в голосовий канал, щоб користуватися мікрофоном" }
                    else if mic { "Вимкнути мікрофон · Ctrl+M" } else { "Увімкнути мікрофон · Ctrl+M" });
                if voice && resp.hovered() {
                    ui.painter().rect_filled(rect, Rounding::same(8.0), HOVER);
                }
                ui.painter().text(rect.center(), Align2::CENTER_CENTER,
                    if mic || !voice { ph::MICROPHONE } else { ph::MICROPHONE_SLASH },
                    FontId::proportional(18.0), if voice { tint(!mic) } else { TEXT3 });
                if voice && resp.clicked() { self.mic_enabled.store(!mic, Ordering::Relaxed); }
            });
        });
    }


}
