//! Renders the SVG icons shared with the macOS app.

use std::path::Path;

use resvg::{tiny_skia, usvg};

pub const APP_SVG: &str = include_str!("../../apple/Assets/AppIcon.svg");
/// Paperclip outline (Solar by 480 Design, CC BY 4.0) in `currentColor`.
const TRAY_SVG: &str = include_str!("../../apple/Assets/tray.svg");

/// Straight (not premultiplied) RGBA pixels, `size` × `size`.
pub fn render(svg: &str, size: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    #[allow(clippy::cast_precision_loss)]
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let color = pixel.demultiply();
                [color.red(), color.green(), color.blue(), color.alpha()]
            })
            .collect(),
    )
}

/// The paperclip in black for a light taskbar, white for a dark one.
pub fn tray(light_taskbar: bool) -> Option<tray_icon::Icon> {
    let color = if light_taskbar { "#000000" } else { "#ffffff" };
    let size = 32;
    let rgba = render(&TRAY_SVG.replace("currentColor", color), size)?;
    tray_icon::Icon::from_rgba(rgba, size, size).ok()
}

pub fn window() -> Option<tao::window::Icon> {
    let size = 64;
    tao::window::Icon::from_rgba(render(APP_SVG, size)?, size, size).ok()
}

/// A PNG of the app icon for toasts, which need a file.
pub fn save_png(path: &Path) -> bool {
    let size = 256;
    render(APP_SVG, size).is_some_and(|rgba| {
        image::save_buffer(path, &rgba, size, size, image::ExtendedColorType::Rgba8).is_ok()
    })
}
