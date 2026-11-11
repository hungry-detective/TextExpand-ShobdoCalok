//! Reusable pieces shared by several QML files: ToolTip styling, the toast,
//! SettingsRow, text inputs, modal dim layer.

use egui::{Align, Align2, Color32, Id, LayerId, Order, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};

use crate::anim::{self, Ease};
use crate::draw::{self, Canvas};
use crate::fonts::{f, fb, icon_font};
use crate::theme::{self, Theme};

// ── ToolTip ─────────────────────────────────────────────────────────────────

pub enum TipPos {
    /// Qt Quick Controls default: centered above the parent.
    Above,
    /// x: parent.width + gap; vertically centered (sidebar).
    Right(f32),
}

/// Styled tooltip: surface bg, 1px border, radius 6, 11px text.
pub fn tooltip(ui: &Ui, th: &Theme, id: Id, hovered: bool, anchor: Rect, text: &str, delay_ms: f32, pos: TipPos, min: Vec2) {
    let ctx = ui.ctx();
    let key = id.with("tip-hover-start");
    if !hovered {
        ctx.data_mut(|d| d.remove::<f64>(key));
        return;
    }
    let now = ctx.input(|i| i.time);
    let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(key, || now));
    let wait = delay_ms as f64 / 1000.0 - (now - start);
    if wait > 0.0 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        return;
    }
    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, id.with("tip-layer")));
    let c = Canvas::new(&painter);
    let pad = 6.0;
    let ts = c.measure(text, f(11.0));
    let size = Vec2::new((ts.x + pad * 2.0).max(min.x), (ts.y + pad * 2.0).max(min.y));
    let r = match pos {
        TipPos::Above => Rect::from_min_size(Pos2::new(anchor.center().x - size.x / 2.0, anchor.top() - size.y - 3.0), size),
        TipPos::Right(gap) => Rect::from_min_size(Pos2::new(anchor.right() + gap, anchor.center().y - size.y / 2.0), size),
    };
    c.rect_fs(r, 6.0, th.surface, 1.0, th.tip_border);
    c.text(Pos2::new(r.left() + pad, r.center().y), Align2::LEFT_CENTER, text, f(11.0), th.text_primary);
}

// ── Toast ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct Toast {
    text: String,
    shown_at: Option<f64>,
    ever_shown: bool,
}

impl Toast {
    pub fn show(&mut self, ctx: &egui::Context, msg: &str) {
        self.text = msg.to_string();
        self.ever_shown = true;
        self.shown_at = Some(ctx.input(|i| i.time));
        ctx.request_repaint();
    }

    /// Bottom-centered toast inside `page` (width ≤ page.w - 80, h 36, r 10).
    pub fn paint(&self, ui: &Ui, th: &Theme, id: Id, page: Rect) {
        if !self.ever_shown {
            return;
        }
        let ctx = ui.ctx();
        let now = ctx.input(|i| i.time);
        let visible = self.shown_at.map(|t| now - t < 2.6).unwrap_or(false);
        if visible {
            if let Some(t) = self.shown_at {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64((2.6 - (now - t)).max(0.0)));
            }
        }
        let op = anim::anim(ctx, id.with("toast-op"), if visible { 1.0 } else { 0.0 }, 200.0, Ease::Linear);
        if op <= 0.001 {
            return;
        }
        let painter = ui.painter().clone();
        let c = Canvas::new(&painter).faded(op);
        let tw = c.measure(&self.text, f(11.0)).x;
        let w = (tw + 40.0).min(page.width() - 80.0).max(0.0);
        let r = Rect::from_min_size(Pos2::new(page.center().x - w / 2.0, page.bottom() - 20.0 - 36.0), Vec2::new(w, 36.0));
        let bg = if th.is_dark { theme::hex(0x1a1d23) } else { theme::hex(0x0f172a) };
        c.rect_fs(r, 10.0, bg, 1.0, th.hover_bg);
        let shown = draw::elide_middle(&painter, &self.text, f(11.0), (w - 20.0).max(1.0));
        c.text(r.center(), Align2::CENTER_CENTER, &shown, f(11.0), Color32::WHITE);
    }
}

