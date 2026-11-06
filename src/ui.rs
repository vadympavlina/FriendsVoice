//! Native egui interface, laid out like a voice chat client:
//! channel rail on the left, call stage in the middle, text chat on the right.
//!
//! Design rules: flat surfaces separated by tone (not borders or cards), one
//! amber accent used only for primary actions, green only for "speaking /
//! online", red only for destructive or muted states, sentence-case text.
use super::*;
use eframe::egui::{
    self, text::LayoutJob, Align2, FontFamily, FontId, Margin, Pos2, Rect, Response, RichText,
    Rounding, Sense, Stroke, Ui, Vec2,
};
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
const AMBER: Color32 = Color32::from_rgb(240, 177, 74);
const AMBER_INK: Color32 = Color32::from_rgb(34, 23, 5);
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
    v.hyperlink_color = AMBER;
    v.selection.bg_fill = Color32::from_rgb(112, 84, 34);
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
        ui.painter().rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.5_f32, AMBER));
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
        ui.painter().circle_stroke(rect.center(), d * 0.5 + 2.0, Stroke::new(1.5_f32, AMBER));
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
    pill_button(ui, label, AMBER, AMBER_INK, Vec2::new(0.0, 34.0), enabled)
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
        ui.painter().rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.5_f32, AMBER));
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
        ui.painter().rect_filled(badge_rect, Rounding::same(9.0), AMBER);
        ui.painter().text(badge_rect.center(), Align2::CENTER_CENTER, label,
            semibold(11.5), AMBER_INK);
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

fn level_bar(ui: &mut Ui, level: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(4.0), RAISED);
    let fill = Rect::from_min_size(rect.min, Vec2::new(rect.width() * level.clamp(0.0, 1.0), rect.height()));
    ui.painter().rect_filled(fill, Rounding::same(4.0), if level > 0.85 { AMBER } else { GREEN });
}

