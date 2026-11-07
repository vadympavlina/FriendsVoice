//! Call stage: lobby, tiles, expanded view, share banner and call controls.
use super::*;

impl FriendsApp {
    pub(super) fn stage(&mut self, ui: &mut Ui) {
        egui::TopBottomPanel::top("stage_top").exact_height(52.0)
            .frame(egui::Frame::none().fill(STAGE).inner_margin(Margin::symmetric(18.0, 0.0)))
            .show_inside(ui, |ui| {
                let bar = ui.max_rect();
                ui.painter().hline(bar.x_range(), bar.bottom(), Stroke::new(1.0_f32, LINE));
                ui.horizontal_centered(|ui| {
                    ui.label(text(ph::SPEAKER_HIGH, 19.0, TEXT3));
                    ui.label(strong(tr("General"), 15.5, TEXT));
                    if let Some(start) = self.voice_started_at {
                        ui.add_space(6.0);
                        ui.label(mono(clock(start.elapsed().as_secs()), 13.0, GREEN));
                    }
                    if self.voice.is_some() && self.push_to_talk.load(Ordering::Relaxed) {
                        let talking = self.voice.as_ref()
                            .is_some_and(|v| v.transmitting.load(Ordering::Relaxed));
                        ui.add_space(6.0);
                        ui.label(text(trf("Hold {key} to talk", &[("key", &voice::vk_name(self.ptt_key.load(Ordering::Relaxed)))]),
                            12.5, if talking { GREEN } else { TEXT3 }));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if icon_button(ui, ph::GEAR, tr("Settings"), false, 34.0) {
                            self.settings_open = true;
                        }
                        if icon_button(ui,
                            if self.focus_mode { ph::CORNERS_IN } else { ph::CORNERS_OUT },
                            if self.focus_mode { tr("Show chat · Ctrl+E") } else { tr("Theatre mode · Ctrl+E") },
                            self.focus_mode, 34.0) {
                            self.focus_mode = !self.focus_mode;
                            if !self.focus_mode { self.chat_unread = 0; }
                        }
                        if icon_button(ui, ph::SIDEBAR_SIMPLE, tr("Left panel · Ctrl+B"),
                            self.sidebar_open, 34.0) {
                            self.sidebar_open = !self.sidebar_open;
                        }
                        let activity = self.last_notice.as_ref()
                            .filter(|(_, time)| time.elapsed().as_secs() < 8)
                            .map(|(message, _)| message.clone());
                        if let Some(activity) = activity {
                            ui.add_space(8.0);
                            let galley = truncated(ui, &activity, FontId::proportional(12.5), TEXT3,
                                (ui.available_width() - 8.0).max(40.0));
                            ui.label(egui::WidgetText::Galley(galley));
                        }
                    });
                });
            });

        if self.screen.is_some() {
            self.share_banner(ui);
        }

        let in_voice = self.voice.is_some();
        if in_voice || self.focus_mode {
            egui::TopBottomPanel::bottom("callbar").exact_height(92.0)
                .frame(egui::Frame::none().fill(STAGE))
                .show_inside(ui, |ui| self.call_bar(ui));
        }

