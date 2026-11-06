//! Window / taskbar icon, rendered into RGBA pixels (no external image files).
//!
//! The mark is two overlapping circles: two friends in one room. The overlap
//! is the amber lens that also appears next to the wordmark in the sidebar.
pub fn icon() -> eframe::egui::IconData {
    const SIDE: u32 = 128;
    let tile = [24.0f32, 25.0, 29.0];
    let paper = [236.0f32, 235.0, 231.0];
    let amber = [240.0f32, 177.0, 74.0];
    let (left, right) = ((46.0f32, 64.0f32), (82.0f32, 64.0f32));
    let radius = 27.0f32;
    let ring = 4.5f32;

    // Signed distance helpers return coverage in 0..=1 with ~1px anti-aliasing.
    let coverage = |d: f32| (0.5 - d).clamp(0.0, 1.0);
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);

            // Rounded-square tile.
            let dx = (fx - 64.0).abs() - (58.0 - 26.0);
            let dy = (fy - 64.0).abs() - (58.0 - 26.0);
            let tile_d = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt()
                + dx.max(dy).min(0.0)
                - 26.0;
            let alpha = coverage(tile_d);
            if alpha <= 0.0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            let dist_l = ((fx - left.0).powi(2) + (fy - left.1).powi(2)).sqrt() - radius;
            let dist_r = ((fx - right.0).powi(2) + (fy - right.1).powi(2)).sqrt() - radius;
            let mut rgb = tile;
            let mix = |rgb: &mut [f32; 3], tone: [f32; 3], k: f32| {
                for c in 0..3 {
                    rgb[c] = rgb[c] * (1.0 - k) + tone[c] * k;
                }
            };
            // Lens: inside both circles.
            mix(&mut rgb, amber, coverage(dist_l.max(dist_r)));
            // Outlines of both circles.
            mix(&mut rgb, paper, coverage(dist_l.abs() - ring * 0.5));
            mix(&mut rgb, paper, coverage(dist_r.abs() - ring * 0.5));

            rgba.extend_from_slice(&[
                rgb[0].clamp(0.0, 255.0) as u8,
                rgb[1].clamp(0.0, 255.0) as u8,
                rgb[2].clamp(0.0, 255.0) as u8,
                (alpha * 255.0) as u8,
            ]);
        }
    }
    eframe::egui::IconData { rgba, width: SIDE, height: SIDE }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icon_has_alpha_and_valid_dimensions() {
        let image = icon();
        assert_eq!(image.rgba.len(), 128 * 128 * 4);
        assert_eq!(image.rgba[3], 0, "corner must be transparent");
        assert_eq!(image.rgba[((64 * 128 + 64) * 4 + 3) as usize], 255);
    }
}
