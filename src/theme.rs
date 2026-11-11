//! Port of Components/AppTheme.qml — every color token, same values.

use egui::Color32;

pub const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// `#AARRGGBB` (Qt style) to a Color32.
pub fn argb(v: u32) -> Color32 {
    Color32::from_rgba_unmultiplied((v >> 16) as u8, (v >> 8) as u8, v as u8, (v >> 24) as u8)
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, al] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (al as f32 * a.clamp(0.0, 1.0)).round() as u8)
}

/// Qt.darker(color, factor) — HSV value divided by factor.
pub fn darker(c: Color32, factor: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let v = max;
    let d = max - min;
    let s = if max > 0.0 { d / max } else { 0.0 };
    let h = if d == 0.0 {
        0.0
    } else if max == rf {
        60.0 * (((gf - bf) / d).rem_euclid(6.0))
    } else if max == gf {
        60.0 * ((bf - rf) / d + 2.0)
    } else {
        60.0 * ((rf - gf) / d + 4.0)
    };
    let v2 = v / factor;
    let c2 = v2 * s;
    let x = c2 * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v2 - c2;
    let (r1, g1, b1) = match (h / 60.0) as i32 {
        0 => (c2, x, 0.0),
        1 => (x, c2, 0.0),
        2 => (0.0, c2, x),
        3 => (0.0, x, c2),
        4 => (x, 0.0, c2),
        _ => (c2, 0.0, x),
    };
    Color32::from_rgba_unmultiplied(
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
        a,
    )
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let a = a.to_srgba_unmultiplied();
    let b = b.to_srgba_unmultiplied();
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a[0], b[0]), l(a[1], b[1]), l(a[2], b[2]), l(a[3], b[3]))
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub is_dark: bool,
    pub background: Color32,
    pub surface: Color32,
    pub navigation_active_bg: Color32,
    pub hover_bg: Color32,
    pub sidebar_bg: Color32,
    pub library_bg: Color32,
    pub editor_bg: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub primary_dark: Color32,
    pub primary_light: Color32,
    pub danger: Color32,
    pub danger_hover: Color32,
    pub danger_light: Color32,
    /// Tooltip / popup border (#3f3f46 / #e2e8f0)
    pub tip_border: Color32,
}

impl Theme {
    pub fn new(is_dark: bool) -> Self {
        let d = is_dark;
        let pick = |dark: u32, light: u32| if d { hex(dark) } else { hex(light) };
        Self {
            is_dark,
            background: pick(0x111318, 0xfefefe),
            surface: pick(0x24282e, 0xf8f9fa),
            navigation_active_bg: pick(0x1a1d23, 0xf1f5f9),
            hover_bg: pick(0x2a2d35, 0xf1f5f9),
            sidebar_bg: if d { hex(0x111318) } else { Color32::TRANSPARENT },
            library_bg: pick(0x1a1d23, 0xf1f5f9),
            editor_bg: pick(0x1a1d23, 0xffffff),
            text_primary: pick(0xf1f5f9, 0x0f172a),
            text_secondary: pick(0x94a3b8, 0x64748b),
            primary: hex(0x7c3aed),
            primary_hover: hex(0x8b5cf6),
            primary_dark: hex(0x6d28d9),
            primary_light: pick(0x2d1b4e, 0xede9fe),
            danger: hex(0xef4444),
            danger_hover: hex(0xdc2626),
            danger_light: pick(0x451a1e, 0xfef2f2),
            tip_border: pick(0x3f3f46, 0xe2e8f0),
        }
    }
}

pub const WHITE: Color32 = Color32::WHITE;
pub const SUCCESS: Color32 = hex(0x10b981);
pub const GREEN: Color32 = hex(0x22c55e);
pub const RED: Color32 = hex(0xef4444);
pub const SKY: Color32 = hex(0x38bdf8);
