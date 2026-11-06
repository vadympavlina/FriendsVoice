//! Native egui interface, laid out like a voice chat client:
//! channel rail on the left, call stage in the middle, text chat on the right.
//!
//! Design rules: flat surfaces separated by tone (not borders or cards), one
//! violet accent (taken from the mascot) used only for primary actions, green only for "speaking /
//! online", red only for destructive or muted states, sentence-case text.
use super::*;
use eframe::egui::{
    self, text::LayoutJob, Align2, FontFamily, FontId, Margin, Pos2, Rect, Response, RichText,
    Rounding, Sense, Stroke, Ui, Vec2,
};
use crate::screen;
use egui_phosphor::regular as ph;
use std::sync::Arc;

// ── Palette ────────────────────────────────────────────────────────────────
const RAIL: Color32 = Color32::from_rgb(17, 18, 20);
const USERBAR: Color32 = Color32::from_rgb(13, 14, 16);
const STAGE: Color32 = Color32::from_rgb(22, 23, 26);
const CHAT: Color32 = Color32::from_rgb(28, 29, 33);
const TILE: Color32 = Color32::from_rgb(35, 36, 41);
const RAISED: Color32 = Color32::from_rgb(42, 43, 49);
const HOVER: Color32 = Color32::from_rgb(52, 53, 60);
const LINE: Color32 = Color32::from_rgb(40, 41, 46);
const TEXT: Color32 = Color32::from_rgb(236, 235, 231);
const TEXT2: Color32 = Color32::from_rgb(170, 171, 178);
const TEXT3: Color32 = Color32::from_rgb(128, 129, 138);
const ACCENT: Color32 = Color32::from_rgb(158, 128, 252);
/// Warnings (not the brand accent).
const WARN: Color32 = Color32::from_rgb(240, 177, 74);
const ACCENT_INK: Color32 = Color32::from_rgb(24, 14, 52);
const GREEN: Color32 = Color32::from_rgb(79, 209, 139);
const RED: Color32 = Color32::from_rgb(226, 84, 92);

/// Muted identity colours. A friend keeps the same colour in the sidebar,
/// the call tiles and the chat because it is derived from their name.
const IDENTITY: [Color32; 5] = [
    Color32::from_rgb(111, 143, 216),
    Color32::from_rgb(212, 121, 154),
    Color32::from_rgb(84, 176, 174),
    Color32::from_rgb(155, 136, 220),
    Color32::from_rgb(196, 165, 116),
];

fn name_color(name: &str) -> Color32 {
    let mut hash: u32 = 2166136261;
    for byte in name.bytes() {
        hash = (hash ^ byte as u32).wrapping_mul(16777619);
    }
    IDENTITY[(hash % IDENTITY.len() as u32) as usize]
}

