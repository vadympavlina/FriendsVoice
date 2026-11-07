//! Settings pages.
use super::*;

impl FriendsApp {
    pub(super) fn settings_nav(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(strong(tr("Settings"), 15.5, TEXT));
        });
        ui.add_space(12.0);
        for (index, (glyph, label)) in [
            (ph::USER_CIRCLE, tr("Profile")),
            (ph::MICROPHONE, tr("Sound")),
            (ph::VIDEO_CAMERA, tr("Video and screen sharing")),
            (ph::SIDEBAR_SIMPLE, tr("Appearance and keys")),
            (ph::INFO, tr("About")),
        ].into_iter().enumerate() {
            if nav_row(ui, glyph, label, self.settings_page == index, true, 0).clicked() {
                self.settings_page = index;
            }
        }
    }

    pub(super) fn settings_page_ui(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().id_salt("settings_scroll").auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(620.0);
                let title = [tr("Profile"), tr("Sound"), tr("Video and screen sharing"), tr("Appearance and keys"), tr("About")][self.settings_page.min(4)];
                ui.horizontal(|ui| {
                    ui.label(strong(title, 22.0, TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, ph::X, tr("Close settings · Esc"), false, 34.0) {
                            self.settings_open = false;
                        }
                    });
                });
                ui.add_space(6.0);
                match self.settings_page {
                    0 => self.settings_profile(ui),
                    1 => self.settings_audio(ui),
                    2 => self.settings_video(ui),
                    3 => self.settings_appearance(ui),
                    _ => self.settings_about(ui),
                }
                ui.add_space(32.0);
            });
    }

    pub(super) fn group(ui: &mut Ui, title: &str) {
        ui.add_space(22.0);
        ui.label(strong(title, 15.0, TEXT));
        ui.add_space(2.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(rect, Rounding::ZERO, LINE);
        ui.add_space(8.0);
    }
    pub(super) fn field(ui: &mut Ui, label: &str) {
        ui.add_space(6.0);
        ui.label(text(label, 13.0, TEXT2));
    }
    pub(super) fn hint(ui: &mut Ui, note: impl Into<String>) {
        ui.add(egui::Label::new(text(note, 12.5, TEXT3)).wrap());
    }

    pub(super) fn settings_audio(&mut self, ui: &mut Ui) {
        let devices_before = (self.settings.input_device.clone(), self.settings.output_device.clone());
        Self::group(ui, tr("Devices"));
        Self::field(ui, tr("Microphone"));
        egui::ComboBox::from_id_salt("fv_input_device")
            .width(ui.available_width().min(420.0))
            .selected_text(if self.settings.input_device.is_empty() { tr("System default") } else { &self.settings.input_device })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.input_device, String::new(), tr("System default"));
                for name in &self.input_devices {
                    ui.selectable_value(&mut self.settings.input_device, name.clone(), name);
                }
            });
        Self::field(ui, tr("Headphones or speakers"));
        egui::ComboBox::from_id_salt("fv_output_device")
            .width(ui.available_width().min(420.0))
            .selected_text(if self.settings.output_device.is_empty() { tr("System default") } else { &self.settings.output_device })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.output_device, String::new(), tr("System default"));
                for name in &self.output_devices {
                    ui.selectable_value(&mut self.settings.output_device, name.clone(), name);
                }
            });
        ui.add_space(8.0);
        if quiet_button(ui, tr("Refresh list")) {
            self.input_devices = voice::list_input_devices();
            self.output_devices = voice::list_output_devices();
        }
        if devices_before != (self.settings.input_device.clone(), self.settings.output_device.clone()) {
            save_settings(&self.settings);
            self.sounds.open(&self.settings.output_device);
            if self.voice.is_some() { self.notice(tr("Device saved. Rejoin the voice channel for it to take effect.")); }
        }
        Self::hint(ui, tr("Your choice is saved immediately. After changing a device, leave the voice channel and join again."));

        Self::group(ui, tr("Volume and microphone"));
        Self::field(ui, tr("Playback volume"));
        let mut volume = f32::from_bits(self.volume.load(Ordering::Relaxed)).clamp(0.0, 2.0);
        let slider = ui.add(egui::Slider::new(&mut volume, 0.0..=2.0)
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
        if slider.changed() {
            self.volume.store(volume.to_bits(), Ordering::Relaxed);
        }
        if slider.drag_stopped() {
            self.settings.volume = volume;
            save_settings(&self.settings);
        }
        Self::field(ui, tr("Microphone level"));
        let level = self.voice.as_ref()
            .map(|v| f32::from_bits(v.microphone_level.load(Ordering::Relaxed)))
            .unwrap_or(0.0).clamp(0.0, 1.0);
        level_bar(ui, (level * 7.0).min(1.0), ui.available_width().min(420.0));
        if self.voice.is_none() {
            Self::hint(ui, tr("Join a voice channel to see the level."));
        }

        Self::group(ui, tr("Event sounds"));
        let mut sounds_on = self.settings.sounds_enabled;
        if ui.checkbox(&mut sounds_on, tr("Play sounds for the microphone, camera, screen sharing and people joining")).changed() {
            self.settings.sounds_enabled = sounds_on;
            self.sounds.enabled = sounds_on;
            save_settings(&self.settings);
        }
        if sounds_on {
            Self::field(ui, tr("Sounds volume"));
            let mut level = self.settings.sounds_volume;
            let slider = ui.add(egui::Slider::new(&mut level, 0.0..=1.0)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
            if slider.changed() {
                self.settings.sounds_volume = level;
                self.sounds.set_volume(level);
            }
            if slider.drag_stopped() {
                save_settings(&self.settings);
                self.sounds.play(sounds::Sound::MicOn);
            }
            ui.add_space(6.0);
            ui.label(text(tr("Listen"), 12.5, TEXT3));
            ui.horizontal_wrapped(|ui| {
                for (label, sound) in [
                    (tr("Microphone"), sounds::Sound::MicOn), (tr("Microphone off"), sounds::Sound::MicOff),
                    (tr("Camera"), sounds::Sound::CameraOn), (tr("Screen sharing"), sounds::Sound::ScreenOn),
                    (tr("You joined"), sounds::Sound::JoinSelf), (tr("Friend joined"), sounds::Sound::PeerJoin),
                    (tr("Friend left"), sounds::Sound::PeerLeave),
                ] {
                    if quiet_button(ui, label) { self.sounds.play(sound); }
                }
            });
            if let Some(error) = &self.sounds.error {
                ui.label(text(trf("Sounds are unavailable: {error}", &[("error", &error)]), 12.5, WARN));
            }
        }

        Self::group(ui, tr("When to transmit"));
        let mut ptt = self.push_to_talk.load(Ordering::Relaxed);
        if ui.checkbox(&mut ptt, tr("Talk only while a key is held (Push-to-Talk)")).changed() {
            self.push_to_talk.store(ptt, Ordering::Relaxed);
            self.settings.push_to_talk = ptt;
            save_settings(&self.settings);
        }
        Self::field(ui, tr("Key"));
        let key = self.ptt_key.load(Ordering::Relaxed);
        ui.horizontal(|ui| {
            if self.ptt_capture.is_some() {
                egui::Frame::none().fill(RAISED).stroke(Stroke::new(1.5_f32, ACCENT))
                    .rounding(Rounding::same(8.0)).inner_margin(Margin::symmetric(12.0, 7.0))
                    .show(ui, |ui| ui.label(text(tr("Press a key or mouse button…"), 14.0, ACCENT)));
                if quiet_button(ui, tr("Cancel")) { self.ptt_capture = None; }
            } else {
                egui::Frame::none().fill(RAISED).rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(12.0, 7.0))
                    .show(ui, |ui| ui.label(mono(voice::vk_name(key), 14.0, TEXT)));
                if primary_button(ui, tr("Change key"), true) { self.start_ptt_capture(); }
                if key != voice::PTT_VK && quiet_button(ui, tr("Reset to F8")) {
                    self.ptt_key.store(voice::PTT_VK, Ordering::Relaxed);
                    self.settings.ptt_key = voice::PTT_VK;
                    save_settings(&self.settings);
                }
            }
        });
        Self::hint(ui, tr("Any key or mouse button works, including side buttons. Esc cancels. F9 always toggles the microphone, even in games (inside the app Ctrl+M does the same)."));
        if let Some(voice) = &self.voice {
            let talking = voice.transmitting.load(Ordering::Relaxed);
            ui.add_space(4.0);
            ui.label(text(if talking { tr("You are transmitting") }
                else if ptt { tr("Hold the key to talk") } else { tr("Microphone is off") },
                12.5, if talking { GREEN } else { TEXT3 }));
        }

        Self::group(ui, tr("Background noise"));
        let mut gate = self.noise_gate.load(Ordering::Relaxed);
        if ui.checkbox(&mut gate, tr("Transmit only when you speak (Beta)")).changed() {
            self.noise_gate.store(gate, Ordering::Relaxed);
            self.settings.noise_gate = gate;
            save_settings(&self.settings);
        }
        if gate {
            let mut threshold = f32::from_bits(self.gate_threshold.load(Ordering::Relaxed)).clamp(0.003, 0.08);
            Self::field(ui, tr("Sensitivity"));
            if ui.add(egui::Slider::new(&mut threshold, 0.003..=0.08)
                .custom_formatter(|v, _| format!("{:.1}%", v * 100.0))).changed() {
                self.gate_threshold.store(threshold.to_bits(), Ordering::Relaxed);
                self.settings.gate_threshold = threshold;
                save_settings(&self.settings);
            }
            Self::hint(ui, tr("A lower value lets quiet sounds through, a higher one only loud speech. In Push-to-Talk mode the threshold has no effect."));
        }

        let in_call: Vec<(u64, String)> = self.peers.iter().filter(|p| p.voice)
            .map(|p| (p.id, p.name.clone())).collect();
        if let (Some(voice), false) = (&self.voice, in_call.is_empty()) {
            Self::group(ui, tr("Friends' volume"));
            for (id, name) in in_call {
                Self::field(ui, &name);
                let mut gain = voice.per_peer_volumes.lock().ok()
                    .and_then(|levels| levels.get(&id).copied()).unwrap_or(1.0);
                if ui.add(egui::Slider::new(&mut gain, 0.0..=2.0)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed() {
                    if let Ok(mut levels) = voice.per_peer_volumes.lock() {
                        levels.insert(id, gain);
                    }
                }
            }
        }

        if let Some(voice) = &self.voice {
            Self::group(ui, tr("Diagnostics"));
            let active = self.mesh.as_ref()
                .and_then(|mesh| mesh.voice_targets.lock().ok().map(|v| v.len())).unwrap_or(0);
            let sent = voice.packets_sent.load(Ordering::Relaxed);
            let received = voice.packets_received.load(Ordering::Relaxed);
            Self::hint(ui, trf("Friends in voice: {count}", &[("count", &active)]));
            Self::hint(ui, trf("UDP packets: sent {sent}, received {received}", &[("sent", &sent), ("received", &received)]));
            Self::hint(ui, trf("Recovered Opus frames: {recovered}, dropped late: {dropped}",
                &[("recovered", &voice.concealed_frames.load(Ordering::Relaxed)),
                  ("dropped", &voice.duplicate_packets.load(Ordering::Relaxed))]));
            let errors = voice.udp_errors.load(Ordering::Relaxed);
            if errors > 0 {
                ui.label(text(trf("UDP warnings: {count}", &[("count", &errors)]), 12.5, WARN));
            }
            if active > 0 && sent > 100 && received == 0 {
                ui.label(text(tr("No incoming audio. Check that UDP 45889 is open on your friends' computers."), 12.5, RED));
            } else {
                Self::hint(ui, tr("High values mean delays or packet loss in the VPN."));
            }
        }
    }

    pub(super) fn settings_video(&mut self, ui: &mut Ui) {
        let cameras = self.cameras.lock().map(|list| list.clone()).unwrap_or_default();
        let loading = self.cameras_loading.load(Ordering::Relaxed);
        Self::group(ui, tr("Camera"));
        let on = self.video.as_ref().is_some_and(|v| v.enabled());
        let starting = self.video.as_ref().is_some_and(|v| v.starting());

        // Live preview: only you see it until you are in a call.
        let width = ui.available_width().min(460.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, width / TILE_ASPECT), Sense::hover());
        let local_id = self.mesh.as_ref().map(|m| m.id);
        let texture = local_id.and_then(|id| self.video_textures.get(&(id, StreamKind::Camera)));
        let note = if starting { Some(tr("starting camera")) } else if !on { Some(tr("camera off")) } else { None };
        let nickname = self.settings.nickname.clone();
        let view = TileView { name: &nickname, note, speaking: false, badge: None, contain: false, small: false,
            salt: (local_id.unwrap_or(0), StreamKind::Camera) };
        let _ = paint_tile(ui, rect, &view, texture);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if on {
                if pill_button(ui, tr("Turn camera off"), RED, Color32::WHITE, Vec2::new(0.0, 34.0), true) {
                    self.toggle_camera();
                }
            } else if primary_button(ui, tr("Test camera"), self.mesh.is_some()) {
                self.toggle_camera();
            }
        });
        Self::hint(ui, if self.voice.is_some() {
            tr("You are in the voice channel: your friends see what you see here.")
        } else {
            tr("This preview is only for you. No video is sent to friends until you join the voice channel.")
        });

        Self::group(ui, tr("Device"));
        Self::field(ui, tr("Camera"));
        let previous = self.camera_index.load(Ordering::Relaxed);
        let mut chosen = previous;
        let current_name = cameras.iter().find(|(index, _)| *index == previous)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| if cameras.is_empty() {
                if loading { tr("Looking for cameras…").into() } else { tr("No cameras found").into() }
            } else { trf("Camera {number}", &[("number", &(previous + 1))]) });
        ui.add_enabled_ui(!cameras.is_empty(), |ui| {
            egui::ComboBox::from_id_salt("fv_cam_select")
                .width(ui.available_width().min(420.0))
                .selected_text(current_name)
                .show_ui(ui, |ui| {
                    for (index, name) in &cameras {
                        ui.selectable_value(&mut chosen, *index, name);
                    }
                });
        });
        if chosen != previous { self.update_camera_device(chosen); }
        ui.add_space(4.0);
        if quiet_button(ui, if loading { tr("Searching…") } else { tr("Refresh list") }) {
            self.refresh_cameras();
        }
        if cameras.is_empty() && !loading {
            Self::hint(ui, tr("Windows reports no camera. Plug one in and press “Refresh list”. Also check Windows settings: Privacy → Camera → allow desktop apps to access the camera."));
        } else {
            Self::hint(ui, tr("If the camera does not turn on, pick another one or close apps that use it (Zoom, Teams, a browser)."));
        }

        Self::group(ui, tr("Quality"));
        let original = self.video_quality.load(Ordering::Relaxed);
        let mut selected = original;
        let label = |q: u32| match q {
            0 => tr("Saver · 320×180, 6 fps"),
            1 => tr("Standard · 480×270, 10 fps"),
            2 => tr("Improved · 640×360, 12 fps"),
            3 => tr("HD · 960×540, 15 fps"),
            4 => tr("Smooth HD · 1280×720, 30 fps"),
            _ => tr("Maximum · 1920×1080, 30 fps"),
        };
        // Typical bit rate per friend for each level, in Mbit/s.
        let traffic_mbps = |q: u32| -> f32 { match q { 0 => 0.3, 1 => 1.2, 2 | 4 => 3.0, _ => 6.0 } };
        egui::ComboBox::from_id_salt("fv_video_quality").width(300.0).selected_text(label(original))
            .show_ui(ui, |ui| {
                for q in [5u32, 4, 3, 2, 1, 0] { ui.selectable_value(&mut selected, q, label(q)); }
            });
        if selected != original { self.update_video_quality(selected); }
        let friends = self.peers.iter().filter(|p| p.voice).count().max(1);
        let per_friend = traffic_mbps(original);
        Self::hint(ui, trf(
            "Traffic per friend: ≈{each} Mbit/s. Friends in the call: {friends}, so you will send about {total} Mbit/s. The top two levels use the H.264 video codec: better quality for the same traffic. Everyone in the call should use the same version of the app.",
            &[("each", &decimal(per_friend, 1)), ("friends", &friends),
              ("total", &decimal(per_friend * friends as f32, 1))]));
        if let Some(video) = &self.video {
            ui.add_space(6.0);
            let info = video.diagnostics.format.lock().map(|f| f.clone()).unwrap_or_default();
            let captured = video.diagnostics.captured.load(Ordering::Relaxed);
            let failed = video.diagnostics.decode_errors.load(Ordering::Relaxed);
            if !info.is_empty() {
                Self::hint(ui, trf("Camera mode: {info}. Frames from the camera: {captured}, not decoded: {failed}.", &[("info", &info), ("captured", &captured), ("failed", &failed)]));
            }
            if video.stalled() {
                ui.label(text(tr("The camera delivers no frames. Pick another camera, close apps that use it, and check the permission in Windows settings."), 12.5, RED));
            } else if failed > 0 && failed >= captured {
                ui.label(text(tr("The camera frames cannot be decoded. Try another camera."), 12.5, RED));
            }
            Self::hint(ui, trf("Frames: sent {sent}, received {received}, recovered after loss {recovered}",
                &[("sent", &video.frames_sent.load(Ordering::Relaxed)),
                  ("received", &(video.frames_received.load(Ordering::Relaxed) + video.stream_frames_received())),
                  ("recovered", &video.frames_recovered.load(Ordering::Relaxed))]));
        }

        Self::group(ui, tr("Screen sharing"));
        Self::hint(ui, tr("Press the monitor button in the call bar: you can share the whole screen or one window. You choose resolution and frame rate in the same window; your choice is remembered."));
    }

    pub(super) fn settings_profile(&mut self, ui: &mut Ui) {
        Self::group(ui, tr("Language"));
        let languages: Vec<String> = Lang::ALL.iter().map(|l| l.native_name().to_string()).collect();
        let current = Lang::ALL.iter().position(|l| *l == i18n::language()).unwrap_or(0);
        if let Some(index) = segmented(ui, &languages, current) { self.set_language(Lang::ALL[index]); }
        Self::hint(ui, tr("The interface language changes at once. English is the default."));

        Self::group(ui, tr("Your name"));
        ui.horizontal(|ui| {
            avatar(ui, &self.settings.nickname, 44.0, false);
            ui.vertical(|ui| {
                let field = ui.add(egui::TextEdit::singleline(&mut self.nickname_draft)
                    .margin(Margin::symmetric(10.0, 8.0)).desired_width(260.0).char_limit(24));
                // Apply once the user is done typing, not on every letter.
                let done = field.lost_focus();
                if done {
                    let name = self.nickname_draft.trim().to_string();
                    if name.is_empty() {
                        self.nickname_draft = self.settings.nickname.clone();
                    } else if name != self.settings.nickname {
                        self.settings.nickname = name.clone();
                        self.nickname_draft = name.clone();
                        if let Some(mesh) = &self.mesh { mesh.rename(name); }
                        save_settings(&self.settings);
                    }
                }
            });
        });
        Self::hint(ui, tr("This is how your friends see you. The name changes when you press Enter or click outside the field."));

        Self::group(ui, tr("Saved friends"));
        Self::hint(ui, trf("Friend addresses saved on this computer: {count}. They help friends connect faster and are never sent anywhere.",
            &[("count", &self.settings.known_ips.len())]));
        if !self.settings.known_ips.is_empty() && quiet_button(ui, tr("Forget saved addresses")) {
            self.settings.known_ips.clear();
            save_settings(&self.settings);
            self.notice(tr("Saved friend addresses deleted."));
        }
    }

    pub(super) fn settings_appearance(&mut self, ui: &mut Ui) {
        Self::group(ui, tr("Panels"));
        let mut sidebar = self.sidebar_open;
        if ui.checkbox(&mut sidebar, tr("Show the left panel")).changed() { self.sidebar_open = sidebar; }
        let mut theatre = self.focus_mode;
        if ui.checkbox(&mut theatre, tr("Theatre mode: hide the chat, give video more room")).changed() {
            self.focus_mode = theatre;
            if !theatre { self.chat_unread = 0; }
        }
        ui.add_space(4.0);
        if quiet_button(ui, tr("Restore panels")) {
            self.sidebar_open = true;
            self.focus_mode = false;
            self.chat_search_open = false;
            self.chat_unread = 0;
            self.chat_search.clear();
            self.notice(tr("Panels restored."));
        }

        Self::group(ui, tr("Interface size"));
        const SCALES: [f32; 5] = [0.9, 1.0, 1.15, 1.3, 1.5];
        let labels: Vec<String> = SCALES.iter().map(|s| format!("{:.0}%", s * 100.0)).collect();
        let current = SCALES.iter().position(|s| (s - self.settings.ui_scale).abs() < 0.01).unwrap_or(1);
        if let Some(index) = segmented(ui, &labels, current) {
            self.settings.ui_scale = SCALES[index];
            save_settings(&self.settings);
        }
        Self::hint(ui, tr("Makes text and buttons larger or smaller. Useful on large or very sharp screens."));

        Self::group(ui, tr("Keyboard shortcuts"));
        for (keys, action) in [
            ("Ctrl + M", tr("Turn the microphone on or off")),
            ("F9", tr("The same, from any window, even in a game")),
            ("Ctrl + B", tr("Hide or show the left panel")),
            ("Ctrl + E", tr("Theatre mode")),
            ("Ctrl + F", tr("Search chat")),
            ("Esc", tr("Close search, settings or the picker")),
        ] {
            ui.horizontal(|ui| {
                ui.allocate_ui(Vec2::new(96.0, 22.0), |ui| ui.label(mono(keys, 12.5, TEXT)));
                ui.label(text(action, 13.0, TEXT2));
            });
        }
        Self::hint(ui, tr("Choose the Push-to-Talk key in the “Sound” section."));
    }

    pub(super) fn settings_about(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            brand_mark(ui, 72.0);
            ui.vertical(|ui| {
                ui.label(strong("Friends Voice", 17.0, TEXT));
                ui.label(text(trf("Version {version}", &[("version", &env!("CARGO_PKG_VERSION"))]), 12.5, TEXT3));
            });
        });
        Self::group(ui, tr("What is this"));
        Self::hint(ui, tr("Private voice rooms for 2–5 friends in a shared Radmin VPN network. Voice, camera and chat go directly between computers, with no server. When one person leaves, the others keep talking."));
        Self::group(ui, tr("Security"));
        Self::hint(ui, tr("Communication is protected only by the private VPN network. There is no end-to-end encryption or identity check for friends. Do not expose UDP ports 45888–45891 to the internet."));
        Self::group(ui, tr("Help us fix problems"));
        Self::hint(ui, tr("If the app misbehaves or closes by itself, copy the report and send it together with a description. Crashes are also written to a log file."));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if quiet_button(ui, tr("Copy report")) {
                let report = self.diagnostic_report();
                ui.output_mut(|o| o.copied_text = report);
                self.notice(tr("The report is copied to the clipboard."));
            }
            if quiet_button(ui, tr("Open logs folder")) { self.open_logs_folder(); }
        });

        Self::group(ui, tr("Feature status"));
        Self::hint(ui, tr("Voice, chat, Push-to-Talk, the HD camera and screen sharing work. Echo cancellation is not implemented, and computer sound is not shared during screen sharing."));
    }

}

