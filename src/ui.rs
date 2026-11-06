//! Native egui desktop design. Every interactive element is wired to the existing P2P client.
//! Camera and screen-share controls are visibly unavailable until their codecs exist.
use super::*;
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2};

const SURFACE: Color32 = Color32::from_rgb(27, 30, 49);
const SURFACE_2: Color32 = Color32::from_rgb(34, 38, 62);
const BORDER: Color32 = Color32::from_rgb(66, 71, 111);
const BRIGHT: Color32 = Color32::from_rgb(239, 243, 255);
const PURPLE: Color32 = Color32::from_rgb(139, 93, 255);
const BLUE: Color32 = Color32::from_rgb(60, 183, 255);
const SOFT: Color32 = Color32::from_rgb(184, 192, 220);

#[derive(Clone, Copy)]
enum Glyph {
    Mic, Headphones, Camera, Monitor, Settings, Send, Power,
    Users, Network, Volume, Search, Close, Shield, Chat,
}
fn card() -> egui::Frame {
    egui::Frame::none()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER.linear_multiply(0.7)))
        .rounding(Rounding::same(15.0))
        .inner_margin(egui::Margin::same(14.0))
}
fn soft_card() -> egui::Frame {
    egui::Frame::none()
        .fill(SURFACE_2)
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::same(11.0))
}
fn text_title(text: impl Into<String>) -> RichText {
    RichText::new(text).size(18.0).strong().color(BRIGHT)
}
fn text_caption(text: impl Into<String>) -> RichText {
    RichText::new(text).size(11.0).color(MUTED)
}
fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(11.0).strong().color(SOFT));
    ui.add_space(4.0);
}
fn tinted_button(ui: &mut Ui, label: &str, accent: Color32, enabled: bool, height: f32) -> Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).color(BRIGHT).strong().size(13.0))
            .fill(accent)
            .stroke(Stroke::new(1.0, accent.linear_multiply(1.3)))
            .rounding(Rounding::same(11.0))
            .min_size(Vec2::new(ui.available_width(), height))
    )
}
fn outline_button(ui: &mut Ui, label: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).color(SOFT).size(12.0))
            .fill(SURFACE_2)
            .stroke(Stroke::new(1.0, BORDER))
            .rounding(Rounding::same(10.0))
    ).clicked()
}
fn small_chip(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::none()
        .fill(color.linear_multiply(0.20))
        .stroke(Stroke::new(1.0, color.linear_multiply(0.64)))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::symmetric(10.0, 5.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(color));
        });
}
fn dot(ui: &mut Ui, color: Color32, radius: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(radius * 2.0 + 5.0), Sense::hover());
    let center = r.center();
    ui.painter().circle_filled(center, radius + 3.0, color.linear_multiply(0.14));
    ui.painter().circle_filled(center, radius, color);
}
fn avatar(ui: &mut Ui, label: &str, diameter: f32, color: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::hover());
    let c = r.center();
    ui.painter().circle_filled(c, diameter * 0.48, color.linear_multiply(0.22));
    ui.painter().circle_stroke(c, diameter * 0.47, Stroke::new(1.4, color.linear_multiply(0.7)));
    let initials: String = label.chars().take(2).collect::<String>().to_uppercase();
    ui.painter().text(c, Align2::CENTER_CENTER, initials,
        FontId::proportional((diameter * 0.34).min(23.0)), BRIGHT);
}
fn stroke_line(ui: &Ui, a: Pos2, b: Pos2, color: Color32, width: f32) {
    ui.painter().line_segment([a,b], Stroke::new(width,color));
}
fn glyph(ui: &Ui, icon: Glyph, r: Rect, color: Color32) {
    let x = r.center().x;
    let y = r.center().y;
    let at = |dx: f32, dy: f32| Pos2::new(x+dx,y+dy);
    let line = |a: (f32,f32), b: (f32,f32)| {
        stroke_line(ui, at(a.0,a.1), at(b.0,b.1), color, 1.9);
    };
    match icon {
        Glyph::Mic => {
            ui.painter().rect_stroke(Rect::from_center_size(at(0.0,-3.0), Vec2::new(7.0,13.0)),
                Rounding::same(3.5), Stroke::new(1.8,color));
            line((-7.0,0.0),(-7.0,3.0)); line((7.0,0.0),(7.0,3.0));
            line((-7.0,3.0),(-4.0,7.0)); line((7.0,3.0),(4.0,7.0));
            line((-4.0,7.0),(4.0,7.0)); line((0.0,7.0),(0.0,11.0));
            line((-5.0,11.0),(5.0,11.0));
        }
        Glyph::Headphones => {
            for i in 0..12 {
                let a = std::f32::consts::PI + i as f32 * std::f32::consts::PI / 11.0;
                let b = std::f32::consts::PI + (i+1) as f32 * std::f32::consts::PI / 11.0;
                // Upper half of a circle (mathematical y positive is down in egui).
                line((9.0*a.cos(), 9.0*a.sin()+1.0), (9.0*b.cos(),9.0*b.sin()+1.0));
            }
            ui.painter().rect_filled(Rect::from_center_size(at(-8.0,4.0), Vec2::new(4.0,10.0)),
                Rounding::same(2.0), color);
            ui.painter().rect_filled(Rect::from_center_size(at(8.0,4.0), Vec2::new(4.0,10.0)),
                Rounding::same(2.0), color);
        }
        Glyph::Camera => {
            ui.painter().rect_stroke(Rect::from_min_max(at(-10.0,-7.0),at(5.0,7.0)),
                Rounding::same(3.0), Stroke::new(1.8,color));
            line((6.0,-3.0),(11.0,-6.0)); line((11.0,-6.0),(11.0,6.0));
            line((11.0,6.0),(6.0,3.0));
        }
        Glyph::Monitor => {
            ui.painter().rect_stroke(Rect::from_min_max(at(-11.0,-8.0),at(11.0,5.0)),
                Rounding::same(2.0), Stroke::new(1.8,color));
            line((0.0,5.0),(0.0,10.0)); line((-6.0,10.0),(6.0,10.0));
        }
        Glyph::Settings => {
            ui.painter().circle_stroke(at(0.0,0.0),6.5,Stroke::new(1.9,color));
            ui.painter().circle_filled(at(0.0,0.0),2.2,color);
            for i in 0..8 {
                let a=i as f32*std::f32::consts::TAU/8.0;
                line((a.cos()*9.0,a.sin()*9.0),(a.cos()*12.0,a.sin()*12.0));
            }
        }
        Glyph::Send => {
            line((-11.0,-7.0),(11.0,0.0)); line((11.0,0.0),(-11.0,8.0));
            line((-11.0,8.0),(-5.0,1.0)); line((-5.0,1.0),(11.0,0.0));
            line((-5.0,1.0),(-11.0,-7.0));
        }
        Glyph::Power => {
            ui.painter().circle_stroke(at(0.0,2.0),8.0,Stroke::new(1.9,color));
            stroke_line(ui,at(0.0,-10.0),at(0.0,1.0),color,2.8);
        }
        Glyph::Users => {
            ui.painter().circle_stroke(at(-5.0,-5.0),4.1,Stroke::new(1.7,color));
            ui.painter().circle_stroke(at(6.0,-3.0),3.2,Stroke::new(1.5,color));
            line((-12.0,10.0),(-12.0,6.0)); line((-12.0,6.0),(-8.0,2.0));
            line((-8.0,2.0),(-2.0,2.0)); line((-2.0,2.0),(3.0,6.0));
            line((3.0,6.0),(3.0,10.0)); line((5.0,3.0),(10.0,3.0));
            line((10.0,3.0),(13.0,7.0));
        }
        Glyph::Network => {
            for (dx,dy) in [(-9.0,4.0),(0.0,-7.0),(9.0,4.0)] {
                ui.painter().circle_filled(at(dx,dy),3.0,color);
            }
            line((-7.0,2.0),(-1.0,-5.0)); line((1.0,-5.0),(7.0,2.0));
            line((-6.0,4.0),(6.0,4.0));
        }
        Glyph::Volume => {
            ui.painter().rect_filled(Rect::from_min_max(at(-10.0,-4.0),at(-6.0,4.0)),
                Rounding::same(1.0),color);
            line((-6.0,-4.0),(0.0,-9.0)); line((-6.0,4.0),(0.0,9.0));
            line((0.0,-9.0),(0.0,9.0)); line((4.0,-5.0),(7.0,0.0)); line((7.0,0.0),(4.0,5.0));
        }
        Glyph::Search => {
            ui.painter().circle_stroke(at(-2.0,-2.0),6.5,Stroke::new(1.8,color));
            line((3.0,3.0),(10.0,10.0));
        }
        Glyph::Close => {line((-7.0,-7.0),(7.0,7.0)); line((7.0,-7.0),(-7.0,7.0));}
        Glyph::Shield => {
            line((0.0,-11.0),(-9.0,-7.0)); line((-9.0,-7.0),(-8.0,4.0));
            line((-8.0,4.0),(0.0,11.0)); line((0.0,11.0),(8.0,4.0));
            line((8.0,4.0),(9.0,-7.0)); line((9.0,-7.0),(0.0,-11.0));
            line((-4.0,0.0),(-1.0,4.0)); line((-1.0,4.0),(5.0,-3.0));
        }
        Glyph::Chat => {
            ui.painter().rect_stroke(Rect::from_min_max(at(-10.0,-8.0),at(10.0,6.0)),
                Rounding::same(3.0),Stroke::new(1.7,color));
            line((-3.0,6.0),(-7.0,11.0)); line((-7.0,11.0),(-7.0,6.0));
        }
    }
}
fn icon_button(ui: &mut Ui, name: &str, icon: Glyph, enabled: bool, selected: bool, danger: bool, side: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), if enabled {Sense::click()} else {Sense::hover()});
    let response = response.on_hover_text(name);
    let base = if !enabled {SURFACE_2} else if danger {Color32::from_rgb(134, 47, 70)}
        else if selected {Color32::from_rgb(31, 125, 98)} else if response.hovered() {
            Color32::from_rgb(65, 70, 113)
        } else {SURFACE_2};
    ui.painter().rect_filled(rect, Rounding::same(12.0), base);
    ui.painter().rect_stroke(rect, Rounding::same(12.0),
        Stroke::new(if selected {1.5} else {1.0},
            if selected {GREEN} else if danger {RED.linear_multiply(0.65)} else {BORDER}));
    glyph(ui, icon, rect, if !enabled {MUTED.linear_multiply(0.48)} else {BRIGHT});
    enabled && response.clicked()
}
fn meter(ui: &mut Ui, level: f32, bars: usize, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width,22.0), Sense::hover());
    let step = width / bars as f32;
    let active = (level.clamp(0.0,1.0)*bars as f32).round() as usize;
    for i in 0..bars {
        let bar_height = 5.0 + (i as f32 * 0.38).min(13.0);
        let x = rect.left() + step*i as f32 + 1.0;
        let y = rect.center().y;
        let bar_rect=Rect::from_center_size(Pos2::new(x+step*0.32,y),Vec2::new(step*0.6,bar_height));
        ui.painter().rect_filled(bar_rect,Rounding::same(2.0),if i<active {GREEN} else {BORDER});
    }
}
fn peer_color(idx: usize) -> Color32 {
    [PURPLE, BLUE, Color32::from_rgb(248,139,193),GREEN,
        Color32::from_rgb(247,184,98)][idx%5]
}
fn bubble_chat(ui: &mut Ui, msg: &ChatEntry, mine: bool) {
    if msg.system {
        egui::Frame::none()
            .fill(Color32::from_rgb(35,41,63))
            .rounding(Rounding::same(9.0))
            .inner_margin(egui::Margin::symmetric(11.0,7.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    dot(ui,BLUE,2.2);
                    ui.label(RichText::new(&msg.text).size(12.0).color(SOFT));
                });
            });
        return;
    }
    ui.horizontal(|ui| {
        avatar(ui,&msg.name,36.0,if mine {PURPLE} else {BLUE});
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&msg.name).color(if mine {Color32::from_rgb(193,169,255)} else {BRIGHT})
                    .strong().size(13.0));
                if mine {ui.label(text_caption("ти"));}
            });
            ui.label(RichText::new(&msg.text).color(BRIGHT).size(13.0));
        });
    });
}
fn peer_tile(ui: &mut Ui, width: f32, height: f32, name: &str, status: &str, color: Color32, active: bool, placeholder: bool) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(width,height),Sense::hover());
    let painter=ui.painter_at(r);
    painter.rect_filled(r,Rounding::same(16.0),Color32::from_rgb(32,35,61));
    // Layered lights create a GPU-friendly glow without external images.
    for i in (1..=8).rev() {
        painter.circle_filled(Pos2::new(r.right()-8.0,r.top()+4.0),i as f32*24.0,
            color.linear_multiply(0.016));
    }
    for i in (1..=5).rev() {
        painter.circle_filled(Pos2::new(r.left()+3.0,r.bottom()-5.0),i as f32*26.0,
            BLUE.linear_multiply(0.013));
    }
    painter.rect_stroke(r,Rounding::same(16.0),
        Stroke::new(if active {1.8} else {1.0},if active {GREEN} else {BORDER}));
    if placeholder {
        let c=Pos2::new(r.center().x, r.center().y-15.0);
        painter.circle_stroke(c,24.0,Stroke::new(1.2,MUTED.linear_multiply(0.45)));
        painter.text(c,Align2::CENTER_CENTER,"+",FontId::proportional(35.0),MUTED);
        painter.text(Pos2::new(r.center().x,r.bottom()-37.0),
            Align2::CENTER_CENTER,"Очікуємо друга",FontId::proportional(12.0),SOFT);
        painter.text(Pos2::new(r.center().x,r.bottom()-19.0),
            Align2::CENTER_CENTER,"P2P • Radmin VPN",FontId::proportional(10.0),MUTED);
        return;
    }
    let avatar_center=Pos2::new(r.center().x,r.center().y-13.0);
    painter.circle_filled(avatar_center,36.0,color.linear_multiply(0.25));
    painter.circle_stroke(avatar_center,36.0,Stroke::new(1.7,color));
    let initials=name.chars().take(2).collect::<String>().to_uppercase();
    painter.text(avatar_center,Align2::CENTER_CENTER,initials,FontId::proportional(25.0),BRIGHT);
    painter.rect_filled(Rect::from_min_max(
        Pos2::new(r.left()+1.0,r.bottom()-47.0),Pos2::new(r.right()-1.0,r.bottom()-1.0)),
        Rounding::same(10.0),Color32::from_rgba_unmultiplied(11,13,27,222));
    painter.circle_filled(Pos2::new(r.left()+16.0,r.bottom()-25.0),4.0,if active {GREEN} else {MUTED});
    painter.text(Pos2::new(r.left()+29.0,r.bottom()-30.0),
        Align2::LEFT_TOP,name,FontId::proportional(13.0),BRIGHT);
    painter.text(Pos2::new(r.left()+29.0,r.bottom()-14.0),
        Align2::LEFT_CENTER,status,FontId::proportional(10.5),SOFT);
}
impl FriendsApp {
    pub(super) fn neon_sidebar(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().auto_shrink([false,false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                // Logo is drawn vectorially to prevent tofu glyphs on Windows.
                let (r,_) = ui.allocate_exact_size(Vec2::new(42.0,42.0),Sense::hover());
                ui.painter().circle_filled(r.center(),19.0,PURPLE.linear_multiply(0.25));
                ui.painter().circle_stroke(r.center(),18.0,Stroke::new(2.3,PURPLE));
                glyph(ui,Glyph::Headphones,r,BRIGHT);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Friends Voice").size(21.0).strong().color(BRIGHT));
                    ui.label(RichText::new("SERVERLESS · RADMIN VPN").size(9.5).color(MUTED));
                });
            });
            ui.add_space(15.0);
            card().show(ui,|ui| {
                ui.horizontal(|ui| {
                    dot(ui,if self.mesh.is_some(){GREEN}else{RED},4.0);
                    ui.label(RichText::new(if self.mesh.is_some(){"P2P вузол працює"}else{"P2P недоступний"})
                        .size(14.0).strong().color(BRIGHT));
                });
                ui.add_space(3.0);
                ui.label(RichText::new(if self.mesh.is_some(){
                    "Прямий зв'язок між друзями без центрального сервера."
                } else {"Не вдалося запустити P2P. Перевір UDP-порт 45888."}).size(11.0).color(SOFT));
                ui.horizontal_wrapped(|ui| {
                    small_chip(ui,"P2P MESH",PURPLE);
                    small_chip(ui,"БЕЗ СЕРВЕРА",BLUE);
                });
                if self.mesh.is_none() {
                    if let Some(message)=&self.network_error {
                        ui.label(RichText::new(message).size(11.0).color(RED));
                    }
                    if outline_button(ui,"Спробувати ще раз") {self.retry_network();}
                }
            });
            ui.add_space(9.0);
            card().show(ui,|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Radmin VPN").strong().size(13.0).color(BRIGHT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                        small_chip(ui,if self.mesh.as_ref().is_some_and(|m|!m.local_radmin_ips.is_empty()){
                            "ВИЯВЛЕНО"
                        } else {"НЕ ЗНАЙДЕНО"},
                            if self.mesh.as_ref().is_some_and(|m|!m.local_radmin_ips.is_empty()){GREEN}else{MUTED});
                    });
                });
                ui.label(text_caption("Твої віртуальні IP-адреси"));
                let ips=self.mesh.as_ref().map(|m|m.local_radmin_ips.as_slice()).unwrap_or(&[]);
                if ips.is_empty() {
                    ui.label(RichText::new("26.x.x.x не знайдено — перевір Radmin VPN.")
                        .size(11.0).color(SOFT));
                } else {
                    for ip in ips {
                        ui.horizontal(|ui|{
                            dot(ui,BLUE,2.5);
                            ui.label(RichText::new(ip.to_string()).size(12.0).strong().color(BRIGHT));
                        });
                    }
                }
            });
            ui.add_space(12.0);
            section(ui,"ГОЛОСОВИЙ КАНАЛ");
            soft_card().show(ui,|ui|{
                ui.horizontal(|ui|{
                    let (r,_)=ui.allocate_exact_size(Vec2::splat(25.0),Sense::hover());
                    glyph(ui,Glyph::Volume,r,BLUE);
                    ui.label(RichText::new("Загальний").size(14.0).strong().color(BRIGHT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                        if self.voice.is_some(){dot(ui,GREEN,3.0);}
                    });
                });
            });
            ui.add_space(7.0);
            let joined=self.voice.is_some();
            if tinted_button(ui,if joined{"Залишити голос"}else{"Приєднатися до голосу"},
                if joined{Color32::from_rgb(157,50,75)}else{PURPLE},
                self.mesh.is_some(),37.0).clicked() {
                self.toggle_voice();
            }
            ui.add_space(15.0);
            section(ui,&format!("ДРУЗІ В МЕРЕЖІ  ({}/{})",self.peers.len()+1,MAX_NODES));
            card().show(ui,|ui| {
                ui.horizontal(|ui| {
                    avatar(ui,&self.settings.nickname,33.0,PURPLE);
                    ui.vertical(|ui|{
                        ui.label(RichText::new(format!("{} (ти)",self.settings.nickname)).strong().size(12.5));
                        ui.label(text_caption(if self.voice.is_some(){"У голосі"}else{"У мережі"}));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                        dot(ui,GREEN,3.5);
                    });
                });
                for (i,p) in self.peers.iter().enumerate() {
                    ui.add_space(7.0);
                    ui.horizontal(|ui|{
                        avatar(ui,&p.name,33.0,peer_color(i+1));
                        ui.vertical(|ui|{
                            ui.label(RichText::new(&p.name).strong().size(12.5));
                            ui.label(text_caption(if p.voice{"У голосі"}else{"Онлайн"}));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                            dot(ui,if p.voice{GREEN}else{BLUE},3.5);
                        });
                    });
                }
                if self.peers.is_empty(){
                    ui.add_space(6.0);
                    ui.label(text_caption("Друзі з'являться тут після підключення."));
                }
            });
            ui.add_space(14.0);
            section(ui,"ПОШУК ДРУЗІВ");
            ui.label(RichText::new("Якщо пошук не спрацював, додай IP друга один раз.")
                .size(11.0).color(SOFT));
            let input=ui.add(egui::TextEdit::singleline(&mut self.friend_ip)
                .hint_text("26.x.x.x")
                .desired_width(ui.available_width()));
            if input.lost_focus() && ui.input(|i|i.key_pressed(egui::Key::Enter)) {
                self.add_friend();
            }
            if tinted_button(ui,"Знайти за IP",Color32::from_rgb(81,87,185),
                self.mesh.is_some(),32.0).clicked(){self.add_friend();}
            ui.add_space(18.0);
        });
    }

    pub(super) fn neon_settings(&mut self,ui:&mut Ui) {
        egui::ScrollArea::vertical().auto_shrink([false,false]).show(ui,|ui|{
            ui.add_space(8.0);
            ui.horizontal(|ui|{
                ui.label(text_title("Налаштування"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if icon_button(ui,"Закрити",Glyph::Close,true,false,false,30.0){
                        self.settings_open=false;
                    }
                });
            });
            ui.add_space(8.0);
            for (index,label) in ["Аудіо", "Відео", "Загальні", "Про програму"].iter().enumerate(){
                let selected=self.settings_page==index;
                let button=egui::Button::new(RichText::new(*label)
                    .color(if selected{BRIGHT}else{SOFT}).size(13.0))
                    .fill(if selected{PURPLE.linear_multiply(0.45)}else{SURFACE_2})
                    .rounding(Rounding::same(9.0))
                    .min_size(Vec2::new(ui.available_width(),29.0));
                if ui.add(button).clicked(){self.settings_page=index;}
            }
            ui.add_space(13.0);
            ui.separator();
            match self.settings_page {
                0=>{
                    section(ui,"АУДІОПРИСТРОЇ");
                    ui.label(RichText::new("Мікрофон").size(12.0).color(BRIGHT));
                    egui::ComboBox::from_id_salt("fv_input_device")
                        .width(ui.available_width()-5.0)
                        .selected_text(if self.settings.input_device.is_empty(){
                            "За замовчуванням"
                        }else{&self.settings.input_device})
                        .show_ui(ui,|ui|{
                            ui.selectable_value(&mut self.settings.input_device,String::new(),"За замовчуванням");
                            for name in &self.input_devices{
                                ui.selectable_value(&mut self.settings.input_device,name.clone(),name);
                            }
                        });
                    ui.add_space(7.0);
                    ui.label(RichText::new("Навушники / динаміки").size(12.0).color(BRIGHT));
                    egui::ComboBox::from_id_salt("fv_output_device")
                        .width(ui.available_width()-5.0)
                        .selected_text(if self.settings.output_device.is_empty(){
                            "За замовчуванням"
                        }else{&self.settings.output_device})
                        .show_ui(ui,|ui|{
                            ui.selectable_value(&mut self.settings.output_device,String::new(),"За замовчуванням");
                            for name in &self.output_devices{
                                ui.selectable_value(&mut self.settings.output_device,name.clone(),name);
                            }
                        });
                    ui.add_space(5.0);
                    if outline_button(ui,"Оновити список пристроїв"){
                        self.input_devices=voice::list_input_devices();
                        self.output_devices=voice::list_output_devices();
                    }
                    ui.add_space(12.0);
                    ui.label(RichText::new("Гучність відтворення").size(12.0).color(BRIGHT));
                    let mut value=f32::from_bits(self.volume.load(Ordering::Relaxed)).clamp(0.0,2.0);
                    if ui.add(egui::Slider::new(&mut value,0.0..=2.0)
                        .show_value(false)).changed() {
                        self.volume.store(value.to_bits(),Ordering::Relaxed);
                    }
                    ui.label(text_caption(format!("{:.0}%",value*100.0)));
                    ui.add_space(7.0);
                    ui.label(RichText::new("Рівень мікрофона").size(12.0).color(BRIGHT));
                    let rms=self.voice.as_ref().map(|v|
                        f32::from_bits(v.microphone_level.load(Ordering::Relaxed))
                    ).unwrap_or(0.0).clamp(0.0,1.0);
                    meter(ui,(rms*7.0).min(1.0),19,ui.available_width()-5.0);
                    ui.add_space(8.0);
                    if let Some(voice) = &self.voice {
                        let active = self.mesh.as_ref()
                            .and_then(|mesh| mesh.voice_targets.lock().ok().map(|v| v.len()))
                            .unwrap_or(0);
                        ui.label(RichText::new("ДІАГНОСТИКА ГОЛОСУ").size(11.0).strong().color(SOFT));
                        ui.label(text_caption(format!("Друзів у голосі: {active}")));
                        ui.label(text_caption(format!(
                            "UDP пакети: відправлено {} · отримано {}",
                            voice.packets_sent.load(Ordering::Relaxed),
                            voice.packets_received.load(Ordering::Relaxed)
                        )));
                        let errors = voice.udp_errors.load(Ordering::Relaxed);
                        if errors > 0 {
                            ui.label(RichText::new(format!("UDP попереджень: {errors}"))
                                .size(11.0).color(Color32::from_rgb(246, 185, 95)));
                        }
                        if active > 0 && voice.packets_sent.load(Ordering::Relaxed) > 100
                            && voice.packets_received.load(Ordering::Relaxed) == 0 {
                            ui.label(RichText::new("Немає вхідного аудіо. Перевір UDP 45889 на комп'ютерах друзів.")
                                .size(11.0).color(RED));
                        }
                    }
                    ui.label(text_caption("Після зміни аудіопристрою перезайди у голосовий канал."));
                    ui.add_space(9.0);
                    if tinted_button(ui,"Зберегти налаштування",PURPLE,true,33.0).clicked(){
                        self.settings.volume=value;
                        save_settings(&self.settings);
                        self.notice("Налаштування звуку збережено.");
                    }
                }
                1=>{
                    section(ui,"ВІДЕО ТА ЕКРАН");
                    card().show(ui,|ui|{
                        ui.label(RichText::new("Камера").strong().color(BRIGHT));
                        ui.label(RichText::new("Поки недоступна. Наступний етап — нативна передача відео P2P.")
                            .size(12.0).color(SOFT));
                    });
                    ui.add_space(9.0);
                    card().show(ui,|ui|{
                        ui.label(RichText::new("Демонстрація екрана").strong().color(BRIGHT));
                        ui.label(RichText::new("Ще не реалізована. Кнопка внизу поки вимкнена.")
                            .size(12.0).color(SOFT));
                    });
                }
                2=>{
                    section(ui,"ПРОФІЛЬ");
                    ui.label(RichText::new("Твій нікнейм").size(12.0).color(BRIGHT));
                    let changed=ui.add(egui::TextEdit::singleline(&mut self.settings.nickname)
                        .desired_width(ui.available_width())).changed();
                    if changed{
                        if self.settings.nickname.chars().count()>24{
                            self.settings.nickname=self.settings.nickname.chars().take(24).collect();
                        }
                        if !self.settings.nickname.trim().is_empty(){
                            if let Some(mesh)=&self.mesh{mesh.rename(self.settings.nickname.clone());}
                        }
                        save_settings(&self.settings);
                    }
                    ui.add_space(13.0);
                    section(ui,"ЗБЕРЕЖЕНІ КОНТАКТИ");
                    ui.label(text_caption(format!("Відомих IP-адрес: {}",self.settings.known_ips.len())));
                    ui.label(text_caption("Адреси зберігаються локально, без жодного акаунта."));
                    ui.add_space(14.0);
                    section(ui,"ГАРЯЧІ КЛАВІШІ");
                    ui.label(RichText::new("Ctrl + M  —  увімкнути / вимкнути мікрофон")
                        .size(12.0).color(SOFT));
                }
                _=>{
                    section(ui,"FRIENDS VOICE");
                    ui.label(text_title("Версія 0.4 · Neon"));
                    ui.label(RichText::new("Нативний P2P-месенджер для Windows 10/11. Rust + egui.")
                        .size(12.0).color(SOFT));
                    ui.add_space(12.0);
                    ui.label(RichText::new("Без центрального сервера").strong().color(GREEN));
                    ui.label(text_caption("Голос і повідомлення передаються напряму через Radmin VPN."));
                    ui.add_space(8.0);
                    ui.label(text_caption("Використовуй лише довірену приватну мережу: власного наскрізного шифрування ще немає."));
                }
            }
            ui.add_space(15.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(17,59,63))
                .stroke(Stroke::new(1.0,GREEN.linear_multiply(0.5)))
                .rounding(Rounding::same(13.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui,|ui|{
                    ui.horizontal(|ui|{
                        dot(ui,GREEN,3.5);
                        ui.label(RichText::new("АВТОНОМНА МЕРЕЖА").size(11.0).strong().color(GREEN));
                    });
                    ui.label(RichText::new("Friends Voice працює напряму між друзями в Radmin VPN. Вихід одного учасника не завершує розмову решти.")
                        .size(11.0).color(SOFT));
                });
        });
    }

    pub(super) fn neon_main(&mut self, ui: &mut Ui) {
        ui.vertical(|ui|{
            ui.horizontal(|ui|{
                ui.label(RichText::new("Загальний").size(23.0).strong().color(BRIGHT));
                ui.add_space(8.0);
                small_chip(ui,"P2P MESH",PURPLE);
                small_chip(ui,"БЕЗ СЕРВЕРА",BLUE);
                if self.mesh.as_ref().is_some_and(|m|!m.local_radmin_ips.is_empty()){
                    small_chip(ui,"RADMIN VPN",GREEN);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if icon_button(ui,"Налаштування",Glyph::Settings,true,self.settings_open,false,32.0){
                        self.settings_open=!self.settings_open;
                    }
                });
            });
            ui.add_space(5.0);
            ui.label(RichText::new("Голосовий канал  /  живе P2P-з'єднання")
                .color(MUTED).size(11.5));
            ui.add_space(13.0);
            let content_w=ui.available_width();
            let gap=10.0;
            let tile_w=((content_w-2.0*gap)/3.0).max(95.0);
            let tile_h=if ui.available_height()<590.0 {147.0}else{174.0};
            ui.horizontal(|ui|{
                ui.spacing_mut().item_spacing.x=gap;
                peer_tile(ui,tile_w,tile_h,&self.settings.nickname,
                    if self.voice.is_some(){"Ти · у голосі"}else{"Ти · онлайн"},
                    PURPLE,self.voice.is_some(),false);
                for i in 0..2{
                    if let Some(p)=self.peers.get(i){
                        peer_tile(ui,tile_w,tile_h,&p.name,
                            if p.voice{"У голосовому каналі"}else{"У мережі"},
                            peer_color(i+1),p.voice,false);
                    }else{
                        peer_tile(ui,tile_w,tile_h,"","",BLUE,false,true);
                    }
                }
            });
            if self.peers.len()>2{
                ui.label(text_caption(format!("Ще {} друзів у мережі — дивись список зліва.",self.peers.len()-2)));
            }
            ui.add_space(11.0);
            // The chat flexes vertically; the footer remains visible.
            let footer_h=64.0;
            let chat_h=(ui.available_height()-footer_h-8.0).max(180.0);
            egui::Frame::none()
                .fill(SURFACE)
                .stroke(Stroke::new(1.0,BORDER))
                .rounding(Rounding::same(15.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui,|ui|{
                    ui.set_min_height(chat_h-25.0);
                    ui.horizontal(|ui|{
                        ui.label(RichText::new("# загальний").strong().size(16.0).color(BRIGHT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                            ui.label(text_caption(format!("{} учасників",self.peers.len()+1)));
                        });
                    });
                    ui.separator();
                    let available_chat=(chat_h-94.0).max(72.0);
                    egui::ScrollArea::vertical()
                        .id_salt("fv_neon_chat")
                        .max_height(available_chat)
                        .stick_to_bottom(true)
                        .auto_shrink([false,false])
                        .show(ui,|ui|{
                            ui.set_min_height(available_chat);
                            for message in &self.messages{
                                let mine=message.name==self.settings.nickname && !message.system;
                                bubble_chat(ui,message,mine);
                                ui.add_space(9.0);
                            }
                        });
                    ui.separator();
                    ui.horizontal(|ui|{
                        let available=(ui.available_width()-51.0).max(80.0);
                        let response=ui.add_enabled(self.mesh.is_some(),
                            egui::TextEdit::singleline(&mut self.draft)
                                .hint_text("Повідомлення у #загальний…")
                                .desired_width(available));
                        if response.lost_focus() && ui.input(|i|i.key_pressed(egui::Key::Enter)){
                            self.send_chat();
                        }
                        if icon_button(ui,"Надіслати повідомлення",Glyph::Send,
                            self.mesh.is_some(),false,false,35.0){
                            self.send_chat();
                        }
                    });
                });
            ui.add_space(8.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(22,27,45))
                .stroke(Stroke::new(1.0,BORDER))
                .rounding(Rounding::same(14.0))
                .inner_margin(egui::Margin::symmetric(12.0,8.0))
                .show(ui,|ui|{
                    ui.horizontal(|ui|{
                        ui.vertical(|ui|{
                            ui.label(RichText::new("P2P-з'єднання").strong().size(12.0).color(GREEN));
                            ui.label(text_caption(format!("Напряму · {} друзів",self.peers.len())));
                        });
                        ui.add_space(7.0);
                        ui.separator();
                        let voice=self.voice.is_some();
                        let mic=self.mic_enabled.load(Ordering::Relaxed);
                        let deaf=self.deafened.load(Ordering::Relaxed);
                        if icon_button(ui,"Мікрофон · Ctrl+M",Glyph::Mic,
                            voice,mic && voice,false,42.0){
                            self.mic_enabled.store(!mic,Ordering::Relaxed);
                        }
                        if icon_button(ui,"Вимкнути / увімкнути звук",Glyph::Headphones,
                            voice,deaf && voice,false,42.0){
                            self.deafened.store(!deaf,Ordering::Relaxed);
                        }
                        icon_button(ui,"Камера — ще в розробці",Glyph::Camera,false,false,false,42.0);
                        icon_button(ui,"Демонстрація екрана — ще в розробці",Glyph::Monitor,false,false,false,42.0);
                        if icon_button(ui,"Налаштування",Glyph::Settings,true,self.settings_open,false,42.0){
                            self.settings_open=!self.settings_open;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                            if voice{
                                if icon_button(ui,"Вийти з голосового каналу",Glyph::Power,true,false,true,42.0){
                                    self.toggle_voice();
                                }
                                ui.label(RichText::new("Вийти з голосу").size(11.0).color(SOFT));
                            }else{
                                if ui.add_enabled(self.mesh.is_some(),
                                    egui::Button::new(RichText::new("Увійти в голос").strong().color(BRIGHT))
                                    .fill(PURPLE).rounding(Rounding::same(9.0))
                                    .min_size(Vec2::new(117.0,40.0))).clicked(){
                                    self.toggle_voice();
                                }
                            }
                        });
                    });
                });
        });
    }
}
