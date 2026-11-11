//! Port of Components/EditorPanel.qml

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Align2, Color32, Id, LayerId, Order, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};

use crate::anim::{self, Ease};
use crate::app::{Action, App};
use crate::draw::{rect_xywh, Canvas};
use crate::fonts::{f, fb, icon_font, mono_b};
use crate::svg;
use crate::theme::{self, Theme};
use crate::widgets;

const MAIN_TEXT: &str = "editor-main-text";

#[derive(Default)]
pub struct EditorState {
    pub current_folder: String,
    pub current_abbrev: String,
    pub current_content: String,
    /// Folder chosen in the dropdown but not yet saved to the backend
    pub pending_folder: String,
    pub folder_names: Vec<String>,
    pub text: String,
    pub abbrev_input: String,
    preview_src: Option<String>,
    preview: String,
    saved_until: f64,
    delete_dialog: bool,
    dropdown: Option<Rect>,
    dropdown_fresh: bool,
}

impl EditorState {
    pub fn on_snippet_selected(&mut self, folder: &str, abbrev: &str, content: &str) {
        self.current_folder = folder.into();
        self.pending_folder = folder.into();
        self.current_abbrev = abbrev.into();
        self.current_content = content.into();
        self.text = content.into();
        self.abbrev_input = abbrev.into();
    }

    pub fn clear(&mut self) {
        self.current_abbrev.clear();
        self.current_content.clear();
        self.current_folder.clear();
        self.pending_folder.clear();
        self.text.clear();
        self.abbrev_input.clear();
    }

    pub fn flash_saved(&mut self, ctx: &egui::Context) {
        self.saved_until = ctx.input(|i| i.time) + 2.0;
        anim::restart(ctx, Id::new("save-pulse"));
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
    }

    fn insert_token(&mut self, ctx: &egui::Context, token: &str) {
        let id = Id::new(MAIN_TEXT);
        let n = self.text.chars().count();
        let pos = TextEditState::load(ctx, id)
            .and_then(|s| s.cursor.char_range())
            .map(|r| r.primary.index.0.min(n))
            .unwrap_or(n);
        let byte = self.text.char_indices().nth(pos).map(|(b, _)| b).unwrap_or(self.text.len());
        self.text.insert_str(byte, token);
        let mut st = TextEditState::load(ctx, id).unwrap_or_default();
        st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(pos + token.chars().count()))));
        st.store(ctx, id);
    }
}

fn token_btn(ui: &mut Ui, c: &Canvas, th: &Theme, id: Id, x: f32, cy: f32, icon: &str, label: &str, highlight: bool) -> (f32, bool) {
    let iw = widgets::icon_text_size(c, icon, 16.0).x;
    let lw = c.measure(label, fb(9.0)).x;
    let w = iw + 4.0 + lw;
    let r = rect_xywh(x, cy - 10.0, w, 20.0);
    let resp = ui.interact(r, id, Sense::CLICK);
    let col = if resp.hovered() || highlight { th.primary } else { th.text_secondary };
    c.icon_left(x, cy, icon, 16.0, col);
    c.text(Pos2::new(x + iw + 4.0, cy), Align2::LEFT_CENTER, label, fb(9.0), col);
    (w, resp.clicked())
}

