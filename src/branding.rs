//! Programmatic Windows taskbar/window icon: Aurora waveform mark.
//! Rendered into RGBA pixels, no external images, fonts or installers required.
pub fn icon() -> eframe::egui::IconData {
    const SIDE: u32 = 96;
    let mut rgba = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    let bars: [(f32, f32); 5] = [
        (23.0, 19.0),
        (35.0, 37.0),
        (47.5, 49.0),
        (60.0, 31.0),
        (72.5, 17.0),
    ];
    for y in 0..SIDE {
        for x in 0..SIDE {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = (fx - 48.0).abs() - 24.0;
            let dy = (fy - 48.0).abs() - 24.0;
            let ax = dx.max(0.0);
            let ay = dy.max(0.0);
            let edge = (ax * ax + ay * ay).sqrt() + dx.max(dy).min(0.0) - 21.0;
            let alpha = (0.5 - edge).clamp(0.0, 1.0);
            if alpha <= 0.0 {
                rgba.extend_from_slice(&[0,0,0,0]);
                continue;
            }
            let t = ((fx + fy) / 192.0).clamp(0.0, 1.0);
            let mut rgb = [
                91.0 + 14.0*t,
                67.0 + 37.0*t,
                196.0 + 37.0*t,
            ];
            let gl = ((fx-66.0).powi(2)+(fy-32.0).powi(2)).sqrt();
            let glow = (1.0 - gl/74.0).clamp(0.0,1.0) * 0.23;
            rgb[1] += glow * 81.0;
            rgb[2] += glow * 12.0;
            for (index,(bx,height)) in bars.iter().enumerate() {
                let half = *height * 0.5;
                let d = (fx-*bx).abs() - 4.0;
                let vertical = (fy-48.0).abs()-(half-4.0);
                let distance = (d.max(0.0).powi(2)+vertical.max(0.0).powi(2)).sqrt()
                    + d.max(vertical).min(0.0)-4.0;
                let coverage = (0.8 - distance).clamp(0.0, 1.0);
                let tone = if index == 2 {
                    [244.0, 253.0, 255.0]
                } else {
                    [196.0, 242.0, 255.0]
                };
                for channel in 0..3 {
                    rgb[channel] = rgb[channel] * (1.0-coverage) + tone[channel]*coverage;
                }
            }
            rgba.extend_from_slice(&[
                rgb[0].clamp(0.0,255.0) as u8,
                rgb[1].clamp(0.0,255.0) as u8,
                rgb[2].clamp(0.0,255.0) as u8,
                (alpha*255.0) as u8,
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
        assert_eq!(image.rgba.len(), 96 * 96 * 4);
        assert_eq!(image.rgba[3],0);
        assert_eq!(image.rgba[((48*96+48)*4+3) as usize],255);
    }
}