// ── SettingsRow (SettingsPage.qml / BackupPage.qml component) ───────────────

pub struct RowSpec<'a> {
    pub icon: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub description_color: Option<Color32>,
    pub active_color: Color32,
    pub checked: bool,
    pub is_button: bool,
    pub button_text: &'a str,
    pub button_enabled: bool,
}

/// Returns Some(new_value) when toggled / clicked.
pub fn settings_row(ui: &mut Ui, th: &Theme, id: Id, r: Rect, spec: &RowSpec) -> Option<bool> {
    let ctx = ui.ctx().clone();
    let c = Canvas::new(ui.painter());
    c.rect(r, 14.0, th.surface);

    // 1. Icon (left 12)
    let ir = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.center().y - 16.0), Vec2::splat(32.0));
    let icon_bg = anim::anim_color(&ctx, id.with("ibg"), if spec.checked { th.primary_light } else { th.surface }, 200.0);
    c.rect(ir, 8.0, icon_bg);
    let icon_col = anim::anim_color(&ctx, id.with("icol"), if spec.checked { spec.active_color } else { th.text_secondary }, 200.0);
    c.icon(ir.center(), spec.icon, 18.0, icon_col);

    // 3. Toggle / button (right 12)
    let tw = if spec.is_button { 90.0 } else { 50.0 };
    let toggle_item = Rect::from_min_max(Pos2::new(r.right() - 12.0 - tw, r.top()), Pos2::new(r.right() - 12.0, r.bottom()));

    // 2. Text column at x+48
    let text_w = (toggle_item.left() - 12.0 - (r.left() + 48.0)).max(1.0);
    let tg = draw::layout_elide(&c.p, spec.title, fb(13.0), th.text_primary, text_w);
    let dg = draw::layout_elide(&c.p, spec.description, f(11.0), spec.description_color.unwrap_or(th.text_secondary), text_w);
    let total = tg.size().y + 1.0 + dg.size().y;
    let top = r.center().y - total / 2.0;
    c.galley(Pos2::new(r.left() + 48.0, top), tg.clone(), th.text_primary);
    c.galley(Pos2::new(r.left() + 48.0, top + tg.size().y + 1.0), dg, spec.description_color.unwrap_or(th.text_secondary));

    let mut out = None;
    if !spec.is_button {
        let pill = Rect::from_center_size(toggle_item.center(), Vec2::new(44.0, 24.0));
        let off = if th.is_dark { theme::hex(0x3a3d47) } else { theme::hex(0xd1d5db) };
        let col = anim::anim_color(&ctx, id.with("pill"), if spec.checked { th.primary } else { off }, 200.0);
        c.rect(pill, 12.0, col);
        let kx = anim::anim(&ctx, id.with("knob"), if spec.checked { pill.width() - 18.0 - 3.0 } else { 3.0 }, 200.0, Ease::OutCubic);
        c.circle(Pos2::new(pill.left() + kx + 9.0, pill.center().y), 9.0, Color32::WHITE);
        if ui.interact(pill, id.with("pill-tap"), Sense::CLICK).clicked() {
            out = Some(!spec.checked);
        }
    } else {
        let br = Rect::from_center_size(toggle_item.center(), Vec2::new(82.0, 30.0));
        let resp = ui.interact(br, id.with("btn"), Sense::CLICK);
        let disabled = if th.is_dark { theme::hex(0x3a3d47) } else { theme::hex(0xd1d5db) };
        let col = if !spec.button_enabled {
            disabled
        } else if resp.hovered() {
            th.primary_hover
        } else {
            th.primary
        };
        c.rect(br, 8.0, col);
        c.text(br.center(), Align2::CENTER_CENTER, spec.button_text, fb(10.0), if spec.button_enabled { Color32::WHITE } else { th.text_secondary });
        if resp.clicked() && spec.button_enabled {
            out = Some(true);
        }
    }
    out
}