fn brand_mark(ui: &mut Ui, side: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    let r = side * 0.3;
    let offset = side * 0.17;
    let (left, right) = (Pos2::new(rect.center().x - offset, rect.center().y),
                         Pos2::new(rect.center().x + offset, rect.center().y));
    // Lens = intersection of the two circles.
    let half = (offset / r).clamp(0.0, 1.0).acos();
    let mut points = Vec::new();
    for i in 0..=24 {
        let a = -half + 2.0 * half * i as f32 / 24.0;
        points.push(Pos2::new(left.x + r * a.cos(), left.y + r * a.sin()));
    }
    for i in (0..=24).rev() {
        let a = -half + 2.0 * half * i as f32 / 24.0;
        points.push(Pos2::new(right.x - r * a.cos(), right.y + r * a.sin()));
    }
    ui.painter().add(egui::Shape::convex_polygon(points, AMBER, Stroke::NONE));
    let ring = Stroke::new(side * 0.075, TEXT);
    ui.painter().circle_stroke(left, r, ring);
    ui.painter().circle_stroke(right, r, ring);
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

fn paint_tile(
    ui: &mut Ui, rect: Rect, name: &str, note: Option<&str>,
    speaking: bool, video: Option<&egui::TextureHandle>,
) {
    let p = ui.painter_at(rect);
    let rounding = Rounding::same(12.0);
    p.rect_filled(rect, rounding, TILE);
    if let Some(frame) = video {
        // Cover-fit the 16:9 camera frame instead of stretching it.
        let aspect = (rect.width() / rect.height()).max(0.01);
        let uv = if aspect < TILE_ASPECT {
            let visible = (aspect / TILE_ASPECT).clamp(0.01, 1.0);
            Rect::from_min_max(Pos2::new((1.0 - visible) * 0.5, 0.0), Pos2::new((1.0 + visible) * 0.5, 1.0))
        } else {
            let visible = (TILE_ASPECT / aspect).clamp(0.01, 1.0);
            Rect::from_min_max(Pos2::new(0.0, (1.0 - visible) * 0.5), Pos2::new(1.0, (1.0 + visible) * 0.5))
        };
        let mut shape = egui::epaint::RectShape::filled(rect, rounding, Color32::WHITE);
        shape.fill_texture_id = frame.id();
        shape.uv = uv;
        p.add(shape);
    } else {
        let diameter = (rect.height() * 0.36).clamp(30.0, 120.0);
        paint_avatar(&p, rect.center() - Vec2::new(0.0, 6.0), diameter, name);
    }

    // Name label.
    let label = match note {
        Some(note) => format!("{name}  ·  {note}"),
        None => name.to_string(),
    };
    let galley = truncated(ui, &label, FontId::proportional(13.0), TEXT, rect.width() - 36.0);
    let size = galley.size() + Vec2::new(16.0, 8.0);
    let pill = Rect::from_min_size(
        Pos2::new(rect.left() + 10.0, rect.bottom() - 10.0 - size.y), size);
    p.rect_filled(pill, Rounding::same(6.0), Color32::from_black_alpha(165));
    p.galley(pill.min + Vec2::new(8.0, 4.0), galley, TEXT);

    let glow = ui.ctx().animate_bool_with_time(
        egui::Id::new(("tile-ring", name)), speaking, 0.12);
    if glow > 0.01 {
        p.rect_stroke(rect.shrink(1.25), rounding, Stroke::new(2.5_f32, GREEN.linear_multiply(glow)));
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
                            self.video_textures.contains_key(&peer.id).then_some(ph::VIDEO_CAMERA));
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
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let connected = self.mesh.is_some();
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 3.5, if connected { GREEN } else { RED });
            ui.label(text(if connected { "P2P працює, сервера немає" } else { "Немає з'єднання" },
                13.0, TEXT2));
        });
        if self.mesh.is_none() {
            if let Some(error) = &self.network_error {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(text(error.clone(), 12.0, RED)).wrap());
                });
            }
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                if primary_button(ui, "Спробувати знову", true) {
                    self.retry_network();
                }
            });
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            match ip {
                Some(ip) => {
                    ui.label(mono(ip.to_string(), 13.0, TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);
                        let copied = self.ip_copied_at.is_some_and(|t| t.elapsed().as_secs() < 2);
                        if icon_button(ui, if copied { ph::CHECK } else { ph::COPY },
                            if copied { "Скопійовано" } else { "Скопіювати свою адресу" }, false, 28.0) {
                            ui.output_mut(|o| o.copied_text = ip.to_string());
                            self.ip_copied_at = Some(Instant::now());
                        }
                    });
                }
                None => {
                    ui.add(egui::Label::new(text(
                        "Адресу 26.x.x.x не знайдено. Запусти Radmin VPN.", 12.5, TEXT3)).wrap());
                }
            }
        });
        ui.add_space(4.0);
        if nav_row(ui, ph::USER_PLUS, "Додати друга за IP", self.add_friend_open, true, 0).clicked() {
            self.add_friend_open = !self.add_friend_open;
        }
        if self.add_friend_open {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width() - 8.0);
                    let entry = ui.add(egui::TextEdit::singleline(&mut self.friend_ip)
                        .hint_text("26.12.34.56")
                        .font(FontId::monospace(13.0))
                        .margin(Margin::symmetric(10.0, 8.0))
                        .desired_width(f32::INFINITY));
                    let submit = entry.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
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
                let resp = resp.on_hover_text(if deaf { "Увімкнути звук" } else { "Вимкнути звук" });
                if voice && resp.hovered() {
                    ui.painter().rect_filled(rect, Rounding::same(8.0), HOVER);
                }
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, ph::HEADPHONES,
                    FontId::proportional(18.0), if voice { tint(deaf) } else { TEXT3 });
                if voice && resp.clicked() { self.deafened.store(!deaf, Ordering::Relaxed); }
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0),
                    if voice { Sense::click() } else { Sense::hover() });
                let resp = resp.on_hover_text(if mic { "Вимкнути мікрофон · Ctrl+M" }
                    else { "Увімкнути мікрофон · Ctrl+M" });
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

        egui::TopBottomPanel::bottom("callbar").exact_height(92.0)
            .frame(egui::Frame::none().fill(STAGE))
            .show_inside(ui, |ui| self.call_bar(ui));

        let area = ui.available_rect_before_wrap().shrink2(Vec2::new(24.0, 14.0));
        ui.allocate_rect(ui.available_rect_before_wrap(), Sense::hover());
        let alone = self.peers.is_empty();
        let hint_height = if alone { 44.0 } else { 0.0 };
        let grid_area = Rect::from_min_max(area.min, area.max - Vec2::new(0.0, hint_height));
        let in_voice = self.voice.is_some();

        // Everyone in the room is on stage; friends who have not joined voice
        // are shown only in the sidebar.
        struct Entry { name: String, note: Option<&'static str>, speaking: bool, id: Option<u64> }
        let mut entries = vec![Entry {
            name: self.settings.nickname.clone(),
            note: Some("ти"),
            speaking: self.voice.as_ref().is_some_and(|v| v.transmitting.load(Ordering::Relaxed)),
            id: self.mesh.as_ref().map(|m| m.id),
        }];
        for peer in self.peers.iter().filter(|p| p.voice || !in_voice) {
            entries.push(Entry {
                name: peer.name.clone(),
                note: (peer.protocol != PROTOCOL_VERSION).then_some("потрібне оновлення"),
                speaking: self.voice.as_ref().is_some_and(|v| v.is_speaking(peer.id)),
                id: Some(peer.id),
            });
        }
        let gap = 12.0;
        let (columns, tile_w, tile_h) = best_grid(entries.len(), grid_area.width(), grid_area.height(), gap);
        let rows = entries.len().div_ceil(columns);
        let total_h = rows as f32 * tile_h + gap * (rows - 1) as f32;
        let top = grid_area.center().y - total_h * 0.5;
        for (index, entry) in entries.iter().enumerate() {
            let (row, column) = (index / columns, index % columns);
            let in_row = (entries.len() - row * columns).min(columns);
            let row_w = in_row as f32 * tile_w + gap * (in_row - 1) as f32;
            let left = grid_area.center().x - row_w * 0.5;
            let rect = Rect::from_min_size(
                Pos2::new(left + column as f32 * (tile_w + gap), top + row as f32 * (tile_h + gap)),
                Vec2::new(tile_w, tile_h));
            let texture = entry.id.and_then(|id| self.video_textures.get(&id));
            paint_tile(ui, rect, &entry.name, entry.note, entry.speaking, texture);
        }
        if alone {
            let center = Pos2::new(area.center().x, top + total_h + 26.0);
            ui.painter().text(center, Align2::CENTER_CENTER,
                "Тут поки тільки ти. Додай друга за IP у лівій панелі.",
                FontId::proportional(13.5), TEXT3);
        }
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
        let leave_width = if room >= 470.0 { 124.0 } else { 96.0 };
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
                    if camera_starting { AMBER } else if camera { TEXT } else { RAISED },
                    if camera_starting { AMBER_INK } else if camera { STAGE } else { TEXT },
                    self.mesh.is_some(), d) {
                    self.toggle_camera();
                }
                round_button(ui, ph::MONITOR_ARROW_UP, "Демонстрація екрана поки недоступна",
                    RAISED, TEXT, false, d);
                ui.add_space(6.0);
                if pill_button(ui, "Вийти", RED, Color32::WHITE, Vec2::new(leave_width, d), true) {
                    self.toggle_voice();
                }
            } else if pill_button(ui, "Приєднатися до голосу", AMBER, AMBER_INK,
                Vec2::new(join_width, d), self.mesh.is_some()) {
                self.toggle_voice();
            }
            if chat_button {
                let unread = self.chat_unread;
                if round_button(ui, ph::CHAT_CIRCLE_TEXT,
                    if unread > 0 { "Відкрити чат · є нові повідомлення" } else { "Відкрити чат" },
                    if unread > 0 { AMBER } else { RAISED },
                    if unread > 0 { AMBER_INK } else { TEXT }, true, d) {
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
                        if ready { if resp.hovered() { AMBER.lerp_to_gamma(Color32::WHITE, 0.12) } else { AMBER } } else { RAISED });
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, ph::PAPER_PLANE_RIGHT,
                        FontId::proportional(19.0), if ready { AMBER_INK } else { TEXT3 });
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
            (ph::MICROPHONE, "Звук"),
            (ph::VIDEO_CAMERA, "Відео"),
            (ph::GEAR, "Загальні"),
            (ph::WARNING_CIRCLE, "Про програму"),
        ].into_iter().enumerate() {
            if nav_row(ui, glyph, label, self.settings_page == index, true, 0).clicked() {
                self.settings_page = index;
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(4.0);
            if nav_row(ui, ph::X, "Закрити · Esc", false, true, 0).clicked() {
                self.settings_open = false;
            }
        });
    }

    fn settings_page_ui(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().id_salt("settings_scroll").auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_max_width(620.0);
                let title = ["Звук", "Відео", "Загальні", "Про програму"][self.settings_page.min(3)];
                ui.label(strong(title, 22.0, TEXT));
                ui.add_space(6.0);
                match self.settings_page {
                    0 => self.settings_audio(ui),
                    1 => self.settings_video(ui),
                    2 => self.settings_general(ui),
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
        ui.horizontal(|ui| {
            if quiet_button(ui, "Оновити список") {
                self.input_devices = voice::list_input_devices();
                self.output_devices = voice::list_output_devices();
            }
            if primary_button(ui, "Зберегти", true) {
                self.settings.volume = f32::from_bits(self.volume.load(Ordering::Relaxed));
                save_settings(&self.settings);
                self.notice("Налаштування звуку збережено.");
            }
        });
        Self::hint(ui, "Після зміни пристрою вийди з голосового каналу й зайди знову.");

        Self::group(ui, "Гучність і мікрофон");
        Self::field(ui, "Гучність відтворення");
        let mut volume = f32::from_bits(self.volume.load(Ordering::Relaxed)).clamp(0.0, 2.0);
        if ui.add(egui::Slider::new(&mut volume, 0.0..=2.0)
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed() {
            self.volume.store(volume.to_bits(), Ordering::Relaxed);
        }
        Self::field(ui, "Рівень мікрофона");
        let level = self.voice.as_ref()
            .map(|v| f32::from_bits(v.microphone_level.load(Ordering::Relaxed)))
            .unwrap_or(0.0).clamp(0.0, 1.0);
        level_bar(ui, (level * 7.0).min(1.0), ui.available_width().min(420.0));
        if self.voice.is_none() {
            Self::hint(ui, "Зайди в голосовий канал, щоб побачити рівень.");
        }

        Self::group(ui, "Коли передавати голос");
        let mut ptt = self.push_to_talk.load(Ordering::Relaxed);
        let hotkey_label = match self.ptt_key.load(Ordering::Relaxed) {
            0x05 => "Mouse4", 0x06 => "Mouse5", 0x75 => "F6", 0x76 => "F7",
            0x77 => "F8", 0x79 => "F10", 0x7A => "F11", 0x7B => "F12", _ => "F8",
        };
        if ui.checkbox(&mut ptt, "Говорити лише при натиснутій клавіші (Push-to-Talk)").changed() {
            self.push_to_talk.store(ptt, Ordering::Relaxed);
            self.settings.push_to_talk = ptt;
            save_settings(&self.settings);
        }
        Self::field(ui, "Клавіша");
        let mut key = self.ptt_key.load(Ordering::Relaxed);
        egui::ComboBox::from_id_salt("fv_ptt_key").selected_text(hotkey_label).show_ui(ui, |ui| {
            for (label, vk) in [("F6", 0x75), ("F7", 0x76), ("F8", 0x77), ("F10", 0x79),
                ("F11", 0x7A), ("F12", 0x7B), ("Mouse4 (бічна)", 0x05), ("Mouse5 (бічна)", 0x06)] {
                ui.selectable_value(&mut key, vk, label);
            }
        });
        if key != self.ptt_key.load(Ordering::Relaxed) {
            self.ptt_key.store(key, Ordering::Relaxed);
            self.settings.ptt_key = key;
            save_settings(&self.settings);
        }
        Self::hint(ui, "F9 вмикає й вимикає мікрофон навіть під час гри. Ctrl+M робить те саме всередині програми.");
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
                ui.label(text(format!("Попереджень UDP: {errors}"), 12.5, AMBER));
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
        let texture = local_id.and_then(|id| self.video_textures.get(&id));
        let note = if starting { Some("запуск камери") } else if !on { Some("камеру вимкнено") } else { None };
        paint_tile(ui, rect, &self.settings.nickname.clone(), note, false, texture);
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
            2 => "Висока · 640×360, 12 к/с",
            _ => "Баланс · 480×270, 10 к/с",
        };
        egui::ComboBox::from_id_salt("fv_video_quality").width(260.0).selected_text(label(original))
            .show_ui(ui, |ui| {
                for q in 0..3u32 { ui.selectable_value(&mut selected, q, label(q)); }
            });
        if selected != original { self.update_video_quality(selected); }
        Self::hint(ui, "Висока якість надсилає втричі більше даних кожному другу. Якщо відео смикається, обери «Економію». Камера підганяється під формат 16:9.");
        if let Some(video) = &self.video {
            ui.add_space(6.0);
            Self::hint(ui, format!("Кадрів: надіслано {}, отримано {}, відновлено після втрат {}",
                video.frames_sent.load(Ordering::Relaxed),
                video.frames_received.load(Ordering::Relaxed),
                video.frames_recovered.load(Ordering::Relaxed)));
        }

        Self::group(ui, "Демонстрація екрана");
        Self::hint(ui, "Ще не реалізована.");
    }

    fn settings_general(&mut self, ui: &mut Ui) {
        Self::group(ui, "Профіль");
        ui.horizontal(|ui| {
            avatar(ui, &self.settings.nickname, 44.0, false);
            ui.vertical(|ui| {
                ui.label(text("Ім'я в кімнаті", 13.0, TEXT2));
                let changed = ui.add(egui::TextEdit::singleline(&mut self.settings.nickname)
                    .margin(Margin::symmetric(10.0, 8.0)).desired_width(260.0)).changed();
                if changed {
                    if self.settings.nickname.chars().count() > 24 {
                        self.settings.nickname = self.settings.nickname.chars().take(24).collect();
                    }
                    if !self.settings.nickname.trim().is_empty() {
                        if let Some(mesh) = &self.mesh { mesh.rename(self.settings.nickname.clone()); }
                    }
                    save_settings(&self.settings);
                }
            });
        });

        Self::group(ui, "Вигляд");
        let mut sidebar = self.sidebar_open;
        if ui.checkbox(&mut sidebar, "Показувати бічну панель").changed() { self.sidebar_open = sidebar; }
        let mut theatre = self.focus_mode;
        if ui.checkbox(&mut theatre, "Театральний режим (без чату)").changed() {
            self.focus_mode = theatre;
            if !theatre { self.chat_unread = 0; }
        }
        ui.add_space(4.0);
        if quiet_button(ui, "Скинути вигляд") {
            self.sidebar_open = true;
            self.focus_mode = false;
            self.chat_search_open = false;
            self.chat_unread = 0;
            self.chat_search.clear();
            self.notice("Вигляд скинуто.");
        }

        Self::group(ui, "Гарячі клавіші");
        for (keys, action) in [
            ("Ctrl + M", "Мікрофон"),
            ("Ctrl + B", "Бічна панель"),
            ("Ctrl + E", "Театральний режим"),
            ("Ctrl + F", "Пошук у чаті"),
            ("Esc", "Закрити пошук, налаштування чи театральний режим"),
            ("F9", "Мікрофон у будь-якому вікні"),
        ] {
            ui.horizontal(|ui| {
                ui.add_sized([96.0, 22.0], egui::Label::new(mono(keys, 12.5, TEXT)));
                ui.label(text(action, 13.0, TEXT2));
            });
        }

        Self::group(ui, "Збережені адреси");
        Self::hint(ui, format!("Адрес друзів на цьому комп'ютері: {}. Вони нікуди не передаються.",
            self.settings.known_ips.len()));
    }

    fn settings_about(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            brand_mark(ui, 48.0);
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
