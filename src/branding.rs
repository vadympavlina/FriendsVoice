//! Window / taskbar icon and the in-app logo, decoded from the embedded mascot PNGs.
use eframe::egui;

const WINDOW_ICON: &[u8] = include_bytes!("../assets/icon-256.png");
const LOGO_SMALL: &[u8] = include_bytes!("../assets/icon-48.png");
const LOGO_LARGE: &[u8] = include_bytes!("../assets/icon-96.png");

fn decode(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some((image.into_raw(), width, height))
}

pub fn icon() -> egui::IconData {
    match decode(WINDOW_ICON) {
        Some((rgba, width, height)) => egui::IconData { rgba, width, height },
        None => egui::IconData { rgba: vec![0; 4], width: 1, height: 1 },
    }
}

/// Uploads the logo textures once; the UI looks them up by name.
pub fn install_logo_textures(ctx: &egui::Context) {
    for (name, bytes) in [("logo-small", LOGO_SMALL), ("logo-large", LOGO_LARGE)] {
        if let Some((rgba, width, height)) = decode(bytes) {
            let image = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
            let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
            ctx.data_mut(|data| data.insert_temp(egui::Id::new(name), texture));
        }
    }
}

pub fn logo_texture(ctx: &egui::Context, large: bool) -> Option<egui::TextureHandle> {
    let name = if large { "logo-large" } else { "logo-small" };
    ctx.data(|data| data.get_temp::<egui::TextureHandle>(egui::Id::new(name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_icon_is_square_with_transparent_corners_and_opaque_centre() {
        let image = icon();
        assert_eq!((image.width, image.height), (256, 256));
        assert_eq!(image.rgba.len(), 256 * 256 * 4);
        assert_eq!(image.rgba[3], 0, "corner must be transparent");
        let centre = ((128 * 256 + 128) * 4 + 3) as usize;
        assert!(image.rgba[centre] >= 240, "mascot body must be (nearly) opaque");
    }
    #[test]
    fn logo_pngs_decode() {
        assert!(decode(LOGO_SMALL).is_some());
        assert!(decode(LOGO_LARGE).is_some());
    }
}
