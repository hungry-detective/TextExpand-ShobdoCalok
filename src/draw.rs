//! Small drawing toolkit used by every page: rounded rects, text with QML-like
//! alignment / elide / letter-spacing, Material icons and transforms (for the
//! scale animations QML gets for free).

use egui::emath::TSTransform;
use egui::epaint::{CornerRadius, RectShape, Stroke, StrokeKind, TextShape};
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align2, Color32, FontId, Galley, Painter, Pos2, Rect, Shape, Vec2};
use std::sync::Arc;

use crate::fonts;

/// Qt renders text with hinted (pixel-rounded) advances, which makes bold
/// Segoe UI noticeably wider than its outline metrics; mimic that.
pub fn letter_spacing_for(font: &FontId) -> f32 {
    match &font.family {
        egui::FontFamily::Name(n) if n.as_ref() == fonts::UI_BOLD => 0.6,
        _ => 0.0,
    }
}

pub fn job(text: &str, font: FontId, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let sp = letter_spacing_for(&font);
    job.append(text, 0.0, TextFormat { font_id: font, color, extra_letter_spacing: sp, ..Default::default() });
    job
}

pub fn cr(r: f32) -> CornerRadius {
    CornerRadius::same(r.round().clamp(0.0, 255.0) as u8)
}

#[derive(Clone)]
pub struct Canvas {
    pub p: Painter,
    pub xf: TSTransform,
}

impl Canvas {
    pub fn new(p: &Painter) -> Self {
        Self { p: p.clone(), xf: TSTransform::IDENTITY }
    }

    /// Scale everything drawn through the returned canvas about `center`.
    pub fn scaled(&self, center: Pos2, s: f32) -> Self {
        if (s - 1.0).abs() < 1e-4 {
            return self.clone();
        }
        let local = TSTransform::from_translation(center.to_vec2())
            * TSTransform::from_scaling(s)
            * TSTransform::from_translation(-center.to_vec2());
        Self { p: self.p.clone(), xf: self.xf * local }
    }

    pub fn faded(&self, opacity: f32) -> Self {
        let mut p = self.p.clone();
        p.multiply_opacity(opacity.clamp(0.0, 1.0));
        Self { p, xf: self.xf }
    }

    pub fn clipped(&self, r: Rect) -> Self {
        let r = if self.xf == TSTransform::IDENTITY { r } else { self.xf * r };
        Self { p: self.p.with_clip_rect(r.intersect(self.p.clip_rect())), xf: self.xf }
    }

    pub fn add(&self, mut s: Shape) {
        if self.xf != TSTransform::IDENTITY {
            s.transform(self.xf);
        }
        self.p.add(s);
    }

    pub fn ctx(&self) -> &egui::Context {
        self.p.ctx()
    }

    // ── Shapes ───────────────────────────────────────────────────────────────

    pub fn rect(&self, r: Rect, radius: f32, fill: Color32) {
        if fill.a() == 0 {
            return;
        }
        self.add(Shape::Rect(RectShape::filled(r, cr(radius.min(r.height() / 2.0).min(r.width() / 2.0)), fill)));
    }

    /// QML `border` is drawn inside the item's rect.
    pub fn rect_stroke(&self, r: Rect, radius: f32, width: f32, color: Color32) {
        if color.a() == 0 || width <= 0.0 {
            return;
        }
        self.add(Shape::Rect(RectShape::stroke(
            r,
            cr(radius.min(r.height() / 2.0).min(r.width() / 2.0)),
            Stroke::new(width, color),
            StrokeKind::Inside,
        )));
    }

    pub fn rect_fs(&self, r: Rect, radius: f32, fill: Color32, bw: f32, bc: Color32) {
        self.rect(r, radius, fill);
        self.rect_stroke(r, radius, bw, bc);
    }

    pub fn circle(&self, c: Pos2, radius: f32, fill: Color32) {
        self.add(Shape::circle_filled(c, radius, fill));
    }

    pub fn circle_stroke(&self, c: Pos2, radius: f32, width: f32, color: Color32) {
        // QML border is inside the item
        self.add(Shape::circle_stroke(c, radius - width / 2.0, Stroke::new(width, color)));
    }

    pub fn hline(&self, r: Rect, color: Color32) {
        self.rect(r, 0.0, color);
    }

    // ── Text ─────────────────────────────────────────────────────────────────

    pub fn galley(&self, pos: Pos2, g: Arc<Galley>, color: Color32) {
        self.add(Shape::galley(pos, g, color));
    }

    /// Place text by an anchor point (Align2) — returns the text rect.
    pub fn text(&self, anchor: Pos2, align: Align2, text: &str, font: FontId, color: Color32) -> Rect {
        let g = self.p.layout_job(job(text, font, color));
        let r = align.anchor_size(anchor, g.size());
        self.galley(r.min, g, color);
        r
    }

