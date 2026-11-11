//! Port of Components/BackupPage.qml (+ its two Dialogs)

use egui::{Align2, Color32, CursorIcon, Id, Pos2, Rect, Sense, Ui, Vec2};

use crate::anim;
use crate::app::{Action, App};
use crate::draw::{rect_xywh, Canvas};
use crate::events::BackupItem;
use crate::fonts::{f, fb};
use crate::theme::{self};
use crate::widgets::{self, RowSpec, Toast};

#[derive(Default)]
pub struct BackupState {
    pub toast: Toast,
    scroll: f32,
    options_open: bool,
    restore_open: bool,
    backup_name: String,
    restore_list: Vec<BackupItem>,
    existing_list: Vec<BackupItem>,
    focus_name: bool,
}

impl BackupState {
    pub fn on_backup_list(&mut self, list: Vec<BackupItem>) {
        self.restore_list = list.clone();
        self.existing_list = list;
    }
}

fn default_backup_name() -> String {
    let d = chrono::Local::now();
    format!("Snippets of {}", d.format("%-d %B %Y at %-I:%M %p"))
}

fn two_line_btn(ui: &mut Ui, c: &Canvas, th: &crate::theme::Theme, id: Id, r: Rect, title: &str, sub: &str, enabled: bool) -> bool {
    let resp = ui.interact(r, id, if enabled { Sense::CLICK } else { Sense::hover() });
    let cc = c.faded(if enabled { 1.0 } else { 0.5 });
    cc.rect(r, 9.0, if resp.hovered() && enabled { th.primary_hover } else { th.primary });
    let tg = cc.measure(title, fb(12.0));
    let sg = cc.measure(sub, f(9.0));
    let top = r.center().y - (tg.y + 1.0 + sg.y) / 2.0;
    cc.text(Pos2::new(r.center().x, top), Align2::CENTER_TOP, title, fb(12.0), Color32::WHITE);
    cc.faded(0.75).text(Pos2::new(r.center().x, top + tg.y + 1.0), Align2::CENTER_TOP, sub, f(9.0), Color32::WHITE);
    enabled && resp.clicked()
}

