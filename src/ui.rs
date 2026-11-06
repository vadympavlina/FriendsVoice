//! Native egui desktop design. Every interactive element is wired to the existing P2P client.
//! Webcam tiles are live when enabled; screen-sharing remains visibly disabled.
use super::*;
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2};

const SURFACE: Color32 = Color32::from_rgb(22, 27, 46);
const SURFACE_2: Color32 = Color32::from_rgb(31, 38, 64);
const BORDER: Color32 = Color32::from_rgb(56, 70, 100);
const BRIGHT: Color32 = Color32::from_rgb(241, 246, 255);
const PURPLE: Color32 = Color32::from_rgb(148, 108, 250);
const BLUE: Color32 = Color32::from_rgb(76, 199, 244);
const SOFT: Color32 = Color32::from_rgb(181, 193, 216);

#[derive(Clone, Copy)]
enum Glyph {
    Mic, Headphones, Camera, Monitor, Settings, Send, Power,
    Users, Network, Volume, Search, Close, Shield, Chat, Sidebar, Focus, Keyboard,
}
fn card() -> egui::Frame {
    egui::Frame::none()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER.linear_multiply(0.7)))
        .rounding(Rounding::same(18.0))
        .inner_margin(egui::Margin::same(15.0))
}
fn soft_card() -> egui::Frame {
    egui::Frame::none()
        .fill(SURFACE_2)
        .rounding(Rounding::same(14.0))
        .inner_margin(egui::Margin::same(12.0))
}
fn text_title(text: impl Into<String>) -> RichText {
    RichText::new(text).size(18.0).strong().color(BRIGHT)
}
fn text_caption(text: impl Into<String>) -> RichText {
    RichText::new(text).size(11.0).color(MUTED)
}
fn section(ui: &mut Ui, title: &str) {
    ui.add_space(7.0);
    ui.label(RichText::new(title).size(10.5).strong().color(Color32::from_rgb(151, 166, 202)));
    ui.add_space(5.0);
}
fn tinted_button(ui: &mut Ui, label: &str, accent: Color32, enabled: bool, height: f32) -> Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).color(BRIGHT).strong().size(13.0))
            .fill(accent)
            .stroke(Stroke::new(1.0, accent.linear_multiply(1.3)))
            .rounding(Rounding::same(12.0))
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
        .rounding(Rounding::same(15.0))
        .inner_margin(egui::Margin::symmetric(11.0, 5.0))
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
        Glyph::Sidebar => {
            ui.painter().rect_stroke(Rect::from_min_max(at(-11.0,-9.0),at(11.0,9.0)),
                Rounding::same(3.0),Stroke::new(1.7,color));
            line((-4.0,-9.0),(-4.0,9.0));
            line((-1.0,-4.0),(8.0,-4.0));
            line((-1.0,1.0),(6.0,1.0));
        }
        Glyph::Focus => {
            line((-11.0,-4.0),(-11.0,-10.0));line((-11.0,-10.0),(-5.0,-10.0));
            line((5.0,-10.0),(11.0,-10.0));line((11.0,-10.0),(11.0,-4.0));
            line((-11.0,4.0),(-11.0,10.0));line((-11.0,10.0),(-5.0,10.0));
            line((5.0,10.0),(11.0,10.0));line((11.0,10.0),(11.0,4.0));
        }
        Glyph::Keyboard => {
            ui.painter().rect_stroke(Rect::from_min_max(at(-12.0,-8.0),at(12.0,8.0)),
                Rounding::same(3.0),Stroke::new(1.6,color));
            for yy in [-4.0,0.0] {
                for xx in [-7.0,-2.0,3.0,8.0] {
                    ui.painter().circle_filled(at(xx,yy),1.0,color);
                }
            }
            line((-6.0,4.0),(6.0,4.0));
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
    if enabled && response.hovered() {
        ui.painter().rect_filled(rect.expand(2.0), Rounding::same(14.0),
            PURPLE.linear_multiply(0.14));
    }
    ui.painter().rect_filled(rect, Rounding::same(12.0), base);
    ui.painter().rect_stroke(rect, Rounding::same(12.0),
        Stroke::new(if selected {1.8} else if response.hovered() {1.4}else{1.0},
            if selected {GREEN} else if danger {RED.linear_multiply(0.75)}
                else if response.hovered(){PURPLE}else{BORDER}));
    glyph(ui, icon, rect, if !enabled {MUTED.linear_multiply(0.48)} else {BRIGHT});
    enabled && response.clicked()
}
fn dock_action(
    ui: &mut Ui, label: &str, tooltip: &str, icon: Glyph,
    enabled: bool, selected: bool, danger: bool, compact: bool,
) -> bool {
    if compact {
        return icon_button(ui,tooltip,icon,enabled,selected,danger,38.0);
    }
    let mut pressed=false;
    ui.vertical(|ui|{
        pressed=icon_button(ui,tooltip,icon,enabled,selected,danger,42.0);
        ui.label(RichText::new(label).size(10.0).color(if enabled{SOFT}else{MUTED}));
    });
    pressed
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
fn gallery_columns(width: f32) -> usize {
    if width >= 805.0 {3} else if width >= 370.0 {2} else {1}
}
fn gallery_count(connected: usize, columns: usize, focus: bool) -> usize {
    let visible=connected.saturating_add(1).min(MAX_NODES);
    if focus {visible} else {visible.max(columns).min(MAX_NODES)}
}
#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn gallery_adapts_to_small_and_wide_windows() {
        assert_eq!(gallery_columns(369.0),1);
        assert_eq!(gallery_columns(370.0),2);
        assert_eq!(gallery_columns(804.0),2);
        assert_eq!(gallery_columns(805.0),3);
        assert_eq!(gallery_count(0,3,false),3);
        assert_eq!(gallery_count(0,3,true),1);
        assert_eq!(gallery_count(4,3,false),5);
        assert_eq!(gallery_count(9,3,true),5);
    }
}
fn bubble_chat(ui: &mut Ui, msg: &ChatEntry, mine: bool) {
    if msg.system {
        egui::Frame::none()
            .fill(Color32::from_rgb(29, 43, 65))
            .stroke(Stroke::new(0.6, Color32::from_rgb(48, 75, 105)))
            .rounding(Rounding::same(10.0))
            .inner_margin(egui::Margin::symmetric(11.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    dot(ui, BLUE, 2.4);
                    ui.label(RichText::new(&msg.text).size(11.5).color(SOFT));
                });
            });
        return;
    }
    ui.horizontal(|ui| {
        avatar(ui, &msg.name, 34.0, if mine {PURPLE} else {BLUE});
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&msg.name)
                    .color(if mine {Color32::from_rgb(189, 163, 255)} else {BRIGHT})
                    .strong().size(12.5));
                if mine {
                    small_chip(ui,"ТИ",PURPLE);
                }
            });
            egui::Frame::none()
                .fill(if mine {Color32::from_rgb(51, 44, 82)}
                      else {Color32::from_rgb(35, 43, 68)})
                .rounding(Rounding::same(10.0))
                .inner_margin(egui::Margin::symmetric(11.0, 8.0))
                .show(ui,|ui| {
                    ui.add(egui::Label::new(RichText::new(&msg.text)
                        .color(BRIGHT).size(12.7)).wrap());
                });
        });
    });
}