// ── Theme ──────────────────────────────────────────────────────────────────
pub fn install_theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("plex".into(), egui::FontData::from_static(
        include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf")));
    fonts.font_data.insert("plex-semibold".into(), egui::FontData::from_static(
        include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf")));
    fonts.font_data.insert("plex-mono".into(), egui::FontData::from_static(
        include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")));
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "plex".into());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.insert(0, "plex-mono".into());
    }
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts.families.insert(
        FontFamily::Name("semibold".into()),
        vec!["plex-semibold".into(), "plex".into(), "phosphor".into()],
    );
    ctx.set_fonts(fonts);
    branding::install_logo_textures(ctx);

    let mut v = egui::Visuals::dark();
    v.panel_fill = STAGE;
    v.window_fill = CHAT;
    v.extreme_bg_color = RAISED; // text inputs
    v.faint_bg_color = CHAT;
    v.window_rounding = Rounding::same(12.0);
    v.menu_rounding = Rounding::same(8.0);
    v.window_stroke = Stroke::new(1.0_f32, LINE);
    v.window_shadow = egui::Shadow { offset: Vec2::new(0.0, 8.0), blur: 24.0, spread: 0.0,
        color: Color32::from_black_alpha(110) };
    v.popup_shadow = v.window_shadow;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = Color32::from_rgb(86, 66, 150);
    v.selection.stroke = Stroke::new(1.0_f32, TEXT);
    v.slider_trailing_fill = true;
    let radius = Rounding::same(8.0);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT2);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.noninteractive.rounding = radius;
    for (state, fill) in [
        (&mut v.widgets.inactive, RAISED),
        (&mut v.widgets.hovered, HOVER),
        (&mut v.widgets.active, Color32::from_rgb(64, 65, 73)),
        (&mut v.widgets.open, HOVER),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::NONE;
        state.fg_stroke = Stroke::new(1.0_f32, TEXT);
        state.rounding = radius;
    }
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.expansion = 0.0;
    ctx.set_visuals(v);

    ctx.style_mut(|style| {
        use egui::TextStyle::*;
        style.text_styles.insert(Body, FontId::proportional(14.0));
        style.text_styles.insert(Button, FontId::proportional(14.0));
        style.text_styles.insert(Small, FontId::proportional(12.0));
        style.text_styles.insert(Heading, FontId::new(20.0, FontFamily::Name("semibold".into())));
        style.text_styles.insert(Monospace, FontId::monospace(13.0));
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(14.0, 7.0);
        style.spacing.interact_size.y = 32.0;
        style.spacing.combo_width = 220.0;
        style.spacing.slider_width = 220.0;
        style.spacing.scroll.bar_width = 6.0;
        style.spacing.scroll.floating = true;
    });
}

// ── Text helpers ───────────────────────────────────────────────────────────
fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
fn text(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).size(size).color(color)
}
fn strong(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(semibold(size)).color(color)
}
fn mono(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(FontId::monospace(size)).color(color)
}
fn truncated(ui: &Ui, s: &str, font: FontId, color: Color32, max_width: f32) -> Arc<egui::Galley> {
    let mut job = LayoutJob::simple_singleline(s.to_string(), font, color);
    job.wrap.max_width = max_width.max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.fonts(|fonts| fonts.layout_job(job))
}
fn clock(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

// ── Small widgets ──────────────────────────────────────────────────────────
fn paint_avatar(p: &egui::Painter, center: Pos2, diameter: f32, name: &str) {
    let color = name_color(name);
    let fill = TILE.lerp_to_gamma(color, 0.34);
    p.circle_filled(center, diameter * 0.5, fill);
    let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    p.text(center, Align2::CENTER_CENTER, initial, semibold(diameter * 0.44),
        color.lerp_to_gamma(Color32::WHITE, 0.55));
}

/// Avatar with an optional green speaking ring.
fn avatar(ui: &mut Ui, name: &str, diameter: f32, speaking: bool) {
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

fn icon_button(ui: &mut Ui, glyph: &str, tip: &str, active: bool, side: f32) -> bool {
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

fn round_button(ui: &mut Ui, glyph: &str, tip: &str, fill: Color32, fg: Color32,
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

fn pill_button(ui: &mut Ui, label: &str, fill: Color32, fg: Color32, size: Vec2,
               enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(strong(label, 14.0, fg)).fill(fill)
            .rounding(Rounding::same(if size.y >= 44.0 { 24.0 } else { 8.0 }))
            .min_size(size),
    ).clicked()
}
fn primary_button(ui: &mut Ui, label: &str, enabled: bool) -> bool {
    pill_button(ui, label, ACCENT, ACCENT_INK, Vec2::new(0.0, 34.0), enabled)
}
fn quiet_button(ui: &mut Ui, label: &str) -> bool {
    pill_button(ui, label, RAISED, TEXT, Vec2::new(0.0, 34.0), true)
}

/// Full-width channel-style row: icon, label, optional count badge.
fn nav_row(ui: &mut Ui, glyph: &str, label: &str, selected: bool, enabled: bool,
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
fn person_row(ui: &mut Ui, indent: f32, name: &str, detail: Option<(&str, Color32)>,
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

fn section_label(ui: &mut Ui, label: &str, trailing: Option<String>) {
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
fn segmented(ui: &mut Ui, labels: &[String], selected: usize) -> Option<usize> {
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

fn level_bar(ui: &mut Ui, level: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(4.0), RAISED);
    let fill = Rect::from_min_size(rect.min, Vec2::new(rect.width() * level.clamp(0.0, 1.0), rect.height()));
    ui.painter().rect_filled(fill, Rounding::same(4.0), if level > 0.85 { WARN } else { GREEN });
}

/// The mascot logo. Falls back to a plain accent circle if the texture is missing.
fn brand_mark(ui: &mut Ui, side: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    match branding::logo_texture(ui.ctx(), side > 40.0) {
        Some(texture) => {
            ui.painter().image(texture.id(), rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => { ui.painter().circle_filled(rect.center(), side * 0.45, ACCENT); }
    }
}

// ── Call grid ──────────────────────────────────────────────────────────────
const TILE_ASPECT: f32 = 16.0 / 9.0;

/// Pick the column count that gives the biggest 16:9 tiles inside `w × h`.
/// Returns `(columns, tile_width, tile_height)`.
fn best_grid(count: usize, w: f32, h: f32, gap: f32) -> (usize, f32, f32) {
    let count = count.max(1);
    let (w, h) = (w.max(1.0), h.max(1.0));
    let mut best = (1usize, 0.0f32);
    for columns in 1..=count {
        let rows = count.div_ceil(columns);
        let cell_w = (w - gap * (columns - 1) as f32) / columns as f32;
        let cell_h = (h - gap * (rows - 1) as f32) / rows as f32;
        let fit = cell_w.min(cell_h * TILE_ASPECT).max(1.0);
        if fit > best.1 + 0.5 {
            best = (columns, fit);
        }
    }
    let width = best.1.min(980.0);
    (best.0, width, width / TILE_ASPECT)
}

/// What a tile shows.
struct TileView<'a> {
    name: &'a str,
    note: Option<&'a str>,
    speaking: bool,
    badge: Option<&'a str>,
    /// Fit the whole picture inside the tile (screens) instead of filling it (cameras).
    contain: bool,
    /// Small strip tile: smaller text, no avatar glow.
    small: bool,
    salt: (u64, StreamKind),
}

fn paint_tile(ui: &mut Ui, rect: Rect, view: &TileView<'_>, video: Option<&egui::TextureHandle>) -> Response {
    let response = ui.interact(rect, egui::Id::new(("tile", view.salt.0, view.salt.1.index())), Sense::click());
    let p = ui.painter_at(rect);
    let rounding = Rounding::same(if view.small { 8.0 } else { 12.0 });
    p.rect_filled(rect, rounding, TILE);
    if let Some(frame) = video {
        let [texture_w, texture_h] = frame.size();
        let picture_aspect = texture_w as f32 / texture_h.max(1) as f32;
        if view.contain {
            // Screens are never cropped: the whole picture is shown, centred.
            let (w, h) = if rect.width() / rect.height() > picture_aspect {
                (rect.height() * picture_aspect, rect.height())
            } else {
                (rect.width(), rect.width() / picture_aspect)
            };
            let fitted = Rect::from_center_size(rect.center(), Vec2::new(w, h));
            let mut shape = egui::epaint::RectShape::filled(fitted, Rounding::same(4.0), Color32::WHITE);
            shape.fill_texture_id = frame.id();
            shape.uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            p.add(shape);
        } else {
            // Cameras fill the tile: crop the picture's edges instead of stretching it.
            let aspect = (rect.width() / rect.height()).max(0.01);
            let uv = if aspect < picture_aspect {
                let visible = (aspect / picture_aspect).clamp(0.01, 1.0);
                Rect::from_min_max(Pos2::new((1.0 - visible) * 0.5, 0.0), Pos2::new((1.0 + visible) * 0.5, 1.0))
            } else {
                let visible = (picture_aspect / aspect).clamp(0.01, 1.0);
                Rect::from_min_max(Pos2::new(0.0, (1.0 - visible) * 0.5), Pos2::new(1.0, (1.0 + visible) * 0.5))
            };
            let mut shape = egui::epaint::RectShape::filled(rect, rounding, Color32::WHITE);
            shape.fill_texture_id = frame.id();
            shape.uv = uv;
            p.add(shape);
        }
    } else {
        let diameter = (rect.height() * 0.36).clamp(24.0, 120.0);
        paint_avatar(&p, rect.center() - Vec2::new(0.0, if view.small { 0.0 } else { 6.0 }), diameter, view.name);
    }

    // Name label.
    let label = match view.note {
        Some(note) => format!("{}  ·  {note}", view.name),
        None => view.name.to_string(),
    };
    let font = FontId::proportional(if view.small { 11.5 } else { 13.0 });
    let galley = truncated(ui, &label, font, TEXT, rect.width() - 28.0);
    let size = galley.size() + Vec2::new(14.0, 6.0);
    let pill = Rect::from_min_size(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0 - size.y), size);
    p.rect_filled(pill, Rounding::same(6.0), Color32::from_black_alpha(165));
    p.galley(pill.min + Vec2::new(7.0, 3.0), galley, TEXT);

    if let Some(glyph) = view.badge {
        let center = Pos2::new(rect.right() - 22.0, rect.top() + 22.0);
        p.circle_filled(center, 14.0, RED);
        p.text(center, Align2::CENTER_CENTER, glyph, FontId::proportional(16.0), Color32::WHITE);
    }
    let glow = ui.ctx().animate_bool_with_time(
        egui::Id::new(("tile-ring", view.salt.0, view.salt.1.index())), view.speaking, 0.12);
    if glow > 0.01 {
        p.rect_stroke(rect.shrink(1.25), rounding, Stroke::new(2.5_f32, GREEN.linear_multiply(glow)));
    } else if response.hovered() && view.small {
        p.rect_stroke(rect.shrink(1.0), rounding, Stroke::new(1.5_f32, ACCENT));
    }
    if response.hovered() && view.small {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// Layout of a spotlight (one big tile) plus a strip of the others.
/// Returns `(big tile area, strip tile rects)`.
fn spotlight_layout(area: Rect, others: usize, gap: f32) -> (Rect, Vec<Rect>) {
    if others == 0 { return (area, Vec::new()); }
    let n = others as f32;
    if area.width() >= 820.0 {
        let strip_w = (area.width() * 0.2).clamp(190.0, 260.0);
        let main = Rect::from_min_max(area.min, Pos2::new(area.max.x - strip_w - gap, area.max.y));
        let tile_h = (strip_w / TILE_ASPECT).min((area.height() - gap * (n - 1.0)) / n).max(48.0);
        let rects = (0..others).map(|i| Rect::from_min_size(
            Pos2::new(area.max.x - strip_w, area.min.y + i as f32 * (tile_h + gap)),
            Vec2::new(strip_w, tile_h))).collect();
        (main, rects)
    } else {
        let strip_h = (area.height() * 0.22).clamp(64.0, 110.0);
        let main = Rect::from_min_max(area.min, Pos2::new(area.max.x, area.max.y - strip_h - gap));
        let tile_w = (strip_h * TILE_ASPECT).min((area.width() - gap * (n - 1.0)) / n).max(40.0);
        let total = tile_w * n + gap * (n - 1.0);
        let left = area.center().x - total * 0.5;
        let rects = (0..others).map(|i| Rect::from_min_size(
            Pos2::new(left.max(area.min.x) + i as f32 * (tile_w + gap), area.max.y - strip_h),
            Vec2::new(tile_w, strip_h))).collect();
        (main, rects)
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn fits(count: usize, w: f32, h: f32) {
        let gap = 12.0;
        let (columns, tw, th) = best_grid(count, w, h, gap);
        let rows = count.div_ceil(columns);
        assert!(columns >= 1 && columns <= count.max(1));
        assert!(columns as f32 * tw + gap * (columns - 1) as f32 <= w + 0.6, "{count} wide");
        assert!(rows as f32 * th + gap * (rows - 1) as f32 <= h + 0.6, "{count} tall");
        assert!((tw / th - TILE_ASPECT).abs() < 0.01);
    }

    #[test]
    fn grid_always_fits_the_stage() {
        for count in 1..=5 {
            for (w, h) in [(380.0, 300.0), (700.0, 420.0), (1100.0, 640.0), (1800.0, 900.0)] {
                fits(count, w, h);
            }
        }
    }

    #[test]
    fn single_tile_uses_the_available_area() {
        let (columns, tw, _) = best_grid(1, 800.0, 600.0, 12.0);
        assert_eq!(columns, 1);
        assert!((tw - 800.0).abs() < 1.0);
    }

    #[test]
    fn wide_stage_puts_friends_side_by_side() {
        let (columns, _, _) = best_grid(2, 1400.0, 500.0, 12.0);
        assert_eq!(columns, 2);
        let (columns, _, _) = best_grid(2, 500.0, 1000.0, 12.0);
        assert_eq!(columns, 1);
    }

    #[test]
    fn spotlight_layout_keeps_everything_inside_and_apart() {
        for (w, h) in [(1100.0_f32, 620.0_f32), (700.0, 420.0), (900.0, 500.0), (400.0, 300.0)] {
            let area = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(w, h));
            for others in 0..=6 {
                let (main, strip) = spotlight_layout(area, others, 10.0);
                assert_eq!(strip.len(), others);
                assert!(main.width() > 100.0 && main.height() > 60.0, "{w}x{h} n={others}: main {main:?}");
                for rect in &strip {
                    assert!(area.expand(0.5).contains_rect(*rect), "{w}x{h} n={others}: {rect:?} outside {area:?}");
                    assert!(!main.shrink(0.5).intersects(*rect), "{w}x{h} n={others}: strip overlaps main");
                }
                for pair in strip.windows(2) {
                    assert!(!pair[0].shrink(0.5).intersects(pair[1]), "strip tiles overlap");
                }
            }
        }
    }

    #[test]
    fn identity_colour_is_stable_per_name() {
        assert_eq!(name_color("Вадим"), name_color("Вадим"));
    }

    #[test]
    fn clock_formats_minutes_and_hours() {
        assert_eq!(clock(65), "01:05");
        assert_eq!(clock(3725), "1:02:05");
    }
}

// ── Screens ────────────────────────────────────────────────────────────────
impl FriendsApp {
    pub(super) fn render(&mut self, ctx: &egui::Context) {
        let narrow = ctx.screen_rect().width() < 1100.0;
        if self.sidebar_open {
            egui::SidePanel::left("rail").resizable(false).exact_width(if narrow { 232.0 } else { 252.0 })
                .frame(egui::Frame::none().fill(RAIL))
                .show(ctx, |ui| self.rail(ui));
        }
        if self.settings_open {
            egui::SidePanel::left("settings_nav").resizable(false).exact_width(220.0)
                .frame(egui::Frame::none().fill(Color32::from_rgb(20, 21, 24))
                    .inner_margin(Margin::symmetric(12.0, 20.0)))
                .show(ctx, |ui| self.settings_nav(ui));
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(CHAT).inner_margin(Margin::symmetric(40.0, 24.0)))
                .show(ctx, |ui| self.settings_page_ui(ui));
            return;
        }
        if !self.focus_mode {
            egui::SidePanel::right("chat").resizable(true)
                .default_width(if narrow { 300.0 } else { 360.0 })
                .width_range(270.0..=540.0)
                .frame(egui::Frame::none().fill(CHAT))
                .show(ctx, |ui| self.chat_panel(ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::none().fill(STAGE))
            .show(ctx, |ui| self.stage(ui));
        if self.share_picker_open {
            self.share_picker(ctx);
        }
    }

    // Left rail ─────────────────────────────────────────────────────────────
    fn rail(&mut self, ui: &mut Ui) {
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

    fn network_block(&mut self, ui: &mut Ui) {
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

    fn user_bar(&mut self, ui: &mut Ui) {
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

    // Stage ─────────────────────────────────────────────────────────────────
    fn stage(&mut self, ui: &mut Ui) {
        egui::TopBottomPanel::top("stage_top").exact_height(52.0)
            .frame(egui::Frame::none().fill(STAGE).inner_margin(Margin::symmetric(18.0, 0.0)))
            .show_inside(ui, |ui| {
                let bar = ui.max_rect();
                ui.painter().hline(bar.x_range(), bar.bottom(), Stroke::new(1.0_f32, LINE));
                ui.horizontal_centered(|ui| {
                    ui.label(text(ph::SPEAKER_HIGH, 19.0, TEXT3));
                    ui.label(strong("Загальний", 15.5, TEXT));
                    if let Some(start) = self.voice_started_at {
                        ui.add_space(6.0);
                        ui.label(mono(clock(start.elapsed().as_secs()), 13.0, GREEN));
                    }
                    if self.voice.is_some() && self.push_to_talk.load(Ordering::Relaxed) {
                        let talking = self.voice.as_ref()
                            .is_some_and(|v| v.transmitting.load(Ordering::Relaxed));
                        ui.add_space(6.0);
                        ui.label(text(format!("Тримай {}, щоб говорити",
                            voice::vk_name(self.ptt_key.load(Ordering::Relaxed))),
                            12.5, if talking { GREEN } else { TEXT3 }));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if icon_button(ui, ph::GEAR, "Налаштування", false, 34.0) {
                            self.settings_open = true;
                        }
                        if icon_button(ui,
                            if self.focus_mode { ph::CORNERS_IN } else { ph::CORNERS_OUT },
                            if self.focus_mode { "Повернути чат · Ctrl+E" } else { "Театральний режим · Ctrl+E" },
                            self.focus_mode, 34.0) {
                            self.focus_mode = !self.focus_mode;
                            if !self.focus_mode { self.chat_unread = 0; }
                        }
                        if icon_button(ui, ph::SIDEBAR_SIMPLE, "Бічна панель · Ctrl+B",
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
    fn lobby(&mut self, ui: &mut Ui, area: Rect) {
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
                ui.label(strong("Загальний голосовий канал", 22.0, TEXT));
                ui.add_space(2.0);
                ui.label(text(
                    if in_channel.is_empty() { "У каналі поки нікого".to_string() }
                    else { format!("У каналі: {}", in_channel.join(", ")) },
                    14.0, TEXT2));
                ui.add_space(16.0);
                if pill_button(ui, "Приєднатися до голосу", ACCENT, ACCENT_INK, Vec2::new(250.0, 48.0), self.mesh.is_some()) {
                    self.toggle_voice();
                }
                if self.mesh.is_none() {
                    ui.add_space(6.0);
                    ui.label(text("Спершу відновіть з'єднання в лівій панелі.", 12.5, RED));
                }
                if nobody_online {
                    ui.add_space(26.0);
                    egui::Frame::none().fill(CHAT).rounding(Rounding::same(12.0)).inner_margin(Margin::same(16.0)).show(ui, |ui| {
                        ui.set_width(width - 32.0);
                        ui.label(strong("Як позвати друзів", 14.0, TEXT));
                        ui.add_space(4.0);
                        for step in [
                            "1. Усі встановлюють Radmin VPN і заходять в одну мережу.",
                            "2. Друзі знайдуться самі. Якщо ні, обміняйтеся адресами Radmin.",
                            "3. Натисни «Приєднатися до голосу».",
                        ] {
                            ui.add(egui::Label::new(text(step, 13.0, TEXT2)).wrap());
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if let Some(ip) = ip {
                                if quiet_button(ui, "Скопіювати мою адресу") {
                                    ui.output_mut(|o| o.copied_text = ip.to_string());
                                    self.ip_copied_at = Some(Instant::now());
                                    self.notice(format!("Адресу {ip} скопійовано. Надішли її другові."));
                                }
                            }
                            if quiet_button(ui, "Додати друга за адресою") {
                                self.sidebar_open = true;
                                self.add_friend_open = true;
                            }
                        });
                    });
                }
            });
    }

    /// Everything that can be on stage: people's cameras and shared screens.
    fn stage_tiles(&mut self, ui: &mut Ui, area: Rect) {
        let in_voice = self.voice.is_some();
        let local_id = self.mesh.as_ref().map(|m| m.id).unwrap_or(0);
        struct Tile { key: (u64, StreamKind), name: String, note: Option<String>, speaking: bool,
                      badge: Option<&'static str>, contain: bool }
        let mut tiles = vec![Tile {
            key: (local_id, StreamKind::Camera),
            name: self.settings.nickname.clone(),
            note: Some("ти".into()),
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
                note: (peer.protocol != PROTOCOL_VERSION).then(|| "потрібне оновлення".to_string()),
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
                (self.settings.nickname.clone(), "твій показ екрана".to_string())
            } else {
                (self.peers.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_default(),
                 "показує екран".to_string())
            };
            tiles.push(Tile { key: (id, StreamKind::Screen), name, note: Some(note), speaking: false, badge: None, contain: true });
        }

        // A friend's screen automatically becomes the big picture; a click pins any tile instead.
        let automatic = tiles.iter().find(|t| t.key.1 == StreamKind::Screen && t.key.0 != local_id).map(|t| t.key);
        let spotlight = self.spotlight.filter(|k| tiles.iter().any(|t| t.key == *k)).or(automatic);
        let gap = 12.0;
        let mut clicked: Option<(u64, StreamKind)> = None;
        let mut double_clicked = false;

        if let Some(big) = spotlight {
            let others: Vec<&Tile> = tiles.iter().filter(|t| t.key != big).collect();
            let (main, strip) = spotlight_layout(area, others.len(), gap);
            if let Some(tile) = tiles.iter().find(|t| t.key == big) {
                let view = TileView { name: &tile.name, note: tile.note.as_deref(), speaking: tile.speaking,
                    badge: tile.badge, contain: tile.contain, small: false, salt: tile.key };
                let response = paint_tile(ui, main, &view, self.video_textures.get(&tile.key));
                if response.double_clicked() { double_clicked = true; }
                response.on_hover_text("Двічі клікни, щоб розгорнути на весь екран");
            }
            for (tile, rect) in others.iter().zip(strip) {
                let view = TileView { name: &tile.name, note: tile.note.as_deref(), speaking: tile.speaking,
                    badge: tile.badge, contain: tile.contain, small: true, salt: tile.key };
                if paint_tile(ui, rect, &view, self.video_textures.get(&tile.key)).clicked() {
                    clicked = Some(tile.key);
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
                let response = paint_tile(ui, rect, &view, self.video_textures.get(&tile.key));
                if response.clicked() && tiles.len() > 1 { clicked = Some(tile.key); }
                if tiles.len() > 1 { response.on_hover_text("Клікни, щоб збільшити"); }
            }
            if alone {
                let center = Pos2::new(area.center().x, top + total_h + 26.0);
                ui.painter().text(center, Align2::CENTER_CENTER,
                    "Тут поки тільки ти. Додай друга за IP у лівій панелі.",
                    FontId::proportional(13.5), TEXT3);
            }
        }
        if let Some(key) = clicked { self.spotlight = Some(key); }
        if double_clicked {
            self.focus_mode = !self.focus_mode;
            if !self.focus_mode { self.chat_unread = 0; }
        }
    }

    /// Strip under the top bar while you are sharing your screen.
    fn share_banner(&mut self, ui: &mut Ui) {
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
                    let name = truncated(ui, &format!("Ти показуєш: {title}"), semibold(14.0), TEXT, (ui.available_width() * 0.42).max(120.0));
                    ui.label(egui::WidgetText::Galley(name));
                    let detail = if width == 0 { "запуск…".to_string() } else {
                        format!("{width}×{height} · {fps} к/с · {} Мбіт/с", format!("{mbps:.1}").replace('.', ","))
                    };
                    ui.label(text(detail, 12.5, TEXT2));
                    if viewers == 0 { ui.label(text("У каналі поки ніхто не бачить показ", 12.5, TEXT3)); }
                    else { ui.label(text(format!("дивляться: {viewers}"), 12.5, TEXT2)); }
                    if overloaded {
                        ui.label(text(format!("{} Комп'ютер не встигає: зменш якість або частоту кадрів", ph::WARNING), 12.5, WARN));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if pill_button(ui, "Зупинити показ", RED, Color32::WHITE, Vec2::new(0.0, 30.0), true) { stop = true; }
                    });
                });
            });
        if stop { self.stop_screen_share(None); }
    }

    /// "What do you want to show?" window: pick a screen or a window and the quality.
    fn share_picker(&mut self, ctx: &egui::Context) {
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
                ui.label(strong("Що показати друзям?", 19.0, TEXT));
                ui.add_space(2.0);
                Self::hint(ui, "Друзі в каналі побачать це великим зображенням. Звук комп'ютера поки не передається.");
                ui.add_space(12.0);

                let screens = self.share_sources.iter().filter(|s| !s.is_window()).count();
                let windows = self.share_sources.iter().filter(|s| s.is_window()).count();
                ui.horizontal(|ui| {
                    if let Some(tab) = segmented(ui, &[format!("Екран ({screens})"), format!("Вікно ({windows})")], self.share_tab) {
                        if tab != self.share_tab {
                            self.share_tab = tab;
                            self.share_selected = self.share_sources.iter().position(|s| s.is_window() == (tab == 1));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, ph::ARROWS_CLOCKWISE, "Оновити список", false, 32.0) { refresh = true; }
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
                                    ui.label(strong(if want_windows { "Немає вікон для показу" } else { "Екрани не знайдено" }, 14.0, TEXT));
                                    ui.label(text("Відкрий потрібну програму й натисни «Оновити список».", 12.5, TEXT3));
                                } else {
                                    ui.label(strong("Показ екрана працює лише у Windows", 14.0, TEXT));
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

                ui.label(strong("Якість", 14.0, TEXT));
                ui.add_space(4.0);
                ui.label(text("Роздільність", 12.5, TEXT3));
                let labels: Vec<String> = screen::RESOLUTIONS.iter().map(|(_, l)| l.to_string()).collect();
                let current = screen::RESOLUTIONS.iter().position(|(h, _)| *h == self.share_quality.max_height).unwrap_or(1);
                if let Some(i) = segmented(ui, &labels, current) { self.share_quality.max_height = screen::RESOLUTIONS[i].0; }
                ui.add_space(6.0);
                ui.label(text("Кадрів за секунду", 12.5, TEXT3));
                let labels: Vec<String> = screen::FRAME_RATES.iter().map(|f| f.to_string()).collect();
                let current = screen::FRAME_RATES.iter().position(|f| *f == self.share_quality.fps).unwrap_or(1);
                if let Some(i) = segmented(ui, &labels, current) { self.share_quality.fps = screen::FRAME_RATES[i]; }
                ui.add_space(8.0);

                let source_size = self.share_selected.and_then(|i| self.share_sources.get(i)).map(|s| s.size)
                    .filter(|(w, h)| *w > 0 && *h > 0).unwrap_or((1920, 1080));
                let (out_w, out_h) = self.share_quality.output_size(source_size.0, source_size.1);
                let mbps = self.share_quality.bitrate(out_w, out_h) as f32 / 1_000_000.0;
                let each = format!("{mbps:.1}").replace('.', ",");
                let total = if friends > 1 { format!(", разом ≈{} Мбіт/с", format!("{:.1}", mbps * friends as f32).replace('.', ",")) } else { String::new() };
                Self::hint(ui, format!("Буде надіслано {out_w}×{out_h}, {} к/с: ≈{each} Мбіт/с кожному другові{total}.", self.share_quality.fps));
                if self.share_quality.fps == 60 || self.share_quality.max_height >= 1440 {
                    ui.label(text("Висока якість сильно навантажує процесор і канал. Якщо показ смикається, зменш її.", 12.5, WARN));
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, "Почати показ", self.share_selected.is_some()) { start = true; }
                        if quiet_button(ui, "Скасувати") { close = true; }
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

    fn call_bar(&mut self, ui: &mut Ui) {
        let voice = self.voice.is_some();
        let mic = self.mic_enabled.load(Ordering::Relaxed);
        let deaf = self.deafened.load(Ordering::Relaxed);
        let camera = self.video.as_ref().is_some_and(|v| v.enabled());
        let camera_starting = self.video.as_ref().is_some_and(|v| v.starting());
        let chat_button = self.focus_mode;
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
                    if mic { "Вимкнути мікрофон · Ctrl+M" } else { "Увімкнути мікрофон · Ctrl+M" },
                    if mic { RAISED } else { RED }, TEXT, true, d) {
                    self.mic_enabled.store(!mic, Ordering::Relaxed);
                }
                if round_button(ui, ph::HEADPHONES,
                    if deaf { "Увімкнути звук" } else { "Вимкнути звук" },
                    if deaf { RED } else { RAISED }, TEXT, true, d) {
                    self.deafened.store(!deaf, Ordering::Relaxed);
                }
                if round_button(ui, if camera { ph::VIDEO_CAMERA } else { ph::VIDEO_CAMERA_SLASH },
                    if camera_starting { "Камера запускається. Натисни, щоб скасувати." }
                        else if camera { "Вимкнути камеру" } else { "Увімкнути камеру" },
                    if camera_starting { ACCENT } else if camera { TEXT } else { RAISED },
                    if camera_starting { ACCENT_INK } else if camera { STAGE } else { TEXT },
                    self.mesh.is_some(), d) {
                    self.toggle_camera();
                }
                let sharing = self.screen.is_some();
                if round_button(ui, ph::MONITOR_ARROW_UP,
                    if sharing { "Зупинити показ екрана" } else { "Показати екран або вікно" },
                    if sharing { ACCENT } else { RAISED }, if sharing { ACCENT_INK } else { TEXT }, true, d) {
                    if sharing { self.stop_screen_share(None); } else { self.open_share_picker(); }
                }
                ui.add_space(6.0);
                if pill_button(ui, if leave_width > 120.0 { "Вийти з каналу" } else { "Вийти" }, RED, Color32::WHITE, Vec2::new(leave_width, d), true) {
                    self.toggle_voice();
                }
            } else if pill_button(ui, "Приєднатися до голосу", ACCENT, ACCENT_INK,
                Vec2::new(join_width, d), self.mesh.is_some()) {
                self.toggle_voice();
            }
            if chat_button {
                let unread = self.chat_unread;
                if round_button(ui, ph::CHAT_CIRCLE_TEXT,
                    if unread > 0 { "Відкрити чат · є нові повідомлення" } else { "Відкрити чат" },
                    if unread > 0 { ACCENT } else { RAISED },
                    if unread > 0 { ACCENT_INK } else { TEXT }, true, d) {
                    self.focus_mode = false;
                    self.chat_unread = 0;
                }
            }
        });
    }

    // Chat ──────────────────────────────────────────────────────────────────
    fn chat_panel(&mut self, ui: &mut Ui) {
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

    // Settings ──────────────────────────────────────────────────────────────
    fn settings_nav(&mut self, ui: &mut Ui) {
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

    fn settings_page_ui(&mut self, ui: &mut Ui) {
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

    fn group(ui: &mut Ui, title: &str) {
        ui.add_space(22.0);
        ui.label(strong(title, 15.0, TEXT));
        ui.add_space(2.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(rect, Rounding::ZERO, LINE);
        ui.add_space(8.0);
    }
    fn field(ui: &mut Ui, label: &str) {
        ui.add_space(6.0);
        ui.label(text(label, 13.0, TEXT2));
    }
    fn hint(ui: &mut Ui, note: impl Into<String>) {
        ui.add(egui::Label::new(text(note, 12.5, TEXT3)).wrap());
    }

    fn settings_audio(&mut self, ui: &mut Ui) {
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

    fn settings_video(&mut self, ui: &mut Ui) {
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
        paint_tile(ui, rect, &view, texture);
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

    fn settings_profile(&mut self, ui: &mut Ui) {
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

    fn settings_appearance(&mut self, ui: &mut Ui) {
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

    fn settings_about(&mut self, ui: &mut Ui) {
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