pub fn show(app: &mut App, ui: &mut Ui, root: Rect) {
    let th = app.th;
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let app_icon = app.svg.get_raw(&ctx, "app-icon", svg::APP_ICON, 16.0, 16.0);
    let ed = &mut app.editor;
    let actions = &mut app.actions;
    let c = Canvas::new(ui.painter());
    let content = root.shrink(16.0);

    // ── Header row ─────────────────────────────────────────────────────
    let hy = content.top();
    let del_r = rect_xywh(content.right() - 34.0, hy, 34.0, 34.0);
    let ab_r = rect_xywh(del_r.left() - 12.0 - 130.0, hy, 130.0, 34.0);
    let fp_r = rect_xywh(ab_r.left() - 12.0 - 160.0, hy, 160.0, 34.0);
    c.text(Pos2::new(content.left(), hy + 17.0), Align2::LEFT_CENTER, "Snippet Editor", fb(16.0), th.text_primary);

    // Folder picker
    let fp_resp = ui.interact(fp_r, Id::new("ed-folder-picker"), Sense::CLICK);
    c.rect(fp_r, 10.0, if fp_resp.hovered() { th.hover_bg } else { th.surface });
    let has_pending = !ed.pending_folder.is_empty();
    let fi = c.icon_left(fp_r.left() + 10.0, fp_r.center().y, "folder", 14.0, if has_pending { th.primary } else { th.text_secondary });
    let chev_w = if ed.current_abbrev.is_empty() { 0.0 } else { widgets::icon_text_size(&c, "expand_more", 14.0).x };
    if !ed.current_abbrev.is_empty() {
        c.text(Pos2::new(fp_r.right() - 10.0, fp_r.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("expand_more"), icon_font(14.0), th.text_secondary);
    }
    let name = if has_pending { ed.pending_folder.clone() } else { "— no folder —".into() };
    let nx = fi.right() + 5.0;
    c.text_elide(Pos2::new(nx, fp_r.center().y), fp_r.right() - 10.0 - chev_w - 5.0 - nx, &name, fb(11.0), if has_pending { th.text_primary } else { th.text_secondary });
    if fp_resp.clicked() && !ed.current_abbrev.is_empty() {
        if ed.dropdown.is_some() {
            ed.dropdown = None;
        } else {
            ed.dropdown = Some(fp_r);
            ed.dropdown_fresh = true;
        }
    }

    // Abbreviation box
    c.rect(ab_r, 10.0, th.surface);
    c.faded(0.7).image(&app_icon, rect_xywh(ab_r.left() + 10.0, ab_r.center().y - 8.0, 16.0, 16.0), Color32::WHITE);
    let in_r = Rect::from_min_max(Pos2::new(ab_r.left() + 10.0 + 16.0 + 6.0, ab_r.top()), Pos2::new(ab_r.right() - 10.0, ab_r.bottom()));
    widgets::line_edit(ui, Id::new("ed-abbrev"), in_r, &mut ed.abbrev_input, mono_b(12.0), th.primary, Some(("trigger word", mono_b(12.0), th.text_secondary, true)));

    // Delete
    let del_resp = ui.interact(del_r, Id::new("ed-delete"), Sense::CLICK);
    c.rect(del_r, 10.0, if del_resp.hovered() { th.danger } else { th.danger_light });
    c.icon(del_r.center(), "delete", 18.0, if del_resp.hovered() { Color32::WHITE } else { th.danger });
    if del_resp.clicked() && !ed.current_abbrev.is_empty() {
        ed.delete_dialog = true;
    }

    // ── Token insertion + SAVE row ─────────────────────────────────────
    let ty = hy + 34.0 + 16.0;
    let tcy = ty + 16.0;
    let ins = c.text(Pos2::new(content.left(), tcy), Align2::LEFT_CENTER, "INSERT", fb(9.0), th.text_secondary);
    let mut x = ins.right() + 8.0 + 2.0 + 8.0;
    let tokens = [
        ("content_copy", "CLIP", false, "{clipboard}"),
        ("calendar_today", "DATE", false, "{date}"),
        ("schedule", "TIME", false, "{time}"),
        ("input", "FIELD", true, "{field:Label}"),
        ("text_fields", "CURSOR", true, "{cursor}"),
    ];
    for (k, (icon, label, hl, tok)) in tokens.iter().enumerate() {
        let (w, clicked) = token_btn(ui, &c, &th, Id::new(("ed-token", k)), x, tcy, icon, label, *hl);
        if clicked {
            ed.insert_token(&ctx, tok);
        }
        x += w + 8.0;
    }
    let save_r = rect_xywh(content.right() - 130.0, ty, 130.0, 32.0);
    let save_resp = ui.interact(save_r, Id::new("ed-save"), Sense::CLICK);
    let feedback = now < ed.saved_until;
    let save_col = if feedback {
        theme::SUCCESS
    } else if save_resp.hovered() {
        th.primary
    } else {
        th.primary_hover
    };
    let scale = anim::pulse(&ctx, Id::new("save-pulse"), 0.95, 60.0, Ease::Linear, 100.0, Ease::OutBack);
    let sc = c.scaled(save_r.center(), scale);
    sc.rect(save_r, 8.0, save_col);
    sc.text(save_r.center(), Align2::CENTER_CENTER, if feedback { "SAVED!" } else { "SAVE CHANGES" }, fb(9.0), Color32::WHITE);
    if save_resp.clicked() {
        actions.push(Action::EditorSave);
    }

    // ── Editor + live preview ──────────────────────────────────────────
    let show_preview = !ed.current_abbrev.is_empty();
    let area_top = ty + 32.0 + 16.0;
    let area_bottom = if show_preview { content.bottom() - 96.0 - 16.0 } else { content.bottom() };
    let editor_r = Rect::from_min_max(Pos2::new(content.left(), area_top), Pos2::new(content.right(), area_bottom.max(area_top)));
    c.rect(editor_r, 16.0, th.surface);
    {
        let sv = editor_r.shrink(4.0);
        let mut child = ui.new_child(UiBuilder::new().max_rect(sv));
        child.set_clip_rect(sv);
        egui::ScrollArea::vertical().id_salt("ed-scroll").auto_shrink(false).show(&mut child, |ui| {
            egui::TextEdit::multiline(&mut ed.text)
                .id(Id::new(MAIN_TEXT))
                .font(f(13.0))
                .text_color(th.text_primary)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: 24, top: 16, bottom: 16 }))
                .desired_width(sv.width())
                .min_size(Vec2::new(sv.width(), sv.height()))
                .hint_text(egui::RichText::new("Select a snippet from the library to edit it…").font(f(13.0)).color(th.text_secondary))
                .show(ui);
        });
    }

    if show_preview {
        let pr = rect_xywh(content.left(), content.bottom() - 96.0, content.width(), 96.0);
        c.rect(pr, 12.0, th.surface);
        let pin = pr.shrink(10.0);
        let ir = c.icon_left(pin.left(), pin.top() + 8.0, "preview", 13.0, th.primary);
        c.text_spaced(Pos2::new(ir.right() + 6.0, pin.top() + 8.0), Align2::LEFT_CENTER, "LIVE PREVIEW", fb(9.0), th.text_secondary, 1.0);
        let box_r = Rect::from_min_max(Pos2::new(pin.left(), pin.top() + 16.0 + 6.0), pin.max);
        c.rect(box_r, 8.0, th.editor_bg);
        if ed.preview_src.as_deref() != Some(ed.text.as_str()) {
            ed.preview = crate::engine::resolve_tokens(&ed.text, true, false);
            ed.preview_src = Some(ed.text.clone());
        }
        let sv = box_r.shrink(2.0);
        let mut child = ui.new_child(UiBuilder::new().max_rect(sv));
        child.set_clip_rect(sv);
        let mut ro: &str = ed.preview.as_str();
        egui::ScrollArea::vertical().id_salt("ed-preview-scroll").auto_shrink(false).show(&mut child, |ui| {
            egui::TextEdit::multiline(&mut ro)
                .id(Id::new("ed-preview-text"))
                .font(f(12.0))
                .text_color(th.text_primary)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(8)))
                .desired_width(sv.width())
                .min_size(Vec2::new(sv.width(), sv.height()))
                .show(ui);
        });
    }

    // ── Delete confirmation overlay ────────────────────────────────────
    if ed.delete_dialog {
        c.rect(root, 12.0, theme::argb(0x80000000));
        let box_r = Rect::from_center_size(root.center(), Vec2::new(320.0, 160.0));
        c.rect(box_r, 16.0, th.surface);
        let inner = box_r.shrink(24.0);
        let t1 = c.text(inner.left_top(), Align2::LEFT_TOP, "Delete Snippet?", fb(15.0), th.text_primary);
        c.text_wrap(Pos2::new(inner.left(), t1.bottom() + 6.0), inner.width(), "This action cannot be undone.", f(11.0), th.text_secondary);
        let bw = (inner.width() - 10.0) / 2.0;
        let cancel_r = rect_xywh(inner.left(), inner.bottom() - 36.0, bw, 36.0);
        let del2_r = rect_xywh(cancel_r.right() + 10.0, inner.bottom() - 36.0, bw, 36.0);
        let cresp = ui.interact(cancel_r, Id::new("ed-del-cancel"), Sense::CLICK);
        c.rect_fs(cancel_r, 10.0, if cresp.hovered() { th.hover_bg } else { th.surface }, 1.0, th.text_secondary);
        c.text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", fb(11.0), th.text_primary);
        let dresp = ui.interact(del2_r, Id::new("ed-del-confirm"), Sense::CLICK);
        c.rect(del2_r, 10.0, if dresp.hovered() { th.danger } else { th.danger_light });
        c.text(del2_r.center(), Align2::CENTER_CENTER, "Delete", fb(11.0), if dresp.hovered() { Color32::WHITE } else { th.danger });
        if cresp.clicked() {
            ed.delete_dialog = false;
        }
        if dresp.clicked() {
            ed.delete_dialog = false;
            actions.push(Action::EditorDelete);
        }
    }

    // ── Folder picker popup ────────────────────────────────────────────
    if let Some(anchor) = ed.dropdown {
        let n = ed.folder_names.len() as f32;
        let pr = rect_xywh(anchor.left(), anchor.bottom() + 4.0, 180.0, (n * 36.0 + 8.0).min(200.0));
        let layer = LayerId::new(Order::Foreground, Id::new("ed-dropdown"));
        let mut lu = Ui::new(ctx.clone(), Id::new("ed-dropdown-ui"), UiBuilder::new().layer_id(layer).max_rect(pr));
        let pc = Canvas::new(lu.painter());
        pc.rect_fs(pr, 10.0, th.surface, 1.0, th.hover_bg);
        let _bg = lu.interact(pr, Id::new("ed-dd-bg"), Sense::CLICK);
        let list_r = pr.shrink(4.0);
        let mut chosen = None;
        let mut child = lu.new_child(UiBuilder::new().max_rect(list_r));
        child.set_clip_rect(list_r);
        egui::ScrollArea::vertical()
            .id_salt("ed-dd-scroll")
            .auto_shrink(false)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(&mut child, |ui| {
                let (full, _) = ui.allocate_exact_size(Vec2::new(list_r.width(), n * 38.0 - 2.0), Sense::hover());
                let c2 = Canvas::new(ui.painter());
                for (k, name) in ed.folder_names.iter().enumerate() {
                    let rr = rect_xywh(full.left(), full.top() + k as f32 * 38.0, full.width(), 36.0);
                    let resp = ui.interact(rr, Id::new(("ed-dd-row", k)), Sense::CLICK);
                    let sel = *name == ed.pending_folder;
                    if resp.hovered() || sel {
                        c2.rect(rr, 8.0, th.hover_bg);
                    }
                    let ir = c2.icon_left(rr.left() + 12.0, rr.center().y, "folder", 14.0, if sel { th.primary } else { th.text_secondary });
                    let right = if sel {
                        let cr = c2.text(Pos2::new(rr.right() - 12.0, rr.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("check"), icon_font(14.0), th.primary);
                        cr.left() - 5.0
                    } else {
                        rr.right() - 12.0
                    };
                    c2.text_elide(Pos2::new(ir.right() + 5.0, rr.center().y), right - ir.right() - 5.0, name, fb(11.0), if sel { th.primary } else { th.text_primary });
                    if resp.clicked() {
                        chosen = Some(name.clone());
                    }
                }
            });
        if let Some(c) = chosen {
            ed.pending_folder = c;
            ed.dropdown = None;
        } else if !ed.dropdown_fresh {
            let outside = ctx.input(|i| {
                i.pointer.any_pressed() && i.pointer.interact_pos().map(|p| !pr.contains(p) && !anchor.contains(p)).unwrap_or(false)
            });
            if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                ed.dropdown = None;
            }
        }
        ed.dropdown_fresh = false;
    }
}