/// Camera tiles: cover-fit live video, animated-looking soft gradients rendered
/// directly in the native egui painter, speaking ring, top badge and footer.
fn peer_tile(
    ui: &mut Ui, width: f32, height: f32,
    name: &str, status: &str, color: Color32,
    speaking: bool, placeholder: bool, video: Option<&egui::TextureHandle>,
) {
    let (r, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let p = ui.painter_at(r);
    let rounded = Rounding::same(17.0);
    p.rect_filled(r, rounded, Color32::from_rgb(23, 30, 52));

    // Color-light panels and horizontal accents have no texture allocations.
    let c1 = Pos2::new(r.right() - 15.0, r.top() + 8.0);
    let c2 = Pos2::new(r.left() + 11.0, r.bottom() - 29.0);
    for i in (1..=7).rev() {
        let radius = i as f32 * 28.0;
        p.circle_filled(c1, radius, color.linear_multiply(0.017));
        p.circle_filled(c2, radius * 0.85, BLUE.linear_multiply(0.011));
    }
    p.line_segment(
        [Pos2::new(r.left()+18.0,r.top()+18.0),
         Pos2::new(r.right()-18.0,r.top()+18.0)],
        Stroke::new(1.0, color.linear_multiply(0.22)));

    if let Some(frame) = video {
        // Preserve the camera's 16:9 aspect ratio and center-crop rather than stretch.
        let aspect = (width / height).max(0.01);
        let uv = if aspect < 16.0 / 9.0 {
            let visible = (aspect / (16.0/9.0)).clamp(0.01, 1.0);
            Rect::from_min_max(Pos2::new((1.0-visible)*0.5,0.0),
                               Pos2::new((1.0+visible)*0.5,1.0))
        } else {
            let visible = ((16.0/9.0) / aspect).clamp(0.01,1.0);
            Rect::from_min_max(Pos2::new(0.0,(1.0-visible)*0.5),
                               Pos2::new(1.0,(1.0+visible)*0.5))
        };
        p.image(frame.id(), r.shrink(1.0), uv, Color32::WHITE);
    } else if placeholder {
        let center = Pos2::new(r.center().x,r.center().y-16.0);
        p.circle_filled(center, 27.0, Color32::from_rgb(35,45,74));
        p.circle_stroke(center, 26.0, Stroke::new(1.0,BORDER));
        p.text(center, Align2::CENTER_CENTER, "+",
            FontId::proportional(32.0),SOFT);
        p.text(Pos2::new(center.x,center.y+47.0),
            Align2::CENTER_CENTER,"Місце для друга",
            FontId::proportional(12.5),BRIGHT);
    } else {
        let center = Pos2::new(r.center().x,r.center().y-12.0);
        p.circle_filled(center, 43.0, color.linear_multiply(0.20));
        p.circle_stroke(center, 40.0, Stroke::new(1.2,color.linear_multiply(0.7)));
        p.circle_filled(center, 31.0, color.linear_multiply(0.16));
        p.text(center,Align2::CENTER_CENTER,
            name.chars().take(2).collect::<String>().to_uppercase(),
            FontId::proportional(28.0), BRIGHT);
    }

    if !placeholder {
        let bar = Rect::from_min_max(
            Pos2::new(r.left()+1.0,r.bottom()-52.0),
            Pos2::new(r.right()-1.0,r.bottom()-1.0),
        );
        p.rect_filled(bar,Rounding::same(13.0),
            Color32::from_rgba_unmultiplied(11,17,31,235));
        p.circle_filled(Pos2::new(r.left()+17.0,r.bottom()-27.0),4.0,
            if speaking {GREEN} else {color});
        p.text(Pos2::new(r.left()+28.0,r.bottom()-33.0),
            Align2::LEFT_TOP,
            if name.chars().count() > 23 {
                format!("{}…",name.chars().take(22).collect::<String>())
            } else { name.to_string() },
            FontId::proportional(13.0),BRIGHT);
        p.text(Pos2::new(r.left()+28.0,r.bottom()-14.0),
            Align2::LEFT_CENTER, status,FontId::proportional(10.5),SOFT);

        // Top right live badge.
        if video.is_some() {
            let badge = Rect::from_min_size(
                Pos2::new(r.right()-72.0,r.top()+12.0), Vec2::new(58.0,24.0));
            p.rect_filled(badge,Rounding::same(11.0),
                Color32::from_rgba_unmultiplied(14,31,35,220));
            p.circle_filled(Pos2::new(badge.left()+11.0,badge.center().y),3.0,GREEN);
            p.text(Pos2::new(badge.left()+20.0,badge.center().y),
                Align2::LEFT_CENTER,"LIVE",FontId::proportional(10.5),BRIGHT);
        }
        if speaking {
            let center = Pos2::new(r.right()-19.0,r.bottom()-27.0);
            for (x,h) in [(-6.0,9.0),(0.0,17.0),(6.0,12.0)] {
                p.line_segment(
                    [Pos2::new(center.x+x,center.y-h*0.5),
                     Pos2::new(center.x+x,center.y+h*0.5)],
                    Stroke::new(2.3,GREEN));
            }
        }
    } else {
        p.text(Pos2::new(r.center().x,r.bottom()-20.0),
            Align2::CENTER_CENTER,"P2P · FRIENDS VOICE",
            FontId::proportional(10.0),MUTED);
    }

    let speaking_glow=ui.ctx().animate_bool(
        egui::Id::new(("fv_voice_highlight",name)),speaking);
    if speaking_glow > 0.01 {
        p.rect_stroke(r.shrink(1.0),rounded,
            Stroke::new(2.6*speaking_glow,
                GREEN.linear_multiply(speaking_glow*0.55)));
    }
    p.rect_stroke(r,rounded,
        Stroke::new(if speaking {2.0} else if response.hovered() {1.6}else{1.0},
            if speaking {GREEN} else if response.hovered() {color}else{BORDER}));
}

impl FriendsApp {
    pub(super) fn neon_sidebar(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .id_salt("aurora_sidebar")
            .auto_shrink([false,false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add_space(5.0);

                // A native vector mark; does not depend on unavailable emoji fonts.
                ui.horizontal(|ui|{
                    let (r,_) = ui.allocate_exact_size(Vec2::splat(49.0),Sense::hover());
                    ui.painter().rect_filled(r,Rounding::same(15.0),PURPLE);
                    for offset in [-13.0f32,-5.0,4.0,12.0] {
                        let height = if offset == -5.0 {21.0} else if offset == 4.0 {28.0} else {13.0};
                        ui.painter().line_segment(
                            [Pos2::new(r.center().x+offset,r.center().y-height*0.5),
                             Pos2::new(r.center().x+offset,r.center().y+height*0.5)],
                            Stroke::new(3.0,BRIGHT));
                    }
                    ui.add_space(2.0);
                    ui.vertical(|ui|{
                        ui.label(RichText::new("FRIENDS").size(20.0).strong().color(BRIGHT));
                        ui.label(RichText::new("VOICE  /  AURORA").size(10.2).strong().color(BLUE));
                    });
                });
                ui.add_space(5.0);
                ui.label(RichText::new("Твій приватний простір для розмов")
                    .size(11.4).color(SOFT));
                ui.add_space(16.0);

                egui::Frame::none()
                    .fill(Color32::from_rgb(25,42,55))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(43,98,99)))
                    .rounding(Rounding::same(16.0))
                    .inner_margin(egui::Margin::same(14.0))
                    .show(ui,|ui|{
                        ui.horizontal(|ui|{
                            dot(ui,if self.mesh.is_some(){GREEN}else{RED},4.0);
                            ui.label(RichText::new(
                                if self.mesh.is_some(){"P2P мережа активна"}else{"Немає з'єднання"})
                                .size(12.8).strong().color(BRIGHT));
                        });
                        ui.add_space(3.0);
                        ui.label(RichText::new("Без сервера. Прямий зв'язок через Radmin VPN.")
                            .size(11.3).color(SOFT));
                        ui.add_space(5.0);
                        ui.horizontal_wrapped(|ui|{
                            small_chip(ui,"MESH",GREEN);
                            small_chip(ui,"PRIVATE",BLUE);
                        });
                        if self.mesh.is_none(){
                            if let Some(error)=&self.network_error {
                                ui.label(RichText::new(error).size(11.0).color(RED));
                            }
                            if tinted_button(ui,"Відновити з'єднання",PURPLE,true,35.0).clicked(){
                                self.retry_network();
                            }
                        }
                    });
                ui.add_space(18.0);
                section(ui,"МІЙ ПРОСТІР");
                egui::Frame::none()
                    .fill(Color32::from_rgb(35,34,66))
                    .stroke(Stroke::new(1.0,PURPLE.linear_multiply(0.56)))
                    .rounding(Rounding::same(17.0))
                    .inner_margin(egui::Margin::same(14.0))
                    .show(ui,|ui|{
                        ui.horizontal(|ui|{
                            let (r,_) = ui.allocate_exact_size(Vec2::splat(34.0),Sense::hover());
                            ui.painter().rect_filled(r,Rounding::same(11.0),
                                PURPLE.linear_multiply(0.5));
                            glyph(ui,Glyph::Headphones,r,BRIGHT);
                            ui.vertical(|ui|{
                                ui.label(RichText::new("Загальний").strong().size(15.0).color(BRIGHT));
                                ui.label(text_caption("VOICE CHANNEL  /  01"));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                dot(ui,if self.voice.is_some(){GREEN}else{PURPLE},3.0);
                            });
                        });
                        ui.add_space(8.0);
                        let joined=self.voice.is_some();
                        if tinted_button(ui,
                            if joined{"Залишити голосовий канал"}else{"Приєднатися до голосу"},
                            if joined{Color32::from_rgb(136,52,79)}else{PURPLE},
                            self.mesh.is_some(),41.0).clicked() {
                            self.toggle_voice();
                        }
                    });

                ui.add_space(19.0);
                ui.horizontal(|ui|{
                    section(ui,"ДРУЗІ ОНЛАЙН");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                        small_chip(ui,&format!("{}/{}",self.peers.len()+1,MAX_NODES),GREEN);
                    });
                });
                card().show(ui,|ui|{
                    ui.horizontal(|ui|{
                        avatar(ui,&self.settings.nickname,39.0,PURPLE);
                        ui.add_space(2.0);
                        ui.vertical(|ui|{
                            ui.label(RichText::new(format!("{} · ти",self.settings.nickname))
                                .size(13.2).strong().color(BRIGHT));
                            ui.label(text_caption(if self.voice.is_some(){
                                "В голосовому каналі"
                            }else{"У мережі"}));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                            dot(ui,GREEN,3.5);
                        });
                    });
                    for (i,peer) in self.peers.iter().enumerate(){
                        ui.add_space(6.0);
                        ui.separator();
                        ui.horizontal(|ui|{
                            avatar(ui,&peer.name,38.0,peer_color(i+1));
                            ui.add_space(2.0);
                            ui.vertical(|ui|{
                                ui.label(RichText::new(&peer.name).size(12.8)
                                    .strong().color(BRIGHT));
                                let state=if peer.protocol!=PROTOCOL_VERSION {
                                    "Несумісна версія"
                                }else if self.voice.as_ref().is_some_and(|v|v.is_speaking(peer.id)){
                                    "Говорить зараз"
                                }else if peer.voice{"У голосі"}else{"У мережі"};
                                ui.label(RichText::new(state).size(10.9)
                                    .color(if peer.protocol!=PROTOCOL_VERSION{RED}else{SOFT}));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                dot(ui,if peer.protocol!=PROTOCOL_VERSION{RED}
                                    else if peer.voice{GREEN}else{BLUE},3.0);
                            });
                        });
                    }
                    if self.peers.is_empty() {
                        ui.add_space(10.0);
                        ui.label(RichText::new("Кімната готова до зустрічі!")
                            .size(12.0).strong().color(BRIGHT));
                        ui.label(RichText::new("Запроси друга до тієї самої мережі Radmin VPN.")
                            .size(11.3).color(SOFT));
                    }
                });

                ui.add_space(17.0);
                section(ui,"RADMIN VPN");
                soft_card().show(ui,|ui|{
                    ui.horizontal(|ui|{
                        let (r,_) = ui.allocate_exact_size(Vec2::splat(25.0),Sense::hover());
                        glyph(ui,Glyph::Network,r,BLUE);
                        ui.label(RichText::new("Мережеві адреси").size(12.2)
                            .strong().color(BRIGHT));
                    });
                    let ips=self.mesh.as_ref().map(|m|m.local_radmin_ips.as_slice()).unwrap_or(&[]);
                    if ips.is_empty() {
                        ui.label(RichText::new("Адресу 26.x.x.x не знайдено")
                            .size(11.3).color(MUTED));
                    }else{
                        for ip in ips {
                            ui.horizontal(|ui|{
                                dot(ui,GREEN,2.6);
                                ui.label(RichText::new(ip.to_string()).size(12.6)
                                    .strong().color(BRIGHT));
                            });
                        }
                        ui.add_space(7.0);
                        if outline_button(ui,"Скопіювати мій VPN IP"){
                            if let Some(ip) = ips.first() {
                                ui.output_mut(|o| o.copied_text = ip.to_string());
                            }
                        }
                    }
                });

                ui.add_space(17.0);
                section(ui,"ДОДАТИ ДРУГА");
                ui.label(text_caption("IP-адреса друга в Radmin VPN"));
                let entry=ui.add(
                    egui::TextEdit::singleline(&mut self.friend_ip)
                        .hint_text("26.XX.XX.XX")
                        .desired_width(ui.available_width())
                );
                if entry.lost_focus() && ui.input(|i|i.key_pressed(egui::Key::Enter)){
                    self.add_friend();
                }
                if tinted_button(ui,"Знайти за IP",Color32::from_rgb(66,101,153),
                    self.mesh.is_some(),37.0).clicked() {
                    self.add_friend();
                }
                ui.add_space(18.0);
                ui.label(RichText::new("FRIENDS VOICE 1.0  ·  AURORA STUDIO")
                    .size(10.1).color(MUTED));
                ui.add_space(14.0);
            });
    }


    pub(super) fn neon_settings(&mut self,ui:&mut Ui) {
        egui::ScrollArea::vertical().auto_shrink([false,false]).show(ui,|ui|{
            ui.add_space(7.0);
            ui.label(RichText::new("CUSTOMIZE YOUR SPACE")
                .size(10.8).strong().color(BLUE));
            ui.horizontal(|ui|{
                ui.label(RichText::new("Налаштування").size(22.0)
                    .strong().color(BRIGHT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if icon_button(ui,"Закрити",Glyph::Close,true,false,false,30.0){
                        self.settings_open=false;
                    }
                });
            });
            ui.add_space(3.0);
            ui.label(RichText::new("Звук, відео та персональні параметри")
                .size(11.2).color(SOFT));
            ui.add_space(14.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(27, 33, 55))
                .stroke(Stroke::new(1.0,BORDER))
                .rounding(Rounding::same(14.0))
                .inner_margin(egui::Margin::same(7.0))
                .show(ui,|ui|{
                    for (index,label) in ["Аудіо", "Відео", "Загальні", "Про програму"].iter().enumerate(){
                        let selected=self.settings_page==index;
                        let button=egui::Button::new(RichText::new(*label)
                            .color(if selected{BRIGHT}else{SOFT}).size(12.8)
                            .strong())
                            .fill(if selected{Color32::from_rgb(92,67,152)}else{SURFACE_2})
                            .stroke(Stroke::new(if selected{1.0}else{0.5},
                                if selected{PURPLE}else{BORDER}))
                            .rounding(Rounding::same(10.0))
                            .min_size(Vec2::new(ui.available_width(),33.0));
                        if ui.add(button).clicked(){self.settings_page=index;}
                    }
                });
            ui.add_space(15.0);
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
                    ui.add_space(8.0);
                    section(ui,"АКТИВАЦІЯ МІКРОФОНА");
                    let mut ptt = self.push_to_talk.load(Ordering::Relaxed);
                    let hotkey_label = match self.ptt_key.load(Ordering::Relaxed) {
                        0x05 => "Mouse4", 0x06 => "Mouse5", 0x75 => "F6",
                        0x76 => "F7", 0x77 => "F8", 0x79 => "F10",
                        0x7A => "F11", 0x7B => "F12", _ => "F8",
                    };
                    if ui.checkbox(&mut ptt,format!("Push-to-Talk ({hotkey_label})")).changed() {
                        self.push_to_talk.store(ptt, Ordering::Relaxed);
                        self.settings.push_to_talk = ptt;
                        save_settings(&self.settings);
                    }
                    ui.add_space(5.0);
                    ui.label(text_caption("Клавіша активації:"));
                    let mut key = self.ptt_key.load(Ordering::Relaxed);
                    egui::ComboBox::from_id_salt("fv_ptt_key")
                        .selected_text(hotkey_label)
                        .show_ui(ui, |ui| {
                            for (label, vk) in [
                                ("F6", 0x75), ("F7", 0x76), ("F8", 0x77),
                                ("F10", 0x79), ("F11", 0x7A), ("F12", 0x7B),
                                ("Mouse4 (бічна)", 0x05), ("Mouse5 (бічна)", 0x06),
                            ] {
                                ui.selectable_value(&mut key, vk, label);
                            }
                        });
                    if key != self.ptt_key.load(Ordering::Relaxed) {
                        self.ptt_key.store(key, Ordering::Relaxed);
                        self.settings.ptt_key = key;
                        save_settings(&self.settings);
                    }
                    ui.label(text_caption("F9 — увімкнути/вимкнути мікрофон навіть під час гри."));
                    ui.label(text_caption("Ctrl+M — перемикання всередині Friends Voice."));
                    if let Some(voice) = &self.voice {
                        ui.label(RichText::new(if voice.transmitting.load(Ordering::Relaxed) {
                            "● Зараз передаєш голос"
                        } else if ptt { "○ Утримуй вибрану клавішу, щоб говорити" } else {
                            "○ Передача вимкнена або немає голосового каналу"
                        }).size(11.0).color(if voice.transmitting.load(Ordering::Relaxed){GREEN}else{SOFT}));
                    }
                    ui.add_space(10.0);
                    section(ui, "ПРИГЛУШЕННЯ ФОНОВОГО ШУМУ");
                    let mut gate = self.noise_gate.load(Ordering::Relaxed);
                    if ui.checkbox(&mut gate, "Розумна активація голосом (Beta)").changed() {
                        self.noise_gate.store(gate, Ordering::Relaxed);
                        self.settings.noise_gate = gate;
                        save_settings(&self.settings);
                    }
                    if gate {
                        ui.label(text_caption("Чутливість: ліворуч — тихі звуки, праворуч — тільки гучна мова."));
                        let mut threshold = f32::from_bits(
                            self.gate_threshold.load(Ordering::Relaxed)
                        ).clamp(0.003, 0.08);
                        if ui.add(egui::Slider::new(&mut threshold, 0.003..=0.08)
                            .show_value(false)).changed() {
                            self.gate_threshold.store(threshold.to_bits(), Ordering::Relaxed);
                            self.settings.gate_threshold = threshold;
                            save_settings(&self.settings);
                        }
                        ui.label(text_caption(format!("Поріг: {:.1}%", threshold * 100.0)));
                        ui.label(text_caption("У режимі Push-to-Talk голос визначає лише клавіша, незалежно від порогу."));
                    }
                    ui.add_space(9.0);
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
                        ui.label(text_caption(format!(
                            "Відновлено Opus-кадрів: {} · відкинуто запізнілих: {}",
                            voice.concealed_frames.load(Ordering::Relaxed),
                            voice.duplicate_packets.load(Ordering::Relaxed),
                        )));
                        ui.label(text_caption("Високі значення означають затримки або втрату пакетів у VPN."));
                        if active > 0 && voice.packets_sent.load(Ordering::Relaxed) > 100
                            && voice.packets_received.load(Ordering::Relaxed) == 0 {
                            ui.label(RichText::new("Немає вхідного аудіо. Перевір UDP 45889 на комп'ютерах друзів.")
                                .size(11.0).color(RED));
                        }
                    }
                    if let Some(voice) = &self.voice {
                        let speaking: Vec<_> = self.peers.iter().filter(|p| p.voice).collect();
                        if !speaking.is_empty() {
                            ui.add_space(11.0);
                            section(ui, "ГУЧНІСТЬ КОЖНОГО ДРУГА");
                            for peer in speaking {
                                ui.label(RichText::new(&peer.name).size(12.0).color(BRIGHT));
                                let mut gain = voice.per_peer_volumes.lock().ok()
                                    .and_then(|levels| levels.get(&peer.id).copied())
                                    .unwrap_or(1.0);
                                let volume_label = format!("{:.0}%", gain * 100.0);
                                if ui.add(egui::Slider::new(&mut gain, 0.0..=2.0)
                                    .text(volume_label)).changed() {
                                    if let Ok(mut levels) = voice.per_peer_volumes.lock() {
                                        levels.insert(peer.id, gain);
                                    }
                                }
                            }
                            ui.label(text_caption("0% — не чути друга, 100% — звичайна гучність."));
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
                        ui.label(RichText::new("Камера · P2P Beta").size(14.0).strong().color(BRIGHT));
                        ui.label(RichText::new("Три режими якості й відновлення кадру при втраті одного UDP-фрагмента.")
                            .size(12.0).color(SOFT));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Джерело відео").size(12.0).color(BRIGHT));
                        let previous_index = self.camera_index.load(Ordering::Relaxed);
                        let mut chosen_index = previous_index;
                        egui::ComboBox::from_id_salt("fv_cam_select")
                            .width(ui.available_width() - 8.0)
                            .selected_text(format!("Камера {}", chosen_index + 1))
                            .show_ui(ui, |ui| {
                                for index in 0..4u32 {
                                    ui.selectable_value(&mut chosen_index, index, format!("Камера {}", index + 1));
                                }
                            });
                        if chosen_index != previous_index {
                            self.update_camera_device(chosen_index);
                        }
                        ui.label(text_caption("Вибери номер пристрою. Якщо камера не відкриється, спробуй інший номер."));
                        ui.add_space(7.0);
                        ui.label(RichText::new("Якість відео").size(12.0).color(BRIGHT));
                        let original_quality = self.video_quality.load(Ordering::Relaxed);
                        let mut selected_quality = original_quality;
                        let quality_label = match original_quality {
                            0 => "Економія · 320×180 / 5 FPS",
                            2 => "Чіткіше · 480×270 / 8 FPS",
                            _ => "Баланс · 320×180 / 9 FPS",
                        };
                        egui::ComboBox::from_id_salt("fv_video_quality")
                            .width(ui.available_width() - 8.0)
                            .selected_text(quality_label)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut selected_quality, 0, "Економія · 320×180 / 5 FPS");
                                ui.selectable_value(&mut selected_quality, 1, "Баланс · 320×180 / 9 FPS");
                                ui.selectable_value(&mut selected_quality, 2, "Чіткіше · 480×270 / 8 FPS");
                            });
                        if selected_quality != original_quality { self.update_video_quality(selected_quality); }
                        ui.add_space(8.0);
                        ui.label(text_caption("Камера вмикається лише вручну. Потрібен вхід у голосовий канал."));
                        let on=self.video.as_ref().is_some_and(|v|v.enabled());
                        if tinted_button(ui,
                            if on{"Вимкнути камеру"}else{"Увімкнути камеру"},
                            if on{Color32::from_rgb(144,55,75)}else{PURPLE},
                            self.video.is_some(),32.0).clicked(){
                            self.toggle_camera();
                        }
                        if let Some(video) = &self.video {
                            ui.label(text_caption(format!("Кадрів: надіслано {} · отримано {}",
                                video.frames_sent.load(Ordering::Relaxed),
                                video.frames_received.load(Ordering::Relaxed))));
                            ui.label(RichText::new(format!("Відновлено після втрати UDP: {}",
                                video.frames_recovered.load(Ordering::Relaxed)))
                                .size(11.0).color(GREEN));
                        }
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
                    ui.label(text_title("Версія 1.0 · Aurora Studio"));
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

    fn studio_dock(&mut self, ui: &mut Ui) {
        let compact=ui.available_width()<590.0;
        egui::Frame::none()
            .fill(Color32::from_rgb(29,35,61))
            .stroke(Stroke::new(1.0,Color32::from_rgb(68,78,117)))
            .rounding(Rounding::same(19.0))
            .inner_margin(egui::Margin::symmetric(12.0,10.0))
            .show(ui,|ui|{
                let voice=self.voice.is_some();
                let mic=self.mic_enabled.load(Ordering::Relaxed);
                let deaf=self.deafened.load(Ordering::Relaxed);
                let on_air=self.voice.as_ref().is_some_and(|v|
                    v.transmitting.load(Ordering::Relaxed));
                let meter_value=self.voice.as_ref().map(|v|
                    f32::from_bits(v.microphone_level.load(Ordering::Relaxed))
                ).unwrap_or(0.0).clamp(0.0,1.0);

                if compact {
                    ui.horizontal(|ui|{
                        dot(ui,if voice{GREEN}else{BLUE},3.0);
                        ui.label(RichText::new(if on_air{"МІКРОФОН ПЕРЕДАЄ"}
                            else if voice{"У ГОЛОСОВОМУ КАНАЛІ"}else{"P2P МЕРЕЖА ГОТОВА"})
                            .strong().size(10.5).color(if voice{GREEN}else{SOFT}));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                            if let Some(start)=self.voice_started_at {
                                let elapsed=start.elapsed().as_secs();
                                ui.label(RichText::new(format!("{:02}:{:02}",
                                    elapsed/60,elapsed%60))
                                    .size(10.8).strong().color(SOFT));
                            }
                        });
                    });
                    ui.add_space(5.0);
                }
                ui.horizontal(|ui|{
                    ui.spacing_mut().item_spacing.x=if compact{6.0}else{10.0};
                    if !compact {
                        ui.vertical(|ui|{
                            ui.horizontal(|ui|{
                                dot(ui,if voice{GREEN}else{BLUE},3.1);
                                ui.label(RichText::new(if on_air{"НА ЗВ'ЯЗКУ"}
                                    else if voice{"У ГОЛОСІ"}else{"P2P READY"})
                                    .size(10.5).strong().color(if voice{GREEN}else{BLUE}));
                            });
                            meter(ui,(meter_value*6.0).min(1.0),12,76.0);
                            ui.label(text_caption(format!("{} друзів",self.peers.len())));
                        });
                        ui.add_space(4.0);
                    }
                    if dock_action(ui,"Мікрофон",
                        if mic{"Вимкнути мікрофон · Ctrl+M / F9"}
                            else{"Увімкнути мікрофон · Ctrl+M / F9"},
                        Glyph::Mic,voice,mic && voice,!mic && voice,compact) {
                        self.mic_enabled.store(!mic,Ordering::Relaxed);
                    }
                    if dock_action(ui,"Звук",if deaf{"Увімкнути навушники"}
                        else{"Вимкнути навушники"},
                        Glyph::Headphones,voice,false,deaf && voice,compact) {
                        self.deafened.store(!deaf,Ordering::Relaxed);
                    }
                    let camera=self.video.as_ref().is_some_and(|v|v.enabled());
                    if dock_action(ui,"Камера",if camera{"Вимкнути камеру"}
                        else{"Увімкнути камеру"},Glyph::Camera,
                        self.video.is_some(),camera,false,compact) {
                        self.toggle_camera();
                    }
                    let _=dock_action(ui,"Екран","Демонстрація екрана поки недоступна",
                        Glyph::Monitor,false,false,false,compact);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                        if voice {
                            if ui.add(egui::Button::new(RichText::new(if compact{"Вийти"}
                                else{"Завершити"})
                                .strong().color(BRIGHT).size(12.0))
                                .fill(Color32::from_rgb(160,51,83))
                                .rounding(Rounding::same(11.0))
                                .min_size(Vec2::new(if compact{73.0}else{102.0},40.0))).clicked(){
                                self.toggle_voice();
                            }
                        } else if ui.add_enabled(self.mesh.is_some(),
                            egui::Button::new(RichText::new(if compact{"Увійти"}
                                else{"Увійти в голос"}).strong().color(BRIGHT).size(12.0))
                                .fill(PURPLE).rounding(Rounding::same(11.0))
                                .min_size(Vec2::new(if compact{73.0}else{117.0},40.0))
                        ).clicked() {
                            self.toggle_voice();
                        }
                    });
                });
            });
    }

    pub(super) fn neon_main(&mut self, ui: &mut Ui) {
        ui.vertical(|ui|{
            ui.add_space(2.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(30, 34, 61))
                .stroke(Stroke::new(1.0, Color32::from_rgb(63, 69, 111)))
                .rounding(Rounding::same(19.0))
                .inner_margin(egui::Margin::symmetric(18.0, 14.0))
                .show(ui, |ui|{
                    ui.horizontal(|ui|{
                        ui.vertical(|ui|{
                            ui.label(RichText::new("YOUR PRIVATE SPACE  /  01")
                                .size(10.8).strong().color(BLUE));
                            ui.add_space(1.0);
                            ui.label(RichText::new("Загальний").size(25.0)
                                .strong().color(BRIGHT));
                            ui.label(RichText::new("Простір для голосу, відео та спілкування")
                                .size(11.8).color(SOFT));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                            if icon_button(ui,"Налаштування",Glyph::Settings,
                                true,self.settings_open,false,37.0){
                                self.settings_open=!self.settings_open;
                            }
                            if icon_button(ui,"Гарячі клавіші",Glyph::Keyboard,
                                true,false,false,37.0){
                                self.settings_open=true;
                                self.settings_page=2;
                            }
                            if icon_button(ui,"Театральний режим · Ctrl+E",Glyph::Focus,
                                true,self.focus_mode,false,37.0){
                                self.focus_mode=!self.focus_mode;
                                if !self.focus_mode {self.chat_unread=0;}
                            }
                            if icon_button(ui,"Бічна панель · Ctrl+B",Glyph::Sidebar,
                                true,self.sidebar_open,false,37.0){
                                self.sidebar_open=!self.sidebar_open;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui|{
                        small_chip(ui,if self.focus_mode{"STUDIO FOCUS"}else{"AURORA STUDIO"},PURPLE);
                        small_chip(ui,if self.voice.is_some(){"ГОЛОС АКТИВНИЙ"}else{"ГОТОВИЙ ДО РОЗМОВИ"},
                            if self.voice.is_some(){GREEN}else{BLUE});
                        if let Some(start) = self.voice_started_at {
                            let elapsed = start.elapsed().as_secs();
                            small_chip(ui,&format!("ДЗВІНОК {:02}:{:02}",
                                elapsed/60,elapsed%60),GREEN);
                        }
                        if self.mesh.as_ref().is_some_and(|m| !m.local_radmin_ips.is_empty()){
                            small_chip(ui,"RADMIN VPN",GREEN);
                        }
                    });
                    ui.add_space(7.0);
                    let activity = self.last_notice.as_ref()
                        .filter(|(_,time)|time.elapsed().as_secs() < 6)
                        .map(|(message,_)|message.as_str())
                        .unwrap_or("Приватна VPN-кімната · без окремого сервера");
                    ui.label(RichText::new(activity).size(10.7).color(SOFT));
                });
            ui.add_space(13.0);

            ui.horizontal(|ui|{
                ui.label(RichText::new("УЧАСНИКИ КАНАЛУ").strong().size(11.5)
                    .color(SOFT));
                ui.add_space(7.0);
                small_chip(ui,&format!("{}/{}",self.peers.len()+1,MAX_NODES),PURPLE);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    ui.label(RichText::new(if self.focus_mode{"ТЕАТРАЛЬНИЙ РЕЖИМ"}else{"LIVE ROOM"})
                        .size(10.0).strong().color(if self.focus_mode{PURPLE}else{MUTED}));
                });
            });
            ui.add_space(8.0);

            // Responsive gallery: 3 columns on spacious desktops, 2 or 1 on
            // compact windows. It scrolls independently so chat stays usable.
            let gallery_width=ui.available_width().max(140.0);
            let columns=gallery_columns(gallery_width);
            let gap=11.0;
            let total=gallery_count(self.peers.len(),columns,self.focus_mode);
            let rows=total.div_ceil(columns);
            let regular_tile_width=((gallery_width-gap*(columns-1) as f32)
                /columns as f32).max(110.0);
            let row_height=if self.focus_mode {
                (regular_tile_width*0.65).clamp(160.0,285.0)
            } else {
                (regular_tile_width*0.53).clamp(132.0,197.0)
            };
            let needed_height=rows as f32*row_height+rows.saturating_sub(1) as f32*gap;
            let max_gallery_height=if self.focus_mode {
                (ui.available_height()-120.0).clamp(185.0,670.0)
            } else {
                ((ui.available_height()-310.0)*0.8).clamp(135.0,340.0)
            };
            let gallery_height=needed_height.min(max_gallery_height);
            egui::ScrollArea::vertical()
                .id_salt("aurora_gallery")
                .max_height(gallery_height)
                .auto_shrink([false,false])
                .show(ui,|ui|{
                    ui.set_min_height(gallery_height);
                    let mut base=0usize;
                    while base<total {
                        let row_count=(total-base).min(columns);
                        // Keep the same card width in every row. Center a
                        // partially-filled last row instead of stretching a
                        // single participant into a panoramic webcam.
                        let tile_width=regular_tile_width;
                        let tile_height=row_height;
                        let offset=(columns-row_count) as f32*(tile_width+gap)*0.5;
                        ui.horizontal(|ui|{
                            ui.spacing_mut().item_spacing.x=gap;
                            if offset > gap {ui.add_space(offset-gap);}
                            for index in base..(base+row_count) {
                                if index==0 {
                                    peer_tile(ui,tile_width,tile_height,&self.settings.nickname,
                                        if self.voice.is_some(){"Ти в голосовому каналі"}else{"Ти онлайн"},
                                        PURPLE,
                                        self.voice.as_ref().is_some_and(|v|v.transmitting.load(Ordering::Relaxed)),
                                        false,
                                        self.mesh.as_ref().and_then(|m|self.video_textures.get(&m.id)));
                                } else if let Some(peer)=self.peers.get(index-1) {
                                    let status = if peer.protocol!=PROTOCOL_VERSION {
                                        "Потрібне оновлення"
                                    }else if peer.voice{"У голосовому каналі"}else{"Онлайн"};
                                    peer_tile(ui,tile_width,tile_height,&peer.name,status,
                                        peer_color(index),
                                        self.voice.as_ref().is_some_and(|v|v.is_speaking(peer.id)),
                                        false,self.video_textures.get(&peer.id));
                                } else {
                                    peer_tile(ui,tile_width,tile_height,"","",
                                        BLUE,false,true,None);
                                }
                            }
                        });
                        base+=row_count;
                        if base<total {ui.add_space(gap);}
                    }
                });

            if self.peers.is_empty() && !self.focus_mode {
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(33,43,69))
                    .stroke(Stroke::new(1.0,BORDER))
                    .rounding(Rounding::same(13.0))
                    .inner_margin(egui::Margin::symmetric(13.0,9.0))
                    .show(ui,|ui|{
                        ui.horizontal(|ui|{
                            ui.label(RichText::new("Запроси друзів через Radmin VPN")
                                .size(11.2).strong().color(SOFT));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                if ui.add(egui::Button::new(
                                    RichText::new("Додати друга").size(11.2).color(BRIGHT))
                                    .fill(Color32::from_rgb(64,80,128))
                                    .rounding(Rounding::same(9.0))).clicked() {
                                    self.sidebar_open=true;
                                }
                            });
                        });
                    });
            }
            ui.add_space(12.0);
            // In Focus Mode, video takes priority. Chat remains one click away.
            if self.focus_mode {
                ui.add_space(14.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(28,37,62))
                    .stroke(Stroke::new(1.0,BORDER))
                    .rounding(Rounding::same(17.0))
                    .inner_margin(egui::Margin::symmetric(16.0,12.0))
                    .show(ui,|ui|{
                        ui.horizontal(|ui|{
                            ui.vertical(|ui|{
                                ui.label(RichText::new("РЕЖИМ ФОКУСУ").strong().size(12.0)
                                    .color(BLUE));
                                ui.label(RichText::new("Більше простору для облич друзів")
                                    .size(11.0).color(SOFT));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                let label=if self.chat_unread>0 {
                                    format!("Відкрити чат · {}",self.chat_unread)
                                } else {"Повернути чат".to_string()};
                                if ui.add(egui::Button::new(RichText::new(label)
                                    .color(BRIGHT).strong().size(11.5))
                                    .fill(PURPLE).rounding(Rounding::same(10.0))
                                    .min_size(Vec2::new(114.0,33.0))).clicked(){
                                    self.focus_mode=false;
                                    self.chat_unread=0;
                                }
                            });
                        });
                    });
            } else {
                ui.add_space(12.0);
                let reserved_footer=if ui.available_width()<530.0 {124.0}else{88.0};
                let chat_height=(ui.available_height()-reserved_footer-10.0).max(172.0);
                egui::Frame::none()
                    .fill(SURFACE)
                    .stroke(Stroke::new(1.0,BORDER))
                    .rounding(Rounding::same(18.0))
                    .inner_margin(egui::Margin::same(14.0))
                    .show(ui,|ui|{
                        ui.set_min_height(chat_height-28.0);
                        ui.horizontal(|ui|{
                            let (r,_) = ui.allocate_exact_size(Vec2::splat(27.0),Sense::hover());
                            glyph(ui,Glyph::Chat,r,BLUE);
                            ui.label(RichText::new("Чат · Загальний").size(15.0)
                                .strong().color(BRIGHT));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                                if icon_button(ui,"Пошук у чаті · Ctrl+F",Glyph::Search,
                                    true,self.chat_search_open,false,30.0) {
                                    self.chat_search_open=!self.chat_search_open;
                                    if !self.chat_search_open {self.chat_search.clear();}
                                }
                                ui.label(text_caption(format!("{} онлайн",self.peers.len()+1)));
                            });
                        });
                        if self.chat_search_open {
                            ui.add_space(6.0);
                            ui.horizontal(|ui|{
                                let width=(ui.available_width()-39.0).max(80.0);
                                ui.add(egui::TextEdit::singleline(&mut self.chat_search)
                                    .hint_text("Пошук за текстом або іменем…")
                                    .desired_width(width));
                                if icon_button(ui,"Очистити пошук",Glyph::Close,
                                    true,false,false,30.0) {
                                    self.chat_search.clear();
                                    self.chat_search_open=false;
                                }
                            });
                        }
                        ui.add_space(2.0);
                        ui.separator();
                        let list_height=(chat_height-
                            if self.chat_search_open{129.0}else{91.0}).max(65.0);
                        let search = self.chat_search.trim().to_lowercase();
                        egui::ScrollArea::vertical()
                            .id_salt("aurora_chat_feed")
                            .max_height(list_height)
                            .stick_to_bottom(search.is_empty())
                            .auto_shrink([false,false])
                            .show(ui,|ui|{
                                ui.set_min_height(list_height);
                                let mut found=0usize;
                                for message in &self.messages {
                                    if !search.is_empty()
                                        && !message.text.to_lowercase().contains(&search)
                                        && !message.name.to_lowercase().contains(&search) {
                                        continue;
                                    }
                                    found+=1;
                                    let mine=message.name==self.settings.nickname && !message.system;
                                    bubble_chat(ui,message,mine);
                                    ui.add_space(9.0);
                                }
                                if found==0 {
                                    ui.add_space(18.0);
                                    ui.vertical_centered(|ui|{
                                        ui.label(RichText::new("Повідомлень не знайдено")
                                            .size(13.0).strong().color(BRIGHT));
                                        ui.label(text_caption("Спробуй інший запит або закрий пошук."));
                                    });
                                }
                            });
                        ui.add_space(3.0);
                        ui.separator();
                        ui.horizontal(|ui|{
                            ui.spacing_mut().item_spacing.x=8.0;
                            let width=(ui.available_width()-47.0).max(65.0);
                            let response=ui.add_enabled(self.mesh.is_some(),
                                egui::TextEdit::singleline(&mut self.draft)
                                    .hint_text("Напиши повідомлення друзям…")
                                    .desired_width(width)
                                    .margin(egui::Margin::symmetric(10.0,8.0)));
                            if response.lost_focus() && ui.input(|i|i.key_pressed(egui::Key::Enter)){
                                self.send_chat();
                            }
                            if icon_button(ui,"Надіслати повідомлення",Glyph::Send,
                                self.mesh.is_some(),false,false,38.0) {
                                self.send_chat();
                            }
                        });
                    });
            }
            ui.add_space(10.0);

            self.studio_dock(ui);
        });
    }
}
