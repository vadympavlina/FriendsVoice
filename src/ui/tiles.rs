//! Video tiles: grid and spotlight layout, tile painting, zoom helpers.
use super::*;

// ── Call grid ──────────────────────────────────────────────────────────────
pub(super) const TILE_ASPECT: f32 = 16.0 / 9.0;

/// Pick the column count that gives the biggest 16:9 tiles inside `w × h`.
/// Returns `(columns, tile_width, tile_height)`.
pub(super) fn best_grid(count: usize, w: f32, h: f32, gap: f32) -> (usize, f32, f32) {
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
pub(super) struct TileView<'a> {
    pub(super) name: &'a str,
    pub(super) note: Option<&'a str>,
    pub(super) speaking: bool,
    pub(super) badge: Option<&'a str>,
    /// Fit the whole picture inside the tile (screens) instead of filling it (cameras).
    pub(super) contain: bool,
    /// Small strip tile: smaller text, no avatar glow.
    pub(super) small: bool,
    pub(super) salt: (u64, StreamKind),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TileAction {
    /// Fill the whole window with this stream.
    Expand,
    /// Fill the whole monitor.
    Fullscreen,
}

pub(super) fn paint_tile(ui: &mut Ui, rect: Rect, view: &TileView<'_>, video: Option<&egui::TextureHandle>) -> (Response, Option<TileAction>) {
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
        let center = Pos2::new(rect.left() + 22.0, rect.top() + 22.0);
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

    // Hover buttons: only for tiles that actually show a picture.
    let mut action = None;
    if video.is_some() && (response.hovered() || ui.rect_contains_pointer(rect)) {
        let side = if view.small { 22.0 } else { 32.0 };
        let first = Rect::from_min_size(Pos2::new(rect.right() - side * 2.0 - 14.0, rect.top() + 8.0), Vec2::splat(side));
        let second = first.translate(Vec2::new(side + 4.0, 0.0));
        for (area, glyph, tip, wanted) in [
            (first, ph::ARROWS_OUT, tr("Fill the window · double-click"), TileAction::Expand),
            (second, ph::CORNERS_OUT, tr("Full screen · F11"), TileAction::Fullscreen),
        ] {
            let button = ui.interact(area, egui::Id::new(("tile-action", view.salt.0, view.salt.1.index(), wanted as u8)), Sense::click());
            let fill = if button.hovered() { Color32::from_black_alpha(215) } else { Color32::from_black_alpha(150) };
            p.rect_filled(area, Rounding::same(8.0), fill);
            p.text(area.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(side * 0.6), TEXT);
            if button.clicked() { action = Some(wanted); }
            button.on_hover_text(tip);
        }
    }
    (response, action)
}

/// Zooms about a point of the view: the picture point under `pointer` stays under it.
/// `base` is the picture size at 100%, `pointer` is relative to the view centre.
/// Returns the new `(zoom, pan)`.
pub(super) fn zoom_at(base: Vec2, zoom: f32, pan: Vec2, pointer: Vec2, scroll: f32) -> (f32, Vec2) {
    let new = (zoom * (scroll * 0.0025).exp()).clamp(1.0, 6.0);
    let relative = (pointer - pan) / (base * zoom);
    (new, pointer - relative * (base * new))
}

/// Keeps the picture covering the view: you can never drag it away.
pub(super) fn clamp_pan(pan: Vec2, base: Vec2, zoom: f32, view: Vec2) -> Vec2 {
    if zoom <= 1.0 { return Vec2::ZERO; }
    let limit = ((base * zoom - view) * 0.5).max(Vec2::ZERO);
    Vec2::new(pan.x.clamp(-limit.x, limit.x), pan.y.clamp(-limit.y, limit.y))
}

/// Layout of a spotlight (one big tile) plus a strip of the others.
/// Returns `(big tile area, strip tile rects)`.
pub(super) fn spotlight_layout(area: Rect, others: usize, gap: f32) -> (Rect, Vec<Rect>) {
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
    fn zoom_keeps_the_point_under_the_cursor_and_stays_in_bounds() {
        let base = Vec2::new(1280.0, 720.0);
        let view = Vec2::new(1400.0, 800.0);
        let pointer = Vec2::new(300.0, -120.0);
        let (zoom, pan) = zoom_at(base, 1.0, Vec2::ZERO, pointer, 300.0);
        assert!(zoom > 1.0 && zoom <= 6.0);
        // The picture point that was under the pointer is still under it.
        let before = (pointer - Vec2::ZERO) / (base * 1.0);
        let after = (pointer - pan) / (base * zoom);
        assert!((before - after).length() < 1e-4, "{before:?} vs {after:?}");
        let (zoom, _) = zoom_at(base, 5.9, Vec2::ZERO, Vec2::ZERO, 5000.0);
        assert_eq!(zoom, 6.0, "zoom is capped");
        let (zoom, _) = zoom_at(base, 1.0, Vec2::ZERO, Vec2::ZERO, -5000.0);
        assert_eq!(zoom, 1.0, "cannot zoom out below the whole picture");
        // Dragging far away is stopped at the picture's edge; at 100% there is no pan.
        let clamped = clamp_pan(Vec2::new(9999.0, -9999.0), base, 3.0, view);
        assert_eq!(clamped, Vec2::new((base.x * 3.0 - view.x) / 2.0, -(base.y * 3.0 - view.y) / 2.0));
        assert_eq!(clamp_pan(Vec2::new(40.0, 40.0), base, 1.0, view), Vec2::ZERO);
        // A picture that is smaller than the view in one direction cannot be moved in it.
        assert_eq!(clamp_pan(Vec2::new(50.0, 0.0), Vec2::new(100.0, 700.0), 1.5, Vec2::new(1000.0, 600.0)).x, 0.0);
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