    /// Text vertically centered in `r` at its left (RowLayout Text).
    pub fn text_left(&self, r: Rect, text: &str, font: FontId, color: Color32) -> Rect {
        self.text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, text, font, color)
    }

    /// Single-line elided text (Text.ElideRight) within width `w`, left-center anchored.
    pub fn text_elide(&self, left_center: Pos2, w: f32, text: &str, font: FontId, color: Color32) -> Rect {
        let g = layout_elide(&self.p, text, font, color, w);
        let r = Align2::LEFT_CENTER.anchor_size(left_center, g.size());
        self.galley(r.min, g, color);
        r
    }

    pub fn text_spaced(&self, anchor: Pos2, align: Align2, text: &str, font: FontId, color: Color32, spacing: f32) -> Rect {
        let mut job = LayoutJob::default();
        let base = letter_spacing_for(&font);
        job.append(text, 0.0, TextFormat { font_id: font, color, extra_letter_spacing: spacing + base, ..Default::default() });
        let g = self.p.layout_job(job);
        let r = align.anchor_size(anchor, g.size());
        self.galley(r.min, g, color);
        r
    }

    /// Word-wrapped text block; returns its rect.
    pub fn text_wrap(&self, top_left: Pos2, w: f32, text: &str, font: FontId, color: Color32) -> Rect {
        let mut j = job(text, font, color);
        j.wrap.max_width = w.max(1.0);
        let g = self.p.layout_job(j);
        let r = Rect::from_min_size(top_left, g.size());
        self.galley(top_left, g, color);
        r
    }

    /// Word-wrapped block whose lines are each centred on `center_x`
    /// (Text { horizontalAlignment: AlignHCenter; wrapMode: WordWrap }).
    pub fn text_wrap_center(&self, center_x: f32, top: f32, w: f32, text: &str, font: FontId, color: Color32, line_height: Option<f32>) -> Rect {
        let mut j = job(text, font, color);
        if let Some(lh) = line_height {
            for s in &mut j.sections {
                s.format.line_height = Some(lh);
            }
        }
        j.wrap.max_width = w.max(1.0);
        j.halign = egui::Align::Center;
        let g = self.p.layout_job(j);
        // Centre-aligned galleys are laid out around x = 0; place by their rect.
        let gr = g.rect;
        let pos = Pos2::new(center_x - gr.center().x, top - gr.top());
        let r = Rect::from_min_size(Pos2::new(center_x - gr.width() / 2.0, top), gr.size());
        self.galley(pos, g, color);
        r
    }

    // ── Icons ────────────────────────────────────────────────────────────────

    pub fn icon(&self, center: Pos2, name: &str, size: f32, color: Color32) -> Rect {
        self.text(center, Align2::CENTER_CENTER, &fonts::icon(name), fonts::icon_font(size), color)
    }

    /// Icon whose left edge sits at `left` (RowLayout item), vertically centered.
    pub fn icon_left(&self, left: f32, cy: f32, name: &str, size: f32, color: Color32) -> Rect {
        self.text(Pos2::new(left, cy), Align2::LEFT_CENTER, &fonts::icon(name), fonts::icon_font(size), color)
    }

    pub fn icon_rot(&self, center: Pos2, name: &str, size: f32, color: Color32, degrees: f32) {
        let g = self.p.layout_no_wrap(fonts::icon(name), fonts::icon_font(size), color);
        let half = g.size() / 2.0;
        let rot = egui::emath::Rot2::from_angle(degrees.to_radians());
        let pos = center - rot * half;
        let mut ts = TextShape::new(pos, g, color);
        ts.angle = degrees.to_radians();
        self.add(Shape::Text(ts));
    }

    pub fn image(&self, tex: &egui::TextureHandle, r: Rect, tint: Color32) {
        self.add(Shape::image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), tint));
    }

    pub fn measure(&self, text: &str, font: FontId) -> Vec2 {
        self.p.layout_job(job(text, font, Color32::WHITE)).size()
    }
}

pub fn layout_elide(p: &Painter, text: &str, font: FontId, color: Color32, w: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(text.replace('\n', " "), TextFormat::simple(font, color));
    job.wrap = TextWrapping { max_width: w.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    p.layout_job(job)
}

/// Elide in the middle (Text.ElideMiddle) — used by the toast.
pub fn elide_middle(p: &Painter, text: &str, font: FontId, w: f32) -> String {
    let full = p.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE).size().x;
    if full <= w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut keep = chars.len();
    while keep > 1 {
        keep -= 1;
        let l = keep.div_ceil(2);
        let r = keep / 2;
        let s: String = chars[..l].iter().chain(['…'].iter()).chain(chars[chars.len() - r..].iter()).collect();
        if p.layout_no_wrap(s.clone(), font.clone(), Color32::WHITE).size().x <= w {
            return s;
        }
    }
    "…".into()
}

pub fn rect_xywh(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
}

/// Rect of size `size` centered in `r`.
pub fn centered(r: Rect, size: Vec2) -> Rect {
    Rect::from_center_size(r.center(), size)
}