/// Section label ("SYSTEM", "APPEARANCE"…) at the locked 48px baseline.
pub fn section_label(c: &Canvas, x: f32, y: f32, text: &str, color: Color32) -> f32 {
    let r = c.text(Pos2::new(x + 48.0, y), Align2::LEFT_TOP, text, fb(11.0), color);
    r.height()
}

// ── Text input helpers ──────────────────────────────────────────────────────

pub struct LineEditOut {
    pub changed: bool,
    pub has_focus: bool,
    pub submitted: bool,
    pub escaped: bool,
}

/// QML `TextInput` (frameless) placed inside `r`, vertically centered.
#[allow(clippy::too_many_arguments)]
pub fn line_edit(
    ui: &mut Ui,
    id: Id,
    r: Rect,
    text: &mut String,
    font: egui::FontId,
    color: Color32,
    placeholder: Option<(&str, egui::FontId, Color32, bool)>,
) -> LineEditOut {
    let had_focus = ui.ctx().memory(|m| m.has_focus(id));
    let enter = had_focus && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter));
    let esc = had_focus && ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
    // Give the TextEdit a rect exactly one text row tall, centred in `r`, so the
    // text sits on the same vertical centre as QML's AlignVCenter TextInput.
    let row_h = ui.painter().layout_no_wrap("Ag".into(), font.clone(), color).size().y;
    let er = Rect::from_center_size(r.center(), Vec2::new(r.width(), row_h));
    let mut child = ui.new_child(UiBuilder::new().max_rect(er).layout(egui::Layout::left_to_right(Align::Min)));
    child.spacing_mut().interact_size.y = 0.0;
    child.spacing_mut().item_spacing = Vec2::ZERO;
    let out = egui::TextEdit::singleline(text)
        .id(id)
        .font(font)
        .text_color(color)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .desired_width(r.width())
        .show(&mut child);
    let has_focus = ui.ctx().memory(|m| m.has_focus(id));
    if let Some((ph, pf, pc, hide_on_focus)) = placeholder {
        if text.is_empty() && !(hide_on_focus && has_focus) {
            let c = Canvas::new(ui.painter());
            c.clipped(r).text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, ph, pf, pc);
        }
    }
    LineEditOut { changed: out.response.changed(), has_focus, submitted: enter, escaped: esc }
}

pub fn request_focus(ui: &Ui, id: Id) {
    ui.ctx().memory_mut(|m| m.request_focus(id));
}

pub fn select_all(ctx: &egui::Context, id: Id, text: &str) {
    let mut st = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let n = text.chars().count();
    st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(n))));
    st.store(ctx, id);
}

// ── Modal dim layer ─────────────────────────────────────────────────────────

/// A Foreground layer that dims the whole window (Overlay.modal, 50% black)
/// and swallows input. Returns (painter-ui, clicked_outside).
pub fn modal_layer(ctx: &egui::Context, id: Id, screen: Rect, dim: bool) -> (Ui, bool) {
    let layer = LayerId::new(Order::Foreground, id);
    let mut ui = Ui::new(ctx.clone(), id, UiBuilder::new().layer_id(layer).max_rect(screen));
    if dim {
        ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(128));
    }
    let bg = ui.interact(screen, id.with("modal-bg"), Sense::CLICK);
    let clicked_outside = bg.clicked();
    (ui, clicked_outside)
}

/// Rectangle "button" with hover detection.
pub fn hit(ui: &Ui, id: Id, r: Rect) -> egui::Response {
    ui.interact(r, id, Sense::CLICK)
}

pub fn icon_text_size(c: &Canvas, name: &str, size: f32) -> Vec2 {
    c.measure(&crate::fonts::icon(name), icon_font(size))
}