pub fn show(app: &mut App, ui: &mut Ui, page: Rect) {
    let th = app.th;
    let c = Canvas::new(ui.painter());
    c.faded(0.03).circle(Pos2::new(page.right() + 60.0 - 140.0, page.bottom() + 60.0 - 140.0), 140.0, th.primary);

    let ds = app.drive.state();
    let x0 = page.left() + 32.0;
    let w = page.width() - 64.0;
    let content_h = 29.3 + 4.0 + 16.0 + 41.0 + (15.0 + 12.0 + 60.0 * 2.0 + 12.0) + 24.0
        + (15.0 + 12.0 + 60.0 + if ds.logged_in { 12.0 + 56.0 } else { 0.0 })
        + 40.0
        + 15.0
        + 64.0;
    crate::pages::settings_page::flick(ui, page, &mut app.backup.scroll, content_h);
    let mut y = page.top() - app.backup.scroll;

    let t = c.text(Pos2::new(x0, y), Align2::LEFT_TOP, "Backup", fb(22.0), th.text_primary);
    y = t.bottom() + 4.0;
    let t = c.text(Pos2::new(x0, y), Align2::LEFT_TOP, "Protect your snippets — locally or in the cloud", f(12.0), th.text_secondary);
    y = t.bottom() + 20.0;
    c.faded(0.15).hline(rect_xywh(x0, y, w, 1.0), th.text_secondary);
    y += 21.0;

    // ── LIBRARY ────────────────────────────────────────────────────────
    y += widgets::section_label(&c, x0, y, "LIBRARY", th.text_secondary) + 12.0;
    let spec = |icon, title, desc, btn| RowSpec {
        icon,
        title,
        description: desc,
        description_color: None,
        active_color: th.primary,
        checked: false,
        is_button: true,
        button_text: btn,
        button_enabled: true,
    };
    if widgets::settings_row(ui, &th, Id::new("bk-export"), rect_xywh(x0, y, w, 60.0), &spec("upload_file", "Export Library", "Save all snippets as a JSON backup", "EXPORT")).is_some() {
        app.actions.push(Action::Export);
    }
    y += 72.0;
    if widgets::settings_row(ui, &th, Id::new("bk-import"), rect_xywh(x0, y, w, 60.0), &spec("download", "Import Library", "Restore snippets from a JSON file", "IMPORT")).is_some() {
        app.actions.push(Action::Import { toast: true });
    }
    y += 60.0 + 24.0;

    // ── GOOGLE BACKUP ──────────────────────────────────────────────────
    y += widgets::section_label(&c, x0, y, "GOOGLE BACKUP", th.text_secondary) + 12.0;
    let row = rect_xywh(x0, y, w, 60.0);
    c.rect(row, 14.0, th.surface);
    let ir = rect_xywh(row.left() + 12.0, row.center().y - 16.0, 32.0, 32.0);
    c.rect(ir, 8.0, if ds.logged_in { th.primary_light } else { th.surface });
    c.icon(ir.center(), if ds.logged_in { "cloud_done" } else { "cloud" }, 18.0, if ds.logged_in { theme::GREEN } else { th.text_secondary });
    let ab = rect_xywh(row.right() - 12.0 - 92.0, row.center().y - 15.0, 92.0, 30.0);
    let ab_resp = ui.interact(ab, Id::new("bk-account"), Sense::CLICK);
    c.rect(ab, 8.0, if ab_resp.hovered() { th.primary_hover } else { th.primary });
    c.text(ab.center(), Align2::CENTER_CENTER, if ds.logged_in { "SIGN OUT" } else { "SIGN IN" }, fb(10.0), Color32::WHITE);
    if ab_resp.clicked() {
        if ds.logged_in {
            app.drive.logout();
        } else {
            app.drive.login();
        }
    }
    let tw = ab.left() - 12.0 - (row.left() + 48.0);
    let title = if ds.logged_in { "Google Drive connected" } else { "Connect Google account" };
    let desc = if ds.logged_in {
        format!(
            "{}{}",
            ds.account_name,
            if ds.last_backup.is_empty() { "  ·  No backup yet".to_string() } else { format!("  ·  Last backup: {}", ds.last_backup) }
        )
    } else {
        "Back up and sync your snippets securely".into()
    };
    let tg = crate::draw::layout_elide(&c.p, title, fb(13.0), th.text_primary, tw);
    let dg = crate::draw::layout_elide(&c.p, &desc, f(11.0), th.text_secondary, tw);
    let top = row.center().y - (tg.size().y + 1.0 + dg.size().y) / 2.0;
    let th_h = tg.size().y;
    c.galley(Pos2::new(row.left() + 48.0, top), tg, th.text_primary);
    c.galley(Pos2::new(row.left() + 48.0, top + th_h + 1.0), dg, th.text_secondary);
    y += 60.0;

    if ds.logged_in {
        y += 12.0;
        let br = rect_xywh(x0, y, w, 56.0);
        c.rect(br, 14.0, th.surface);
        let inner = br.shrink(12.0);
        let bw = (inner.width() - 10.0) / 2.0;
        let b1 = rect_xywh(inner.left(), inner.top(), bw, inner.height());
        let b2 = rect_xywh(b1.right() + 10.0, inner.top(), bw, inner.height());
        if two_line_btn(ui, &c, &th, Id::new("bk-backup"), b1, "Backup to Drive", "Upload snippets", !ds.busy) {
            app.backup.backup_name = default_backup_name();
            app.backup.options_open = true;
            app.backup.focus_name = true;
            app.drive.list_backups();
        }
        if two_line_btn(ui, &c, &th, Id::new("bk-restore"), b2, "Restore from Drive", "Download backup", !ds.busy) {
            app.backup.restore_list.clear();
            app.backup.restore_open = true;
            app.drive.list_backups();
        }
        y += 56.0;
    }

    y += 40.0;
    c.faded(0.5).text(Pos2::new(x0 + w / 2.0, y), Align2::CENTER_TOP, &format!("Shobdo Calok  v{}", app.updater.current_version()), f(11.0), th.text_secondary);

    app.backup.toast.paint(ui, &th, Id::new("backup-toast"), page);
}

