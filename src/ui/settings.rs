//! Settings pages.
use super::*;

impl FriendsApp {
    pub(super) fn settings_nav(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(strong("Налаштування", 15.5, TEXT));
        });
        ui.add_space(12.0);
        for (index, (glyph, label)) in [
            (ph::USER_CIRCLE, "Профіль"),
            (ph::MICROPHONE, "Звук"),
            (ph::VIDEO_CAMERA, "Відео й показ екрана"),
            (ph::SIDEBAR_SIMPLE, "Вигляд і клавіші"),
            (ph::INFO, "Про програму"),
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
                let title = ["Профіль", "Звук", "Відео й показ екрана", "Вигляд і клавіші", "Про програму"][self.settings_page.min(4)];
                ui.horizontal(|ui| {
                    ui.label(strong(title, 22.0, TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, ph::X, "Закрити налаштування · Esc", false, 34.0) {
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
        Self::group(ui, "Пристрої");
        Self::field(ui, "Мікрофон");
        egui::ComboBox::from_id_salt("fv_input_device")
            .width(ui.available_width().min(420.0))
            .selected_text(if self.settings.input_device.is_empty() { "За замовчуванням" } else { &self.settings.input_device })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.input_device, String::new(), "За замовчуванням");
                for name in &self.input_devices {
                    ui.selectable_value(&mut self.settings.input_device, name.clone(), name);
                }
            });
        Self::field(ui, "Навушники або динаміки");
        egui::ComboBox::from_id_salt("fv_output_device")
            .width(ui.available_width().min(420.0))
            .selected_text(if self.settings.output_device.is_empty() { "За замовчуванням" } else { &self.settings.output_device })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.settings.output_device, String::new(), "За замовчуванням");
                for name in &self.output_devices {
                    ui.selectable_value(&mut self.settings.output_device, name.clone(), name);
                }
            });
        ui.add_space(8.0);
        if quiet_button(ui, "Оновити список") {
            self.input_devices = voice::list_input_devices();
            self.output_devices = voice::list_output_devices();
        }
        if devices_before != (self.settings.input_device.clone(), self.settings.output_device.clone()) {
            save_settings(&self.settings);
            self.sounds.open(&self.settings.output_device);
            if self.voice.is_some() { self.notice("Пристрій збережено. Перезайди у голосовий канал, щоб він почав працювати."); }
        }
        Self::hint(ui, "Вибір зберігається одразу. Після зміни пристрою вийди з голосового каналу й зайди знову.");

        Self::group(ui, "Гучність і мікрофон");
        Self::field(ui, "Гучність відтворення");
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
        Self::field(ui, "Рівень мікрофона");
        let level = self.voice.as_ref()
            .map(|v| f32::from_bits(v.microphone_level.load(Ordering::Relaxed)))
            .unwrap_or(0.0).clamp(0.0, 1.0);
        level_bar(ui, (level * 7.0).min(1.0), ui.available_width().min(420.0));
        if self.voice.is_none() {
            Self::hint(ui, "Зайди в голосовий канал, щоб побачити рівень.");
        }

        Self::group(ui, "Звуки подій");
        let mut sounds_on = self.settings.sounds_enabled;
        if ui.checkbox(&mut sounds_on, "Грати звуки для мікрофона, камери, показу екрана та входу в канал").changed() {
            self.settings.sounds_enabled = sounds_on;
            self.sounds.enabled = sounds_on;
            save_settings(&self.settings);
        }
        if sounds_on {
            Self::field(ui, "Гучність звуків");
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
            ui.label(text("Прослухати", 12.5, TEXT3));
            ui.horizontal_wrapped(|ui| {
                for (label, sound) in [
                    ("Мікрофон", sounds::Sound::MicOn), ("Вимкнути мікрофон", sounds::Sound::MicOff),
                    ("Камера", sounds::Sound::CameraOn), ("Показ екрана", sounds::Sound::ScreenOn),
                    ("Ти зайшов", sounds::Sound::JoinSelf), ("Друг зайшов", sounds::Sound::PeerJoin),
                    ("Друг вийшов", sounds::Sound::PeerLeave),
                ] {
                    if quiet_button(ui, label) { self.sounds.play(sound); }
                }
            });
            if let Some(error) = &self.sounds.error {
                ui.label(text(format!("Звуки недоступні: {error}"), 12.5, WARN));
            }
        }

        Self::group(ui, "Коли передавати голос");
        let mut ptt = self.push_to_talk.load(Ordering::Relaxed);
        if ui.checkbox(&mut ptt, "Говорити лише при натиснутій клавіші (Push-to-Talk)").changed() {
            self.push_to_talk.store(ptt, Ordering::Relaxed);
            self.settings.push_to_talk = ptt;
            save_settings(&self.settings);
        }
        Self::field(ui, "Клавіша");
        let key = self.ptt_key.load(Ordering::Relaxed);
        ui.horizontal(|ui| {
            if self.ptt_capture.is_some() {
                egui::Frame::none().fill(RAISED).stroke(Stroke::new(1.5_f32, ACCENT))
                    .rounding(Rounding::same(8.0)).inner_margin(Margin::symmetric(12.0, 7.0))
                    .show(ui, |ui| ui.label(text("Натисни клавішу або кнопку миші…", 14.0, ACCENT)));
                if quiet_button(ui, "Скасувати") { self.ptt_capture = None; }
            } else {
                egui::Frame::none().fill(RAISED).rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(12.0, 7.0))
                    .show(ui, |ui| ui.label(mono(voice::vk_name(key), 14.0, TEXT)));
                if primary_button(ui, "Змінити клавішу", true) { self.start_ptt_capture(); }
                if key != voice::PTT_VK && quiet_button(ui, "Повернути F8") {
                    self.ptt_key.store(voice::PTT_VK, Ordering::Relaxed);
                    self.settings.ptt_key = voice::PTT_VK;
                    save_settings(&self.settings);
                }
            }
        });
        Self::hint(ui, "Підходить будь-яка клавіша чи кнопка миші, зокрема бічні. Esc скасовує вибір. Клавіша F9 завжди вмикає й вимикає мікрофон, навіть у грі (в програмі те саме робить Ctrl+M).");
        if let Some(voice) = &self.voice {
            let talking = voice.transmitting.load(Ordering::Relaxed);
            ui.add_space(4.0);
            ui.label(text(if talking { "Зараз передаєш голос" }
                else if ptt { "Тримай клавішу, щоб говорити" } else { "Мікрофон вимкнено" },
                12.5, if talking { GREEN } else { TEXT3 }));
        }

        Self::group(ui, "Фоновий шум");
        let mut gate = self.noise_gate.load(Ordering::Relaxed);
        if ui.checkbox(&mut gate, "Передавати лише коли ти говориш (Beta)").changed() {
            self.noise_gate.store(gate, Ordering::Relaxed);
            self.settings.noise_gate = gate;
            save_settings(&self.settings);
        }
        if gate {
            let mut threshold = f32::from_bits(self.gate_threshold.load(Ordering::Relaxed)).clamp(0.003, 0.08);
            Self::field(ui, "Чутливість");
            if ui.add(egui::Slider::new(&mut threshold, 0.003..=0.08)
                .custom_formatter(|v, _| format!("{:.1}%", v * 100.0))).changed() {
                self.gate_threshold.store(threshold.to_bits(), Ordering::Relaxed);
                self.settings.gate_threshold = threshold;
                save_settings(&self.settings);
            }
            Self::hint(ui, "Менше значення пропускає тихі звуки, більше лише гучну мову. У режимі Push-to-Talk поріг не діє.");
        }

        let in_call: Vec<(u64, String)> = self.peers.iter().filter(|p| p.voice)
            .map(|p| (p.id, p.name.clone())).collect();
        if let (Some(voice), false) = (&self.voice, in_call.is_empty()) {
            Self::group(ui, "Гучність друзів");
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
            Self::group(ui, "Діагностика");
            let active = self.mesh.as_ref()
                .and_then(|mesh| mesh.voice_targets.lock().ok().map(|v| v.len())).unwrap_or(0);
            let sent = voice.packets_sent.load(Ordering::Relaxed);
            let received = voice.packets_received.load(Ordering::Relaxed);
            Self::hint(ui, format!("Друзів у голосі: {active}"));
            Self::hint(ui, format!("UDP-пакети: надіслано {sent}, отримано {received}"));
            Self::hint(ui, format!("Відновлено Opus-кадрів: {}, відкинуто запізнілих: {}",
                voice.concealed_frames.load(Ordering::Relaxed),
                voice.duplicate_packets.load(Ordering::Relaxed)));
            let errors = voice.udp_errors.load(Ordering::Relaxed);
            if errors > 0 {
                ui.label(text(format!("Попереджень UDP: {errors}"), 12.5, WARN));
            }
            if active > 0 && sent > 100 && received == 0 {
                ui.label(text("Вхідного звуку немає. Перевір, чи відкритий UDP 45889 у друзів.", 12.5, RED));
            } else {
                Self::hint(ui, "Високі значення означають затримки або втрату пакетів у VPN.");
            }
        }
    }

    pub(super) fn settings_video(&mut self, ui: &mut Ui) {
        let cameras = self.cameras.lock().map(|list| list.clone()).unwrap_or_default();
        let loading = self.cameras_loading.load(Ordering::Relaxed);
        Self::group(ui, "Камера");
        let on = self.video.as_ref().is_some_and(|v| v.enabled());
        let starting = self.video.as_ref().is_some_and(|v| v.starting());

        // Live preview: only you see it until you are in a call.
        let width = ui.available_width().min(460.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, width / TILE_ASPECT), Sense::hover());
        let local_id = self.mesh.as_ref().map(|m| m.id);
        let texture = local_id.and_then(|id| self.video_textures.get(&(id, StreamKind::Camera)));
        let note = if starting { Some("запуск камери") } else if !on { Some("камеру вимкнено") } else { None };
        let nickname = self.settings.nickname.clone();
        let view = TileView { name: &nickname, note, speaking: false, badge: None, contain: false, small: false,
            salt: (local_id.unwrap_or(0), StreamKind::Camera) };
        let _ = paint_tile(ui, rect, &view, texture);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if on {
                if pill_button(ui, "Вимкнути камеру", RED, Color32::WHITE, Vec2::new(0.0, 34.0), true) {
                    self.toggle_camera();
                }
            } else if primary_button(ui, "Перевірити камеру", self.mesh.is_some()) {
                self.toggle_camera();
            }
        });
        Self::hint(ui, if self.voice.is_some() {
            "Ти в голосовому каналі: друзі бачать те, що ти бачиш тут."
        } else {
            "Це перегляд лише для тебе. Друзям відео не надсилається, поки ти не зайдеш у голосовий канал."
        });

        Self::group(ui, "Пристрій");
        Self::field(ui, "Камера");
        let previous = self.camera_index.load(Ordering::Relaxed);
        let mut chosen = previous;
        let current_name = cameras.iter().find(|(index, _)| *index == previous)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| if cameras.is_empty() {
                if loading { "Пошук камер…".into() } else { "Камер не знайдено".into() }
            } else { format!("Камера {}", previous + 1) });
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
        if quiet_button(ui, if loading { "Шукаю…" } else { "Оновити список" }) {
            self.refresh_cameras();
        }
        if cameras.is_empty() && !loading {
            Self::hint(ui, "Windows не повідомила про жодну камеру. Підключи її та натисни «Оновити список». Також перевір у Параметрах Windows: Конфіденційність → Камера → дозволити доступ класичним програмам.");
        } else {
            Self::hint(ui, "Якщо камера не вмикається, вибери іншу або закрий програми, що її займають (Zoom, Teams, браузер).");
        }

        Self::group(ui, "Якість");
        let original = self.video_quality.load(Ordering::Relaxed);
        let mut selected = original;
        let label = |q: u32| match q {
            0 => "Економія · 320×180, 6 к/с",
            1 => "Звичайна · 480×270, 10 к/с",
            2 => "Покращена · 640×360, 12 к/с",
            3 => "HD · 960×540, 15 к/с",
            4 => "Плавна HD · 1280×720, 30 к/с",
            _ => "Максимум · 1920×1080, 30 к/с",
        };
        let traffic = |q: u32| match q {
            0 => "≈0,3 Мбіт/с", 1 => "≈1,2 Мбіт/с", 2 => "≈3 Мбіт/с", 3 => "≈6 Мбіт/с",
            4 => "≈3 Мбіт/с", _ => "≈6 Мбіт/с",
        };
        egui::ComboBox::from_id_salt("fv_video_quality").width(300.0).selected_text(label(original))
            .show_ui(ui, |ui| {
                for q in [5u32, 4, 3, 2, 1, 0] { ui.selectable_value(&mut selected, q, label(q)); }
            });
        if selected != original { self.update_video_quality(selected); }
        let friends = self.peers.iter().filter(|p| p.voice).count().max(1);
        Self::hint(ui, format!(
            "Трафік на одного друга: {}. Зараз у дзвінку друзів: {}, тож відправка складе приблизно {} Мбіт/с. Два верхні рівні використовують відеокодек H.264: якість краща за той самий трафік. Усі в дзвінку мають користуватися однією версією програми.",
            traffic(original), friends,
            { let one: f32 = match original { 0 => 0.3, 1 => 1.2, 2 | 4 => 3.0, _ => 6.0 };
              format!("{:.1}", one * friends as f32).replace('.', ",") }));
        if let Some(video) = &self.video {
            ui.add_space(6.0);
            let info = video.diagnostics.format.lock().map(|f| f.clone()).unwrap_or_default();
            let captured = video.diagnostics.captured.load(Ordering::Relaxed);
            let failed = video.diagnostics.decode_errors.load(Ordering::Relaxed);
            if !info.is_empty() {
                Self::hint(ui, format!("Режим камери: {info}. Кадрів з камери: {captured}, не розпаковано: {failed}."));
            }
            if video.stalled() {
                ui.label(text("Камера не віддає кадри. Вибери іншу камеру, закрий програми, що її займають, і перевір дозвіл у Параметрах Windows.", 12.5, RED));
            } else if failed > 0 && failed >= captured {
                ui.label(text("Кадри з камери не вдається розпакувати. Спробуй іншу камеру.", 12.5, RED));
            }
            Self::hint(ui, format!("Кадрів: надіслано {}, отримано {}, відновлено після втрат {}",
                video.frames_sent.load(Ordering::Relaxed),
                video.frames_received.load(Ordering::Relaxed) + video.stream_frames_received(),
                video.frames_recovered.load(Ordering::Relaxed)));
        }

        Self::group(ui, "Показ екрана");
        Self::hint(ui, "Натисни кнопку з монітором у панелі дзвінка: можна показати весь екран або одне вікно. Роздільність і частоту кадрів обираєш у тому ж вікні, вибір запам'ятовується.");
    }

    pub(super) fn settings_profile(&mut self, ui: &mut Ui) {
        Self::group(ui, "Твоє ім'я");
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
        Self::hint(ui, "Так тебе бачать друзі. Ім'я змінюється, коли натискаєш Enter або клікаєш поза полем.");

        Self::group(ui, "Збережені друзі");
        Self::hint(ui, format!("Адрес друзів на цьому комп'ютері: {}. Вони потрібні, щоб друзі знаходилися швидше, і нікуди не передаються.",
            self.settings.known_ips.len()));
        if !self.settings.known_ips.is_empty() && quiet_button(ui, "Забути збережені адреси") {
            self.settings.known_ips.clear();
            save_settings(&self.settings);
            self.notice("Збережені адреси друзів видалено.");
        }
    }

    pub(super) fn settings_appearance(&mut self, ui: &mut Ui) {
        Self::group(ui, "Панелі");
        let mut sidebar = self.sidebar_open;
        if ui.checkbox(&mut sidebar, "Показувати ліву панель").changed() { self.sidebar_open = sidebar; }
        let mut theatre = self.focus_mode;
        if ui.checkbox(&mut theatre, "Театральний режим: сховати чат, дати більше місця відео").changed() {
            self.focus_mode = theatre;
            if !theatre { self.chat_unread = 0; }
        }
        ui.add_space(4.0);
        if quiet_button(ui, "Показати все, як було") {
            self.sidebar_open = true;
            self.focus_mode = false;
            self.chat_search_open = false;
            self.chat_unread = 0;
            self.chat_search.clear();
            self.notice("Панелі повернуто, як було.");
        }

        Self::group(ui, "Гарячі клавіші");
        for (keys, action) in [
            ("Ctrl + M", "Увімкнути чи вимкнути мікрофон"),
            ("F9", "Те саме, але з будь-якого вікна, навіть у грі"),
            ("Ctrl + B", "Сховати чи показати ліву панель"),
            ("Ctrl + E", "Театральний режим"),
            ("Ctrl + F", "Пошук у чаті"),
            ("Esc", "Закрити пошук, налаштування чи вікно вибору"),
        ] {
            ui.horizontal(|ui| {
                ui.add_sized([96.0, 22.0], egui::Label::new(mono(keys, 12.5, TEXT)));
                ui.label(text(action, 13.0, TEXT2));
            });
        }
        Self::hint(ui, "Клавішу для Push-to-Talk обирають у розділі «Звук».");
    }

    pub(super) fn settings_about(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            brand_mark(ui, 72.0);
            ui.vertical(|ui| {
                ui.label(strong("Friends Voice", 17.0, TEXT));
                ui.label(text(format!("Версія {}", env!("CARGO_PKG_VERSION")), 12.5, TEXT3));
            });
        });
        Self::group(ui, "Що це");
        Self::hint(ui, "Приватні голосові кімнати для 2–5 друзів у спільній мережі Radmin VPN. Голос, камера й чат ідуть напряму між комп'ютерами, без сервера. Вихід одного учасника не перериває розмову решти.");
        Self::group(ui, "Безпека");
        Self::hint(ui, "Спілкування захищене лише приватною VPN-мережею. Власного наскрізного шифрування й перевірки особи друга немає. Не відкривай порти UDP 45888–45890 в інтернет.");
        Self::group(ui, "Статус функцій");
        Self::hint(ui, "Голос, чат і Push-to-Talk працюють. Камера в Beta (JPEG через UDP). Демонстрація екрана й усунення луни ще не реалізовані.");
    }

}
