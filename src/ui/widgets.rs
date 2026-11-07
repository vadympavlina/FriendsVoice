//! Reusable controls: avatars, buttons, rows, segmented switch, logo.
use super::*;

pub(super) fn paint_avatar(p: &egui::Painter, center: Pos2, diameter: f32, name: &str) {
    let color = name_color(name);
    let fill = TILE.lerp_to_gamma(color, 0.34);
    p.circle_filled(center, diameter * 0.5, fill);
    let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    p.text(center, Align2::CENTER_CENTER, initial, semibold(diameter * 0.44),
        color.lerp_to_gamma(Color32::WHITE, 0.55));
}

/// Avatar with an optional green speaking ring.
pub(super) fn avatar(ui: &mut Ui, name: &str, diameter: f32, speaking: bool) {
    let pad = 3.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(diameter + pad * 2.0), Sense::hover());
    let glow = ui.ctx().animate_bool_with_time(
        egui::Id::new(("avatar-ring", name, diameter as i32)), speaking, 0.12);
    if glow > 0.01 {
        ui.painter().circle_stroke(rect.center(), diameter * 0.5 + 2.0,
            Stroke::new(2.0_f32, GREEN.linear_multiply(glow)));
    }
    paint_avatar(ui.painter(), rect.center(), diameter, name);
}

pub(super) fn icon_button(ui: &mut Ui, glyph: &str, tip: &str, active: bool, side: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let resp = resp.on_hover_text(tip);
    if active || resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(8.0), if active { RAISED } else { HOVER });
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.5_f32, ACCENT));
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph,
        FontId::proportional(side * 0.56), if active || resp.hovered() { TEXT } else { TEXT2 });
    resp.clicked()
}

pub(super) fn round_button(ui: &mut Ui, glyph: &str, tip: &str, fill: Color32, fg: Color32,
                enabled: bool, d: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::splat(d), if enabled { Sense::click() } else { Sense::hover() });
    let resp = resp.on_hover_text(tip);
    let (fill, fg) = if !enabled {
        (RAISED.linear_multiply(0.55), TEXT3.linear_multiply(0.7))
    } else if resp.hovered() {
        (fill.lerp_to_gamma(Color32::WHITE, 0.1), fg)
    } else {
        (fill, fg)
    };
    ui.painter().circle_filled(rect.center(), d * 0.5, fill);
    if resp.has_focus() {
        ui.painter().circle_stroke(rect.center(), d * 0.5 + 2.0, Stroke::new(1.5_f32, ACCENT));
    }
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph,
        FontId::proportional(d * 0.46), fg);
    enabled && resp.clicked()
}

pub(super) fn pill_button(ui: &mut Ui, label: &str, fill: Color32, fg: Color32, size: Vec2,
               enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(strong(label, 14.0, fg)).fill(fill)
            .rounding(Rounding::same(if size.y >= 44.0 { 24.0 } else { 8.0 }))
            .min_size(size),
    ).clicked()
}
pub(super) fn primary_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    pill_button(ui, label, ACCENT, ACCENT_INK, Vec2::new(0.0, 34.0), enabled)
}
pub(super) fn quiet_button(ui: &mut Ui, label: &str) -> bool {
    pill_button(ui, label, RAISED, TEXT, Vec2::new(0.0, 34.0), true)
}

/// Full-width channel-style row: icon, label, optional count badge.
pub(super) fn nav_row(ui: &mut Ui, glyph: &str, label: &str, selected: bool, enabled: bool,
           badge: usize) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 32.0),
        if enabled { Sense::click() } else { Sense::hover() });
    if selected || (enabled && resp.hovered()) {
        ui.painter().rect_filled(rect, Rounding::same(6.0), if selected { RAISED } else { HOVER.linear_multiply(0.6) });
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.5_f32, ACCENT));
    }
    let color = if !enabled { TEXT3 } else if selected || resp.hovered() { TEXT } else { TEXT2 };
    ui.painter().text(Pos2::new(rect.left() + 18.0, rect.center().y), Align2::CENTER_CENTER,
        glyph, FontId::proportional(18.0), color);
    ui.painter().text(Pos2::new(rect.left() + 36.0, rect.center().y), Align2::LEFT_CENTER,
        label, FontId::proportional(14.0), color);
    if badge > 0 {
        let label = if badge > 99 { "99+".to_string() } else { badge.to_string() };
        let badge_rect = Rect::from_center_size(
            Pos2::new(rect.right() - 16.0, rect.center().y), Vec2::new(24.0, 18.0));
        ui.painter().rect_filled(badge_rect, Rounding::same(9.0), ACCENT);
        ui.painter().text(badge_rect.center(), Align2::CENTER_CENTER, label,
            semibold(11.5), ACCENT_INK);
    }
    resp
}