fn outline_btn(ui: &mut Ui, c: &Canvas, id: Id, r: Rect, radius: f32, hover_bg: Color32, border: Color32, icon: &str, isz: f32, label: &str, font: egui::FontId, fg: Color32) -> bool {
    let resp = ui.interact(r, id, Sense::CLICK).on_hover_cursor(CursorIcon::PointingHand);
    c.rect_fs(r, radius, if resp.hovered() { hover_bg } else { Color32::TRANSPARENT }, 1.0, border);
    let iw = widgets::icon_text_size(c, icon, isz).x;
    let lw = c.measure(label, font.clone()).x;
    let x = r.center().x - (iw + 8.0 + lw) / 2.0;
    c.icon_left(x, r.center().y, icon, isz, fg);
    c.text(Pos2::new(x + iw + 8.0, r.center().y), Align2::LEFT_CENTER, label, font, fg);
    resp.clicked()
}

/// The two Dialogs (parented to Overlay.overlay → centered in the window).
pub fn show_dialogs(app: &mut App, _ui: &mut Ui, screen: Rect) {
    let th = app.th;
    let busy = app.drive.state().busy;
    let ctx = _ui.ctx().clone();

    // ── Backup options dialog ──────────────────────────────────────────
    if app.backup.options_open {
        let (mut mu, _) = widgets::modal_layer(&ctx, Id::new("bk-options"), screen, true);
        let dr = Rect::from_center_size(screen.center(), Vec2::new(420.0, 510.0));
        let c = Canvas::new(mu.painter());
        c.rect_fs(dr, 16.0, th.surface, 1.0, th.hover_bg);
        let _eat = mu.interact(dr, Id::new("bk-options-box"), Sense::CLICK);
        let inner = dr.shrink(12.0);
        let x = inner.left();
        let w = inner.width();
        let mut y = inner.top();
        let t = c.text(Pos2::new(x, y), Align2::LEFT_TOP, "Backup to Google Drive", fb(16.0), th.text_primary);
        y = t.bottom() + 12.0;
        let t = c.text(Pos2::new(x, y), Align2::LEFT_TOP, "Backup Name", fb(11.0), th.text_secondary);
        y = t.bottom() + 12.0;
        let ir = rect_xywh(x, y, w, 40.0);
        let nid = Id::new("bk-name-input");
        let focused = ctx.memory(|m| m.has_focus(nid));
        c.rect_fs(ir, 10.0, if th.is_dark { theme::hex(0x1a1d23) } else { theme::hex(0xf8fafc) }, 1.0, if focused { th.primary } else { th.hover_bg });
        let out = widgets::line_edit(&mut mu, nid, ir.shrink(10.0), &mut app.backup.backup_name, f(13.0), th.text_primary, None);
        if app.backup.focus_name {
            widgets::request_focus(&mu, nid);
            app.backup.focus_name = false;
        }
        y += 40.0 + 12.0;
        c.faded(0.15).hline(rect_xywh(x, y, w, 1.0), th.text_secondary);
        y += 1.0 + 12.0;

        let cr = rect_xywh(x, y, w, 44.0);
        let cresp = mu.interact(cr, Id::new("bk-create"), Sense::CLICK).on_hover_cursor(CursorIcon::PointingHand);
        c.rect(cr, 10.0, if cresp.hovered() { th.primary_hover } else { th.primary });
        let iw = widgets::icon_text_size(&c, "cloud_upload", 18.0).x;
        let lw = c.measure("Create Backup", fb(13.0)).x;
        let bx = cr.center().x - (iw + 8.0 + lw) / 2.0;
        c.icon_left(bx, cr.center().y, "cloud_upload", 18.0, Color32::WHITE);
        c.text(Pos2::new(bx + iw + 8.0, cr.center().y), Align2::LEFT_CENTER, "Create Backup", fb(13.0), Color32::WHITE);
        let mut create = cresp.clicked() || out.submitted;
        y += 44.0 + 12.0;

        let del_old = outline_btn(&mut mu, &c, Id::new("bk-delold"), rect_xywh(x, y, w, 44.0), 10.0, theme::hex(0xfef2f2), theme::RED, "delete_sweep", 18.0, "Delete Old Backups", fb(13.0), theme::RED);
        y += 44.0 + 12.0;
        let reset = outline_btn(&mut mu, &c, Id::new("bk-reset"), rect_xywh(x, y, w, 40.0), 10.0, theme::hex(0xfef2f2), th.text_secondary, "refresh", 16.0, "Reset Backup Names", fb(12.0), th.text_secondary);
        y += 40.0 + 12.0;
        c.faded(0.15).hline(rect_xywh(x, y, w, 1.0), th.text_secondary);
        y += 1.0 + 12.0;
        let t = c.text(Pos2::new(x, y), Align2::LEFT_TOP, "Existing Backups", fb(11.0), th.text_secondary);
        y = t.bottom() + 12.0;
        let bottom_reserved = 4.0 + 32.0 + 12.0;
        let list_h = (inner.bottom() - bottom_reserved - y).clamp(80.0, 120.0);
        let lr = rect_xywh(x, y, w, list_h);
        c.rect_fs(lr, 10.0, if th.is_dark { theme::hex(0x1a1d23) } else { theme::hex(0xf8fafc) }, 1.0, th.hover_bg);
        let lc = c.clipped(lr.shrink(6.0));
        let mut ry = lr.top() + 6.0;
        for (k, it) in app.backup.existing_list.iter().enumerate() {
            let rr = rect_xywh(lr.left() + 6.0, ry, lr.width() - 12.0, 28.0);
            let inn = rr.shrink(4.0);
            let icr = lc.icon_left(inn.left(), inn.center().y, if k == 0 { "cloud_done" } else { "history" }, 14.0, if k == 0 { th.primary } else { th.text_secondary });
            lc.text_elide(Pos2::new(icr.right() + 6.0, inn.center().y), inn.right() - icr.right() - 6.0, &it.label, f(11.0), th.text_primary);
            ry += 31.0;
        }
        y += list_h + 12.0 + 4.0;
        let cancel_r = rect_xywh(inner.right() - 80.0, y, 80.0, 32.0);
        let cresp2 = mu.interact(cancel_r, Id::new("bk-opt-cancel"), Sense::CLICK);
        c.rect_fs(cancel_r, 8.0, if cresp2.hovered() { th.hover_bg } else { Color32::TRANSPARENT }, 1.0, th.text_secondary);
        c.text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", f(11.0), th.text_secondary);

        if del_old {
            app.backup.options_open = false;
            app.drive.delete_old_backups();
            create = false;
        }
        if reset {
            app.backup.options_open = false;
            app.drive.clear_metadata();
            create = false;
        }
        if create {
            app.backup.options_open = false;
            app.drive.backup_with_name(app.backup.backup_name.clone());
        }
        if cresp2.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.backup.options_open = false;
        }
    }

    // ── Restore picker ─────────────────────────────────────────────────
    if app.backup.restore_open {
        let (mut mu, _) = widgets::modal_layer(&ctx, Id::new("bk-restore-dlg"), screen, true);
        let dr = Rect::from_center_size(screen.center(), Vec2::new(460.0, 480.0));
        let c = Canvas::new(mu.painter());
        c.rect_fs(dr, 16.0, th.surface, 1.0, th.hover_bg);
        let _eat = mu.interact(dr, Id::new("bk-restore-box"), Sense::CLICK);
        let inner = dr.shrink(12.0);
        let (x, w) = (inner.left(), inner.width());
        let mut y = inner.top();
        let t = c.text(Pos2::new(x, y), Align2::LEFT_TOP, "Your Backups", fb(16.0), th.text_primary);
        y = t.bottom() + 12.0;
        let loading = busy && app.backup.restore_list.is_empty();
        if loading {
            let ringc = Pos2::new(x + 8.0, y + 8.0);
            c.circle_stroke(ringc, 8.0, 2.0, th.primary);
            c.text(Pos2::new(x + 16.0 + 8.0, y + 8.0), Align2::LEFT_CENTER, "Fetching backups from Google Drive...", f(11.0), th.text_secondary);
            y += 16.0 + 12.0;
            ctx.request_repaint();
        }
        c.faded(0.15).hline(rect_xywh(x, y, w, 1.0), th.text_secondary);
        y += 1.0 + 12.0;
        let bottom = inner.bottom() - 32.0 - 12.0 - 1.0 - 12.0;
        let lr = Rect::from_min_max(Pos2::new(x, y), Pos2::new(inner.right(), bottom));
        c.rect_fs(lr, 10.0, if th.is_dark { theme::hex(0x1a1d23) } else { theme::hex(0xf8fafc) }, 1.0, th.hover_bg);
        if loading {
            c.circle_stroke(lr.center(), 20.0, 3.0, th.primary);
        }
        let lc = c.clipped(lr.shrink(6.0));
        let mut ry = lr.top() + 6.0;
        let n = app.backup.restore_list.len();
        let mut restore_req: Option<(usize, String)> = None;
        let mut delete_req: Option<(usize, String)> = None;
        for (k, it) in app.backup.restore_list.iter().enumerate() {
            let rr = rect_xywh(lr.left() + 6.0, ry, lr.width() - 12.0, 56.0);
            ry += 60.0;
            if !rr.intersects(lr) {
                continue;
            }
            let hov = mu.interact(rr.intersect(lr), Id::new(("bk-ri", k)), Sense::hover()).hovered();
            if hov {
                lc.rect(rr, 8.0, th.primary_light);
            }
            let inn = rr.shrink(8.0);
            let ib = rect_xywh(inn.left(), inn.center().y - 18.0, 36.0, 36.0);
            lc.rect(ib, 8.0, if k == 0 { th.primary_light } else if th.is_dark { theme::hex(0x2a2d35) } else { theme::hex(0xf1f5f9) });
            lc.icon(ib.center(), if k == 0 { "cloud_done" } else { "history" }, 16.0, if k == 0 { th.primary } else { th.text_secondary });
            let mut right = inn.right();
            let del_r = rect_xywh(right - 26.0, inn.center().y - 13.0, 26.0, 26.0);
            if n > 1 {
                let dresp = mu.interact(del_r, Id::new(("bk-del", k)), Sense::CLICK).on_hover_cursor(CursorIcon::PointingHand);
                lc.rect_fs(del_r, 6.0, if dresp.hovered() { theme::hex(0xfef2f2) } else { Color32::TRANSPARENT }, 1.0, theme::RED);
                lc.icon(del_r.center(), "delete", 14.0, theme::RED);
                if dresp.clicked() {
                    delete_req = Some((k, it.id.clone()));
                }
                right = del_r.left() - 8.0;
            }
            let rb = rect_xywh(right - 64.0, inn.center().y - 13.0, 64.0, 26.0);
            let rresp = mu.interact(rb, Id::new(("bk-res", k)), Sense::CLICK).on_hover_cursor(CursorIcon::PointingHand);
            lc.rect(rb, 6.0, if rresp.hovered() { th.primary_hover } else { th.primary });
            lc.text(rb.center(), Align2::CENTER_CENTER, "RESTORE", fb(8.0), Color32::WHITE);
            if rresp.clicked() {
                restore_req = Some((k, it.id.clone()));
            }
            let tx = ib.right() + 8.0;
            let tw = rb.left() - 8.0 - tx;
            let ng = crate::draw::layout_elide(&lc.p, &it.backup_name, fb(12.0), th.text_primary, tw);
            let tg = crate::draw::layout_elide(&lc.p, &it.time_label, f(10.0), th.text_secondary, tw);
            let top = inn.center().y - (ng.size().y + 2.0 + tg.size().y) / 2.0;
            let nh = ng.size().y;
            lc.galley(Pos2::new(tx, top), ng, th.text_primary);
            lc.galley(Pos2::new(tx, top + nh + 2.0), tg, th.text_secondary);
        }
        let dy = inner.bottom() - 32.0 - 12.0 - 1.0;
        c.faded(0.15).hline(rect_xywh(x, dy, w, 1.0), th.text_secondary);
        let cancel_r = rect_xywh(inner.right() - 80.0, inner.bottom() - 32.0, 80.0, 32.0);
        let cresp = mu.interact(cancel_r, Id::new("bk-res-cancel"), Sense::CLICK);
        c.rect_fs(cancel_r, 8.0, if cresp.hovered() { th.hover_bg } else { Color32::TRANSPARENT }, 1.0, th.text_secondary);
        c.text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", f(11.0), th.text_secondary);

        if let Some((k, id)) = restore_req {
            app.backup.restore_open = false;
            if k == 0 {
                app.drive.restore();
            } else {
                app.drive.restore_from_file(id);
            }
        }
        if let Some((k, id)) = delete_req {
            app.drive.delete_backup(id);
            app.backup.restore_list.remove(k);
        }
        if cresp.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            app.backup.restore_open = false;
        }
    }
    let _ = anim::Ease::Linear;
}