impl FriendsApp {
    /// A short report for bug reports: versions, devices and the state of the network.
    pub(super) fn diagnostic_report(&self) -> String {
        let ips = self.mesh.as_ref().map(|m| m.local_radmin_ips.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        let cameras = self.cameras.lock().map(|c| c.iter().map(|(i, n)| format!("{i}: {n}")).collect::<Vec<_>>().join("; ")).unwrap_or_default();
        format!(
            "Friends Voice {}\nOS: {} {}\nLanguage: {}\nNetwork: {} (addresses: {})\nFriends online: {}, in voice: {}\nIn voice: {}\n\
             Input device: {}\nOutput device: {}\nPush-to-Talk: {} ({})\nCamera index: {}, quality: {}\nCameras: {}\n\
             Screen sharing: {}\nSounds: {} (volume {:.0}%){}\nUI scale: {:.0}%\n",
            env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH, self.settings.language,
            if self.mesh.is_some() { "running" } else { "not running" }, ips,
            self.peers.len(), self.peers.iter().filter(|p| p.voice).count(), self.voice.is_some(),
            if self.settings.input_device.is_empty() { "default" } else { &self.settings.input_device },
            if self.settings.output_device.is_empty() { "default" } else { &self.settings.output_device },
            self.push_to_talk.load(Ordering::Relaxed), voice::vk_name(self.ptt_key.load(Ordering::Relaxed)),
            self.camera_index.load(Ordering::Relaxed), self.video_quality.load(Ordering::Relaxed), cameras,
            self.screen.is_some(), self.settings.sounds_enabled, self.settings.sounds_volume * 100.0,
            self.sounds.error.as_ref().map(|e| format!(", error: {e}")).unwrap_or_default(),
            self.settings.ui_scale * 100.0,
        )
    }

    pub(super) fn open_logs_folder(&mut self) {
        let Some(dir) = diagnostics::log_dir() else { return; };
        let _ = std::fs::create_dir_all(&dir);
        #[cfg(windows)]
        { let _ = std::process::Command::new("explorer").arg(&dir).spawn(); }
        #[cfg(not(windows))]
        { let _ = dir; }
    }
}
