//! The "what do you want to show" window.
use super::*;

impl FriendsApp {
    pub(super) fn share_picker(&mut self, ctx: &egui::Context) {
        let screen_rect = ctx.screen_rect();
        egui::Area::new("share_dim".into()).order(egui::Order::Middle).fixed_pos(screen_rect.min)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(screen_rect.size(), Sense::click());
                ui.painter().rect_filled(rect, Rounding::ZERO, Color32::from_black_alpha(150));
                if response.clicked() { self.share_picker_open = false; }
            });

        let friends = self.peers.iter().filter(|p| p.voice).count();
        let mut start = false;
        let mut close = false;
        let mut refresh = false;
        egui::Window::new("share_picker").title_bar(false).collapsible(false).resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO).min_width(580.0).max_width(580.0)
            .frame(egui::Frame::none().fill(CHAT).stroke(Stroke::new(1.0_f32, LINE)).rounding(Rounding::same(14.0))
                .inner_margin(Margin::same(22.0))
                .shadow(egui::Shadow { offset: Vec2::new(0.0, 12.0), blur: 32.0, spread: 0.0, color: Color32::from_black_alpha(140) }))
            .show(ctx, |ui| {
                ui.label(strong(tr("What do you want to show?"), 19.0, TEXT));
                ui.add_space(2.0);
                Self::hint(ui, tr("Friends in the channel will see it as a large picture. Computer sound is not shared yet."));
                ui.add_space(12.0);

                let screens = self.share_sources.iter().filter(|s| !s.is_window()).count();
                let windows = self.share_sources.iter().filter(|s| s.is_window()).count();
                ui.horizontal(|ui| {
                    if let Some(tab) = segmented(ui, &[trf("Screen ({count})", &[("count", &screens)]), trf("Window ({count})", &[("count", &windows)])], self.share_tab) {
                        if tab != self.share_tab {
                            self.share_tab = tab;
                            self.share_selected = self.share_sources.iter().position(|s| s.is_window() == (tab == 1));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, ph::ARROWS_CLOCKWISE, tr("Refresh list"), false, 32.0) { refresh = true; }
                    });
                });
                ui.add_space(8.0);

                let want_windows = self.share_tab == 1;
                let visible: Vec<usize> = (0..self.share_sources.len())
                    .filter(|i| self.share_sources[*i].is_window() == want_windows).collect();
                egui::Frame::none().fill(STAGE).rounding(Rounding::same(10.0)).inner_margin(Margin::same(6.0)).show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("share_sources").max_height(236.0).auto_shrink([false, true]).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if visible.is_empty() {
                            ui.add_space(14.0);
                            ui.vertical_centered(|ui| {
                                if cfg!(windows) {
                                    ui.label(strong(if want_windows { tr("No windows to share") } else { tr("No screens found") }, 14.0, TEXT));
                                    ui.label(text(tr("Open the app you need and press “Refresh list”."), 12.5, TEXT3));
                                } else {
                                    ui.label(strong(tr("Screen sharing only works on Windows"), 14.0, TEXT));
                                }
                            });
                            ui.add_space(14.0);
                        }
                        for index in visible.iter().copied() {
                            let source = &self.share_sources[index];
                            let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0), Sense::click());
                            let selected = self.share_selected == Some(index);
                            if selected || response.hovered() {
                                ui.painter().rect_filled(rect, Rounding::same(8.0), if selected { RAISED } else { RAISED.linear_multiply(0.5) });
                            }
                            if selected {
                                ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.5_f32, ACCENT));
                            }
                            ui.painter().text(Pos2::new(rect.left() + 26.0, rect.center().y), Align2::CENTER_CENTER,
                                if source.is_window() { ph::APP_WINDOW } else { ph::MONITOR },
                                FontId::proportional(24.0), if selected { ACCENT } else { TEXT2 });
                            let title = truncated(ui, &source.title, semibold(14.0), TEXT, rect.width() - 64.0);
                            let detail = truncated(ui, &source.detail, FontId::proportional(12.0), TEXT3, rect.width() - 64.0);
                            ui.painter().galley(Pos2::new(rect.left() + 52.0, rect.top() + 8.0), title, TEXT);
                            ui.painter().galley(Pos2::new(rect.left() + 52.0, rect.top() + 28.0), detail, TEXT3);
                            if response.clicked() { self.share_selected = Some(index); }
                            if response.double_clicked() { self.share_selected = Some(index); start = true; }
                        }
                    });
                });
                ui.add_space(14.0);

                ui.label(strong(tr("Quality"), 14.0, TEXT));
                ui.add_space(4.0);
                ui.label(text(tr("Resolution"), 12.5, TEXT3));
                let labels: Vec<String> = screen::RESOLUTIONS.iter().map(|h| screen::resolution_label(*h)).collect();
                let current = screen::RESOLUTIONS.iter().position(|h| *h == self.share_quality.max_height).unwrap_or(1);
                if let Some(i) = segmented(ui, &labels, current) { self.share_quality.max_height = screen::RESOLUTIONS[i]; }
                ui.add_space(6.0);
                ui.label(text(tr("Frames per second"), 12.5, TEXT3));
                let labels: Vec<String> = screen::FRAME_RATES.iter().map(|f| f.to_string()).collect();
                let current = screen::FRAME_RATES.iter().position(|f| *f == self.share_quality.fps).unwrap_or(1);
                if let Some(i) = segmented(ui, &labels, current) { self.share_quality.fps = screen::FRAME_RATES[i]; }
                ui.add_space(8.0);

                let source_size = self.share_selected.and_then(|i| self.share_sources.get(i)).map(|s| s.size)
                    .filter(|(w, h)| *w > 0 && *h > 0).unwrap_or((1920, 1080));
                let (out_w, out_h) = self.share_quality.output_size(source_size.0, source_size.1);
                let mbps = self.share_quality.bitrate(out_w, out_h) as f32 / 1_000_000.0;
                let each = decimal(mbps, 1);
                Self::hint(ui, if friends > 1 {
                    trf("You will send {width}×{height}, {fps} fps: ≈{each} Mbit/s to each friend, ≈{total} Mbit/s in total.",
                        &[("width", &out_w), ("height", &out_h), ("fps", &self.share_quality.fps),
                          ("each", &each), ("total", &decimal(mbps * friends as f32, 1))])
                } else {
                    trf("You will send {width}×{height}, {fps} fps: ≈{each} Mbit/s to each friend.",
                        &[("width", &out_w), ("height", &out_h), ("fps", &self.share_quality.fps), ("each", &each)])
                });
                if self.share_quality.fps == 60 || self.share_quality.max_height >= 1440 {
                    ui.label(text(tr("High quality puts a heavy load on the processor and the connection. If sharing stutters, lower it."), 12.5, WARN));
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, tr("Start sharing"), self.share_selected.is_some()) { start = true; }
                        if quiet_button(ui, tr("Cancel")) { close = true; }
                    });
                });
            });
        if refresh {
            self.share_sources = screen::list_sources();
            self.share_selected = self.share_sources.iter().position(|s| s.is_window() == (self.share_tab == 1));
        }
        if close { self.share_picker_open = false; }
        if start { self.start_screen_share(); }
    }

}