/// Person row in the sidebar: avatar, name, optional trailing icon.
pub(super) fn person_row(ui: &mut Ui, indent: f32, name: &str, detail: Option<(&str, Color32)>,
              speaking: bool, trailing: Option<&str>) {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 34.0), Sense::hover());
    if resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(6.0), HOVER.linear_multiply(0.45));
    }
    let center = Pos2::new(rect.left() + indent + 14.0, rect.center().y);
    let glow = ui.ctx().animate_bool_with_time(
        egui::Id::new(("row-ring", name, indent as i32)), speaking, 0.12);
    if glow > 0.01 {
        ui.painter().circle_stroke(center, 14.0, Stroke::new(2.0_f32, GREEN.linear_multiply(glow)));
    }
    paint_avatar(ui.painter(), center, 24.0, name);
    let text_left = rect.left() + indent + 36.0;
    let max_width = rect.right() - text_left - if trailing.is_some() { 30.0 } else { 8.0 };
    let name_color = if speaking { TEXT } else { TEXT2 };
    let galley = truncated(ui, name, FontId::proportional(14.0), name_color, max_width);
    match detail {
        Some((detail, detail_color)) => {
            let top = rect.center().y - galley.size().y * 0.5 - 6.0;
            ui.painter().galley(Pos2::new(text_left, top), galley, name_color);
            ui.painter().text(Pos2::new(text_left, rect.center().y + 9.0), Align2::LEFT_CENTER,
                detail, FontId::proportional(11.5), detail_color);
        }
        None => {
            let top = rect.center().y - galley.size().y * 0.5;
            ui.painter().galley(Pos2::new(text_left, top), galley, name_color);
        }
    }
    if let Some(glyph) = trailing {
        ui.painter().text(Pos2::new(rect.right() - 16.0, rect.center().y), Align2::CENTER_CENTER,
            glyph, FontId::proportional(16.0), TEXT3);
    }
}

pub(super) fn section_label(ui: &mut Ui, label: &str, trailing: Option<String>) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(strong(label, 12.0, TEXT3));
        if let Some(trailing) = trailing {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(text(trailing, 12.0, TEXT3));
            });
        }
    });
    ui.add_space(3.0);
}

/// A row of mutually exclusive choices. Returns the index clicked, if any.
pub(super) fn segmented(ui: &mut Ui, labels: &[String], selected: usize) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (index, label) in labels.iter().enumerate() {
            let on = index == selected;
            let button = egui::Button::new(if on { strong(label.as_str(), 13.0, ACCENT_INK) } else { text(label.as_str(), 13.0, TEXT) })
                .fill(if on { ACCENT } else { RAISED })
                .rounding(Rounding::same(8.0))
                .min_size(Vec2::new(0.0, 32.0));
            if ui.add(button).clicked() { clicked = Some(index); }
        }
    });
    clicked
}

pub(super) fn level_bar(ui: &mut Ui, level: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(4.0), RAISED);
    let fill = Rect::from_min_size(rect.min, Vec2::new(rect.width() * level.clamp(0.0, 1.0), rect.height()));
    ui.painter().rect_filled(fill, Rounding::same(4.0), if level > 0.85 { WARN } else { GREEN });
}

/// The mascot logo. Falls back to a plain accent circle if the texture is missing.
pub(super) fn brand_mark(ui: &mut Ui, side: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    match branding::logo_texture(ui.ctx(), side > 40.0) {
        Some(texture) => {
            ui.painter().image(texture.id(), rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => { ui.painter().circle_filled(rect.center(), side * 0.45, ACCENT); }
    }
}

