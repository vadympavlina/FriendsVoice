//! Palette, fonts, egui visuals and small text helpers.
use super::*;

// ── Palette ────────────────────────────────────────────────────────────────
pub(super) const RAIL: Color32 = Color32::from_rgb(17, 18, 20);
pub(super) const USERBAR: Color32 = Color32::from_rgb(13, 14, 16);
pub(super) const STAGE: Color32 = Color32::from_rgb(22, 23, 26);
pub(super) const CHAT: Color32 = Color32::from_rgb(28, 29, 33);
pub(super) const TILE: Color32 = Color32::from_rgb(35, 36, 41);
pub(super) const RAISED: Color32 = Color32::from_rgb(42, 43, 49);
pub(super) const HOVER: Color32 = Color32::from_rgb(52, 53, 60);
pub(super) const LINE: Color32 = Color32::from_rgb(40, 41, 46);
pub(super) const TEXT: Color32 = Color32::from_rgb(236, 235, 231);
pub(super) const TEXT2: Color32 = Color32::from_rgb(170, 171, 178);
pub(super) const TEXT3: Color32 = Color32::from_rgb(128, 129, 138);
pub(super) const ACCENT: Color32 = Color32::from_rgb(158, 128, 252);
/// Warnings (not the brand accent).
pub(super) const WARN: Color32 = Color32::from_rgb(240, 177, 74);
pub(super) const ACCENT_INK: Color32 = Color32::from_rgb(24, 14, 52);
pub(super) const GREEN: Color32 = Color32::from_rgb(79, 209, 139);
pub(super) const RED: Color32 = Color32::from_rgb(226, 84, 92);

/// Muted identity colours. A friend keeps the same colour in the sidebar,
/// the call tiles and the chat because it is derived from their name.
pub(super) const IDENTITY: [Color32; 5] = [
    Color32::from_rgb(111, 143, 216),
    Color32::from_rgb(212, 121, 154),
    Color32::from_rgb(84, 176, 174),
    Color32::from_rgb(155, 136, 220),
    Color32::from_rgb(196, 165, 116),
];

pub(super) fn name_color(name: &str) -> Color32 {
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
        include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf")));
    fonts.font_data.insert("plex-semibold".into(), egui::FontData::from_static(
        include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf")));
    fonts.font_data.insert("plex-mono".into(), egui::FontData::from_static(
        include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf")));
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
pub(super) fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub(super) fn text(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).size(size).color(color)
}
pub(super) fn strong(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(semibold(size)).color(color)
}
pub(super) fn mono(s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(FontId::monospace(size)).color(color)
}
pub(super) fn truncated(ui: &Ui, s: &str, font: FontId, color: Color32, max_width: f32) -> Arc<egui::Galley> {
    let mut job = LayoutJob::simple_singleline(s.to_string(), font, color);
    job.wrap.max_width = max_width.max(8.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    ui.fonts(|fonts| fonts.layout_job(job))
}
pub(super) fn clock(seconds: u64) -> String {
    if seconds >= 3600 {
        format!("{}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

// ── Small widgets ──────────────────────────────────────────────────────────