        let area = ui.available_rect_before_wrap().shrink2(Vec2::new(24.0, 14.0));
        ui.allocate_rect(ui.available_rect_before_wrap(), Sense::hover());
        if in_voice || self.focus_mode {
            self.stage_tiles(ui, area);
        } else {
            self.lobby(ui, area);
        }
    }

    /// What you see before joining the call: who is in the channel and how to start.
    pub(super) fn lobby(&mut self, ui: &mut Ui, area: Rect) {
        let in_channel: Vec<String> = self.peers.iter().filter(|p| p.voice).map(|p| p.name.clone()).collect();
        let nobody_online = self.peers.is_empty();
        let ip = self.mesh.as_ref().and_then(|m| m.local_radmin_ips.first().copied());
        let content_h = if nobody_online { 400.0 } else { 270.0 };
        let width = area.width().min(540.0);
        let rect = Rect::from_center_size(
            Pos2::new(area.center().x, (area.center().y).max(area.top() + content_h * 0.5)),
            Vec2::new(width, content_h.min(area.height())));
        ui.allocate_new_ui(
            egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Center)),
            |ui| {
                brand_mark(ui, 88.0);
                ui.add_space(10.0);
                ui.label(strong(tr("General voice channel"), 22.0, TEXT));
                ui.add_space(2.0);
                ui.label(text(
                    if in_channel.is_empty() { tr("Nobody is in the channel yet").to_string() }
                    else { trf("In the channel: {names}", &[("names", &in_channel.join(", "))]) },
                    14.0, TEXT2));
                ui.add_space(16.0);
                if pill_button(ui, tr("Join voice"), ACCENT, ACCENT_INK, Vec2::new(250.0, 48.0), self.mesh.is_some()) {
                    self.toggle_voice();
                }
                if self.mesh.is_none() {
                    ui.add_space(6.0);
                    ui.label(text(tr("First restore the connection in the left panel."), 12.5, RED));
                }
                ui.add_space(14.0);
                // Shown on the first screen so the language can be found without reading English.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let total: f32 = Lang::ALL.iter().map(|l| l.native_name().chars().count() as f32 * 7.5 + 14.0).sum();
                    ui.add_space(((ui.available_width() - total) * 0.5).max(0.0));
                    for lang in Lang::ALL {
                        let active = i18n::language() == lang;
                        let label = text(lang.native_name(), 13.0, if active { TEXT } else { TEXT3 });
                        let response = ui.add(egui::Label::new(if active { label.underline() } else { label }).sense(Sense::click()));
                        if response.clicked() { self.set_language(lang); }
                        if response.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    }
                });
                if nobody_online {
                    ui.add_space(26.0);
                    egui::Frame::none().fill(CHAT).rounding(Rounding::same(12.0)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
                        ui.set_width(width - 32.0);
                        ui.label(strong(tr("How to invite friends"), 14.0, TEXT));
                        ui.add_space(4.0);
                        for step in [
                            tr("1. Everyone installs Radmin VPN and joins the same network."),
                            tr("2. Friends find each other automatically. If not, exchange Radmin addresses."),
                            tr("3. Press “Join voice”."),
                        ] {
                            ui.add(egui::Label::new(text(step, 13.0, TEXT2)).wrap());
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if let Some(ip) = ip {
                                if quiet_button(ui, tr("Copy my address")) {
                                    ui.output_mut(|o| o.copied_text = ip.to_string());
                                    self.ip_copied_at = Some(Instant::now());
                                    self.notice(trf("Address {ip} copied. Send it to your friend.", &[("ip", &ip)]));
                                }
                            }
                            if quiet_button(ui, tr("Add a friend by address")) {
                                self.sidebar_open = true;
                                self.add_friend_open = true;
                            }
                        });
                    });
                }
            });
    }

    /// Everything that can be on stage: people's cameras and shared screens.
    pub(super) fn stage_tiles(&mut self, ui: &mut Ui, area: Rect) {
        let in_voice = self.voice.is_some();
        let local_id = self.mesh.as_ref().map(|m| m.id).unwrap_or(0);
        struct Tile { key: (u64, StreamKind), name: String, note: Option<String>, speaking: bool,
                      badge: Option<&'static str>, contain: bool }
        let mut tiles = vec![Tile {
            key: (local_id, StreamKind::Camera),
            name: self.settings.nickname.clone(),
            note: Some(tr("you").into()),
            speaking: self.voice.as_ref().is_some_and(|v| v.transmitting.load(Ordering::Relaxed)),
            badge: if !in_voice { None }
                else if self.deafened.load(Ordering::Relaxed) { Some(ph::SPEAKER_SLASH) }
                else if !self.mic_enabled.load(Ordering::Relaxed) { Some(ph::MICROPHONE_SLASH) }
                else { None },
            contain: false,
        }];
        // Friends who have not joined voice are shown only in the sidebar.
        for peer in self.peers.iter().filter(|p| p.voice || !in_voice) {
            tiles.push(Tile {
                key: (peer.id, StreamKind::Camera),
                name: peer.name.clone(),
                note: (peer.protocol != PROTOCOL_VERSION).then(|| tr("update required").to_string()),
                speaking: self.voice.as_ref().is_some_and(|v| v.is_speaking(peer.id)),
                badge: None,
                contain: false,
            });
        }
        let mut screens: Vec<u64> = self.video_textures.keys()
            .filter(|k| k.1 == StreamKind::Screen).map(|k| k.0).collect();
        screens.sort_unstable();
        for id in screens {
            let (name, note) = if id == local_id {
                (self.settings.nickname.clone(), tr("your screen share").to_string())
            } else {
                (self.peers.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default(),
                 tr("is sharing a screen").to_string())
            };
            tiles.push(Tile { key: (id, StreamKind::Screen), name, note: Some(note), speaking: false, badge: None, contain: true });
        }

        // A friend's screen automatically becomes the big picture; a click pins any tile instead.
        let automatic = tiles.iter().find(|t| t.key.1 == StreamKind::Screen && t.key.0 != local_id).map(|t| t.key);
        let spotlight = self.spotlight.filter(|k| tiles.iter().any(|t| t.key == *k)).or(automatic);
        let gap = 12.0;
        let mut clicked: Option<(u64, StreamKind)> = None;
        let mut expand: Option<((u64, StreamKind), bool)> = None;

        if let Some(big) = spotlight {
            let others: Vec<&Tile> = tiles.iter().filter(|t| t.key != big).collect();
            let (main, strip) = spotlight_layout(area, others.len(), gap);
            if let Some(tile) = tiles.iter().find(|t| t.key == big) {
                let view = TileView { name: &tile.name, note: tile.note.as_deref(), speaking: tile.speaking,
                    badge: tile.badge, contain: tile.contain, small: false, salt: tile.key };
                let (response, action) = paint_tile(ui, main, &view, self.video_textures.get(&tile.key));
                if response.double_clicked() { expand = Some((tile.key, false)); }
                match action {
                    Some(TileAction::Expand) => expand = Some((tile.key, false)),
                    Some(TileAction::Fullscreen) => expand = Some((tile.key, true)),
                    None => {}
                }
            }
            for (tile, rect) in others.iter().zip(strip) {
                let view = TileView { name: &tile.name, note: tile.note.as_deref(), speaking: tile.speaking,
                    badge: tile.badge, contain: tile.contain, small: true, salt: tile.key };
                let (response, action) = paint_tile(ui, rect, &view, self.video_textures.get(&tile.key));
                if response.clicked() && action.is_none() { clicked = Some(tile.key); }
                match action {
                    Some(TileAction::Expand) => expand = Some((tile.key, false)),
                    Some(TileAction::Fullscreen) => expand = Some((tile.key, true)),
                    None => {}
                }
            }
        } else {
            let alone = self.peers.is_empty() && self.screen.is_none();
            let hint_height = if alone { 44.0 } else { 0.0 };
            let grid_area = Rect::from_min_max(area.min, area.max - Vec2::new(0.0, hint_height));
            let (columns, tile_w, tile_h) = best_grid(tiles.len(), grid_area.width(), grid_area.height(), gap);
            let rows = tiles.len().div_ceil(columns);
            let total_h = rows as f32 * tile_h + gap * (rows - 1) as f32;
            let top = grid_area.center().y - total_h * 0.5;
            for (index, tile) in tiles.iter().enumerate() {
                let (row, column) = (index / columns, index % columns);
                let in_row = (tiles.len() - row * columns).min(columns);
                let row_w = in_row as f32 * tile_w + gap * (in_row - 1) as f32;
                let left = grid_area.center().x - row_w * 0.5;
                let rect = Rect::from_min_size(
                    Pos2::new(left + column as f32 * (tile_w + gap), top + row as f32 * (tile_h + gap)),
                    Vec2::new(tile_w, tile_h));
                let view = TileView { name: &tile.name, note: tile.note.as_deref(), speaking: tile.speaking,
                    badge: tile.badge, contain: tile.contain, small: false, salt: tile.key };
                let (response, action) = paint_tile(ui, rect, &view, self.video_textures.get(&tile.key));
                if response.double_clicked() && self.video_textures.contains_key(&tile.key) { expand = Some((tile.key, false)); }
                else if response.clicked() && action.is_none() && tiles.len() > 1 { clicked = Some(tile.key); }
                match action {
                    Some(TileAction::Expand) => expand = Some((tile.key, false)),
                    Some(TileAction::Fullscreen) => expand = Some((tile.key, true)),
                    None => {}
                }
                if tiles.len() > 1 { response.on_hover_text(tr("Click to enlarge")); }
            }
            if alone {
                let center = Pos2::new(area.center().x, top + total_h + 26.0);
                ui.painter().text(center, Align2::CENTER_CENTER,
                    tr("Only you are here so far. Add a friend by address in the left panel."),
                    FontId::proportional(13.5), TEXT3);
            }
        }
        if let Some(key) = clicked { self.spotlight = Some(key); }
        if let Some((key, fullscreen)) = expand {
            self.open_fullview(key);
            if fullscreen { ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(true)); }
        }
    }

    /// One stream on the whole window, with zoom for reading small text and controls that hide when idle.
    pub(super) fn full_view(&mut self, ui: &mut Ui, key: (u64, StreamKind)) {
        let ctx = ui.ctx().clone();
        let rect = ui.max_rect();
        let local_id = self.mesh.as_ref().map(|m| m.id).unwrap_or(0);
        let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));

        let active = ctx.input(|i| i.pointer.delta().length() > 0.0 || i.pointer.any_down() || i.smooth_scroll_delta.length() > 0.0);
        if active { self.last_pointer_move = Instant::now(); }
        let idle = self.last_pointer_move.elapsed() > Duration::from_millis(2600) && !self.share_picker_open;
        let controls = ctx.animate_bool_with_time(egui::Id::new("fullview-controls"), !idle, 0.18);
        ctx.request_repaint_after(Duration::from_millis(60));
        if idle { ctx.set_cursor_icon(egui::CursorIcon::None); }

        let surface = ui.interact(rect, egui::Id::new("fullview-surface"), Sense::click_and_drag());
        let texture = self.video_textures.get(&key).cloned();

        // The picture: always whole (never cropped), zoom with the wheel, move by dragging.
        if let Some(texture) = &texture {
            let [tw, th] = texture.size();
            let aspect = tw as f32 / th.max(1) as f32;
            let base = if rect.width() / rect.height() > aspect {
                Vec2::new(rect.height() * aspect, rect.height())
            } else {
                Vec2::new(rect.width(), rect.width() / aspect)
            };
            let centre = rect.center();
            if surface.hovered() {
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let pointer = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(centre) - centre;
                    (self.fullview_zoom, self.fullview_pan) =
                        zoom_at(base, self.fullview_zoom, self.fullview_pan, pointer, scroll);
                }
            }
            if surface.dragged() && self.fullview_zoom > 1.0 { self.fullview_pan += surface.drag_delta(); }
            let size = base * self.fullview_zoom;
            self.fullview_pan = clamp_pan(self.fullview_pan, base, self.fullview_zoom, rect.size());
            let shown = Rect::from_center_size(centre + self.fullview_pan, size);
            let mut shape = egui::epaint::RectShape::filled(shown, Rounding::ZERO, Color32::WHITE);
            shape.fill_texture_id = texture.id();
            shape.uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            ui.painter_at(rect).add(shape);
            if surface.hovered() && self.fullview_zoom > 1.0 {
                ctx.set_cursor_icon(if surface.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
            }
        } else {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, tr("The stream has ended"),
                FontId::proportional(18.0), TEXT2);
        }
        if surface.double_clicked() { self.close_fullview(&ctx); return; }
        if ctx.input(|i| i.key_pressed(egui::Key::Num0)) { self.fullview_zoom = 1.0; self.fullview_pan = Vec2::ZERO; }

        // Who is talking, so nobody is missed while a screen fills the monitor.
        let talkers: Vec<String> = self.peers.iter()
            .filter(|p| self.voice.as_ref().is_some_and(|v| v.is_speaking(p.id)))
            .map(|p| p.name.clone()).collect();
        if !talkers.is_empty() {
            let label = format!("{} {}", ph::SPEAKER_HIGH, talkers.join(", "));
            let galley = truncated(ui, &label, FontId::proportional(14.0), TEXT, rect.width() * 0.6);
            let size = galley.size() + Vec2::new(24.0, 14.0);
            let pill = Rect::from_min_size(Pos2::new(rect.left() + 16.0, rect.bottom() - 108.0 - size.y), size);
            ui.painter().rect_filled(pill, Rounding::same(10.0), Color32::from_black_alpha(170));
            ui.painter().rect_stroke(pill, Rounding::same(10.0), Stroke::new(1.5_f32, GREEN));
            ui.painter().galley(pill.min + Vec2::new(12.0, 7.0), galley, TEXT);
        }

        if controls <= 0.01 { return; }
        let fade = |alpha: f32| Color32::from_black_alpha((alpha * controls) as u8);

        // Top strip: what this is, and the way out.
        let top = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 64.0));
        ui.painter().rect_filled(top, Rounding::ZERO, fade(150.0));
        let who = if key.0 == local_id { self.settings.nickname.clone() }
            else { self.peers.iter().find(|p| p.id == key.0).map(|p| p.name.clone()).unwrap_or_default() };
        let what = match (key.1, key.0 == local_id) {
            (StreamKind::Screen, true) => tr("your screen share"),
            (StreamKind::Screen, false) => tr("is sharing a screen"),
            (StreamKind::Camera, true) => tr("your camera"),
            (StreamKind::Camera, false) => tr("camera"),
        };
        let size_text = texture.as_ref().map(|t| format!("{}×{}", t.size()[0], t.size()[1])).unwrap_or_default();
        let zoom_text = if self.fullview_zoom > 1.01 { format!("  ·  {:.0}%", self.fullview_zoom * 100.0) } else { String::new() };
        ui.painter().text(Pos2::new(rect.left() + 20.0, rect.top() + 22.0), Align2::LEFT_CENTER,
            format!("{who}  ·  {what}"), semibold(17.0), Color32::from_white_alpha((255.0 * controls) as u8));
        ui.painter().text(Pos2::new(rect.left() + 20.0, rect.top() + 44.0), Align2::LEFT_CENTER,
            trf("{info}  ·  mouse wheel: zoom, 0: reset", &[("info", &format!("{size_text}{zoom_text}"))]), FontId::proportional(12.0),
            Color32::from_white_alpha((170.0 * controls) as u8));
        let mut leave_view = false;
        let mut toggle_fullscreen = false;
        let buttons = Rect::from_min_size(Pos2::new(rect.right() - 108.0, rect.top() + 14.0), Vec2::new(96.0, 38.0));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(buttons).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if icon_button(ui, ph::ARROWS_IN, tr("Back to the normal view · Esc"), false, 38.0) { leave_view = true; }
            if icon_button(ui, if fullscreen { ph::CORNERS_IN } else { ph::CORNERS_OUT },
                if fullscreen { tr("Leave full screen · Esc") } else { tr("Full screen · F11") }, fullscreen, 38.0) {
                toggle_fullscreen = true;
            }
        });

        // Bottom: the same call controls as everywhere else.
        if self.voice.is_some() || self.screen.is_some() {
            let bottom = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 92.0), rect.max);
            ui.painter().rect_filled(bottom, Rounding::ZERO, fade(150.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(bottom), |ui| self.call_bar(ui));
        }
        if leave_view { self.close_fullview(&ctx); }
        if toggle_fullscreen { ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen)); }
    }

    /// Strip under the top bar while you are sharing your screen.
    pub(super) fn share_banner(&mut self, ui: &mut Ui) {
        let Some(share) = self.screen.as_ref() else { return; };
        let (width, height) = (share.stats.width.load(Ordering::Relaxed), share.stats.height.load(Ordering::Relaxed));
        let overloaded = share.stats.overloaded.load(Ordering::Relaxed);
        let title = share.title.clone();
        let fps = share.quality.fps;
        let viewers = self.peers.iter().filter(|p| p.voice).count();
        let mbps = self.share_rate.2;
        let mut stop = false;
        egui::TopBottomPanel::top("share_banner").exact_height(46.0)
            .frame(egui::Frame::none().fill(STAGE.lerp_to_gamma(ACCENT, 0.16)).inner_margin(Margin::symmetric(18.0, 0.0)))
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(text(ph::MONITOR_ARROW_UP, 20.0, ACCENT));
                    let name = truncated(ui, &trf("You are sharing: {title}", &[("title", &title)]), semibold(14.0), TEXT, (ui.available_width() * 0.42).max(120.0));
                    ui.label(egui::WidgetText::Galley(name));
                    let detail = if width == 0 { tr("starting…").to_string() } else {
                        trf("{width}×{height} · {fps} fps · {mbps} Mbit/s", &[("width", &width), ("height", &height), ("fps", &fps), ("mbps", &decimal(mbps, 1))])
                    };
                    ui.label(text(detail, 12.5, TEXT2));
                    if viewers == 0 { ui.label(text(tr("Nobody in the channel sees the share yet"), 12.5, TEXT3)); }
                    else { ui.label(text(trf("watching: {count}", &[("count", &viewers)]), 12.5, TEXT2)); }
                    if overloaded {
                        ui.label(text(format!("{} {}", ph::WARNING, tr("Your computer cannot keep up: lower the quality or frame rate")), 12.5, WARN));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if pill_button(ui, tr("Stop sharing"), RED, Color32::WHITE, Vec2::new(0.0, 30.0), true) { stop = true; }
                    });
                });
            });
        if stop { self.stop_screen_share(None); }
    }


    pub(super) fn call_bar(&mut self, ui: &mut Ui) {
        let voice = self.voice.is_some();
        let mic = self.mic_enabled.load(Ordering::Relaxed);
        let deaf = self.deafened.load(Ordering::Relaxed);
        let camera = self.video.as_ref().is_some_and(|v| v.enabled());
        let camera_starting = self.video.as_ref().is_some_and(|v| v.starting());
        let chat_button = self.focus_mode || self.fullview.is_some();
        // Shrink the controls instead of clipping them in narrow windows.
        let room = ui.available_width() - 24.0;
        let d = if room >= 470.0 { 48.0 } else { 40.0 };
        let leave_width = if room >= 470.0 { 150.0 } else { 96.0 };
        let join_width = if room >= 300.0 { 240.0 } else { room.max(160.0) };
        let gap = 10.0;
        let buttons = 4.0 + if chat_button { 1.0 } else { 0.0 };
        let width = if voice { buttons * d + buttons * gap + 6.0 + leave_width }
            else { join_width + if chat_button { d + gap } else { 0.0 } };
        ui.add_space(if d > 44.0 { 22.0 } else { 26.0 });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            ui.add_space(((ui.available_width() - width) * 0.5).max(0.0));
            if voice {
                if round_button(ui, if mic { ph::MICROPHONE } else { ph::MICROPHONE_SLASH },
                    if mic { tr("Turn microphone off · Ctrl+M") } else { tr("Turn microphone on · Ctrl+M") },
                    if mic { RAISED } else { RED }, TEXT, true, d) {
                    self.mic_enabled.store(!mic, Ordering::Relaxed);
                }
                if round_button(ui, ph::HEADPHONES,
                    if deaf { tr("Turn sound on") } else { tr("Turn sound off") },
                    if deaf { RED } else { RAISED }, TEXT, true, d) {
                    self.deafened.store(!deaf, Ordering::Relaxed);
                }
                if round_button(ui, if camera { ph::VIDEO_CAMERA } else { ph::VIDEO_CAMERA_SLASH },
                    if camera_starting { tr("The camera is starting. Press to cancel.") }
                        else if camera { tr("Turn camera off") } else { tr("Turn camera on") },
                    if camera_starting { ACCENT } else if camera { TEXT } else { RAISED },
                    if camera_starting { ACCENT_INK } else if camera { STAGE } else { TEXT },
                    self.mesh.is_some(), d) {
                    self.toggle_camera();
                }
                let sharing = self.screen.is_some();
                if round_button(ui, ph::MONITOR_ARROW_UP,
                    if sharing { tr("Stop screen sharing") } else { tr("Share a screen or window") },
                    if sharing { ACCENT } else { RAISED }, if sharing { ACCENT_INK } else { TEXT }, true, d) {
                    if sharing { self.stop_screen_share(None); } else { self.open_share_picker(); }
                }
                ui.add_space(6.0);
                if pill_button(ui, if leave_width > 120.0 { tr("Leave channel") } else { tr("Leave") }, RED, Color32::WHITE, Vec2::new(leave_width, d), true) {
                    self.toggle_voice();
                }
            } else if pill_button(ui, tr("Join voice"), ACCENT, ACCENT_INK,
                Vec2::new(join_width, d), self.mesh.is_some()) {
                self.toggle_voice();
            }
            if chat_button {
                let unread = self.chat_unread;
                if round_button(ui, ph::CHAT_CIRCLE_TEXT,
                    if unread > 0 { tr("Open chat · new messages") } else { tr("Open chat") },
                    if unread > 0 { ACCENT } else { RAISED },
                    if unread > 0 { ACCENT_INK } else { TEXT }, true, d) {
                    self.focus_mode = false;
                    self.chat_unread = 0;
                    self.fullview = None;
                    if ui.ctx().input(|i| i.viewport().fullscreen.unwrap_or(false)) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    }
                }
            }
        });
    }


}
