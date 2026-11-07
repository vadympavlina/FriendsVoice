//! Text chat panel.
use super::*;

impl FriendsApp {
    pub(super) fn chat_panel(&mut self, ui: &mut Ui) {
        egui::TopBottomPanel::top("chat_top").exact_height(52.0)
            .frame(egui::Frame::none().fill(CHAT).inner_margin(Margin::symmetric(16.0, 0.0)))
            .show_inside(ui, |ui| {
                let bar = ui.max_rect();
                ui.painter().hline(bar.x_range(), bar.bottom(), Stroke::new(1.0_f32, LINE));
                ui.horizontal_centered(|ui| {
                    ui.label(text(ph::HASH, 19.0, TEXT3));
                    ui.label(strong("Чат", 15.5, TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, ph::MAGNIFYING_GLASS, "Пошук у чаті · Ctrl+F",
                            self.chat_search_open, 34.0) {
                            self.chat_search_open = !self.chat_search_open;
                            self.chat_search_focus = self.chat_search_open;
                            if !self.chat_search_open { self.chat_search.clear(); }
                        }
                    });
                });
            });

        egui::TopBottomPanel::bottom("composer")
            .frame(egui::Frame::none().fill(CHAT).inner_margin(Margin { left: 16.0, right: 16.0, top: 6.0, bottom: 16.0 }))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let online = self.mesh.is_some();
                    let field = ui.add_enabled(online, egui::TextEdit::singleline(&mut self.draft)
                        .hint_text(if online { "Написати друзям" } else { "Немає з'єднання" })
                        .margin(Margin::symmetric(12.0, 10.0))
                        .desired_width(ui.available_width() - 80.0));
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (field.has_focus() || field.lost_focus()) && enter {
                        self.send_chat();
                        field.request_focus();
                    }
                    let ready = online && !self.draft.trim().is_empty();
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(38.0),
                        if ready { Sense::click() } else { Sense::hover() });
                    let resp = resp.on_hover_text("Надіслати · Enter");
                    ui.painter().rect_filled(rect, Rounding::same(8.0),
                        if ready { if resp.hovered() { ACCENT.lerp_to_gamma(Color32::WHITE, 0.12) } else { ACCENT } } else { RAISED });
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, ph::PAPER_PLANE_RIGHT,
                        FontId::proportional(19.0), if ready { ACCENT_INK } else { TEXT3 });
                    if ready && resp.clicked() { self.send_chat(); }
                });
                if self.draft.chars().count() > 900 {
                    ui.label(text(format!("{}/1000", self.draft.chars().count()), 11.5, TEXT3));
                }
            });

        if self.chat_search_open {
            egui::TopBottomPanel::top("chat_search").frame(egui::Frame::none().fill(CHAT)
                .inner_margin(Margin { left: 16.0, right: 16.0, top: 10.0, bottom: 4.0 }))
                .show_inside(ui, |ui| {
                    let field = ui.add(egui::TextEdit::singleline(&mut self.chat_search)
                        .hint_text("Пошук за текстом або іменем")
                        .margin(Margin::symmetric(12.0, 8.0))
                        .desired_width(f32::INFINITY));
                    if self.chat_search_focus {
                        field.request_focus();
                        self.chat_search_focus = false;
                    }
                });
        }

        let search = self.chat_search.trim().to_lowercase();
        let nickname = self.settings.nickname.clone();
        egui::CentralPanel::default().frame(egui::Frame::none().fill(CHAT)).show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("chat_feed").auto_shrink([false, false])
                .stick_to_bottom(search.is_empty())
                .show(ui, |ui| {
                    egui::Frame::none().inner_margin(Margin::symmetric(16.0, 12.0)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let mut previous: Option<&str> = None;
                        let mut shown = 0;
                        for message in &self.messages {
                            if !search.is_empty()
                                && !message.text.to_lowercase().contains(&search)
                                && !message.name.to_lowercase().contains(&search) {
                                continue;
                            }
                            shown += 1;
                            if message.system {
                                previous = None;
                                ui.add_space(4.0);
                                let note = if message.repeat > 1 {
                                    format!("{}  ×{}", message.text, message.repeat)
                                } else { message.text.clone() };
                                ui.add(egui::Label::new(text(note, 12.5, TEXT3)).wrap());
                                ui.add_space(4.0);
                                continue;
                            }
                            let first = previous != Some(message.name.as_str());
                            ui.add_space(if first { 12.0 } else { 1.0 });
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                if first {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                                    paint_avatar(ui.painter(), rect.center(), 34.0, &message.name);
                                } else {
                                    ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                                }
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    if first {
                                        let mine = message.name == nickname;
                                        ui.horizontal(|ui| {
                                            ui.label(strong(&message.name, 14.0, name_color(&message.name)));
                                            if mine { ui.label(text("ти", 11.5, TEXT3)); }
                                            ui.label(text(&message.time, 11.5, TEXT3));
                                        });
                                    }
                                    ui.add(egui::Label::new(text(&message.text, 14.0, TEXT)).wrap());
                                });
                            });
                            previous = Some(message.name.as_str());
                        }
                        if shown == 0 && !search.is_empty() {
                            ui.add_space(24.0);
                            ui.vertical_centered(|ui| {
                                ui.label(strong("Нічого не знайдено", 14.0, TEXT));
                                ui.label(text("Спробуй інше слово або ім'я.", 12.5, TEXT3));
                            });
                        }
                    });
                });
        });
    }


}
