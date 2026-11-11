//! SVG assets rendered with resvg (tray icons, title bar buttons, app icon,
//! tutorial mouse pointer).

use resvg::{tiny_skia, usvg};

pub const APP_ICON: &str = include_str!("../assets/app-icon.svg");
pub const CURSOR_POINTER: &str = include_str!("../assets/cursor-pointer.svg");
pub const MINIMIZE: &str = include_str!("../assets/minimize_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg");
pub const OPEN_IN_FULL: &str = include_str!("../assets/open_in_full_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg");
pub const CLOSE_FULLSCREEN: &str = include_str!("../assets/close_fullscreen_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg");
pub const CLOSE: &str = include_str!("../assets/close_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg");
pub const TRAY_SHOW: &str = include_str!("../assets/icon-tray-show.svg");
pub const TRAY_PAUSE: &str = include_str!("../assets/icon-tray-pause.svg");
pub const TRAY_RESUME: &str = include_str!("../assets/icon-tray-resume.svg");
pub const TRAY_QUIT: &str = include_str!("../assets/icon-tray-quit.svg");
pub const TRAY_UPDATE: &str = include_str!("../assets/icon-tray-update.svg");

/// Rasterize to a premultiplied RGBA pixmap of exactly `w`×`h`.
pub fn rasterize(svg: &str, w: u32, h: u32) -> Option<tiny_skia::Pixmap> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    let size = tree.size();
    let t = tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, t, &mut pixmap.as_mut());
    Some(pixmap)
}

/// Same SVG with its paint colours replaced (QML `icon.color` tinting).
pub fn recolor(svg: &str, color_hex: &str) -> String {
    svg.replace("#1f1f1f", color_hex).replace("#f1f5f9", color_hex)
}

pub fn to_color_image(svg: &str, w: u32, h: u32) -> egui::ColorImage {
    match rasterize(svg, w, h) {
        Some(p) => egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], p.data()),
        None => egui::ColorImage::filled([w as usize, h as usize], egui::Color32::TRANSPARENT),
    }
}

pub fn icon_data(svg: &str, size: u32) -> egui::IconData {
    let img = match rasterize(svg, size, size) {
        Some(p) => {
            // IconData wants straight (unpremultiplied) RGBA
            let mut out = Vec::with_capacity(p.data().len());
            for px in p.pixels() {
                let c = px.demultiply();
                out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
            }
            out
        }
        None => vec![0; (size * size * 4) as usize],
    };
    egui::IconData { rgba: img, width: size, height: size }
}

/// Texture cache keyed by (name, pixel size).
#[derive(Default)]
pub struct SvgCache {
    map: std::collections::HashMap<(&'static str, u32), egui::TextureHandle>,
}

impl SvgCache {
    pub fn get(&mut self, ctx: &egui::Context, name: &'static str, svg: &str, logical: f32) -> egui::TextureHandle {
        let px = (logical * ctx.pixels_per_point()).round().max(1.0) as u32;
        self.map
            .entry((name, px))
            .or_insert_with(|| {
                let white = recolor(svg, "#ffffff");
                ctx.load_texture(format!("svg-{name}-{px}"), to_color_image(&white, px, px), egui::TextureOptions::LINEAR)
            })
            .clone()
    }

    /// Untinted (original colours) texture.
    pub fn get_raw(&mut self, ctx: &egui::Context, name: &'static str, svg: &str, w: f32, h: f32) -> egui::TextureHandle {
        let pw = (w * ctx.pixels_per_point()).round().max(1.0) as u32;
        let ph = (h * ctx.pixels_per_point()).round().max(1.0) as u32;
        self.map
            .entry((name, pw * 10000 + ph))
            .or_insert_with(|| {
                ctx.load_texture(format!("svgraw-{name}-{pw}x{ph}"), to_color_image(svg, pw, ph), egui::TextureOptions::LINEAR)
            })
            .clone()
    }
}
