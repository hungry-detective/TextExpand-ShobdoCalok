//! Port of Components/AboutPage.qml

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, Id, Pos2, Rect, Sense, Ui, Vec2};

use crate::anim::{self, Ease};
use crate::app::App;
use crate::draw::{rect_xywh, Canvas};
use crate::fonts::{f, fb};
use crate::theme::{self, Theme};

#[derive(Default)]
pub struct AboutState {
    status: String,
    status_color: Option<Color32>,
    reset_at: Option<f64>,
    uptodate_at: Option<f64>,
}

impl AboutState {
    pub fn on_status(&mut self, ctx: &egui::Context, msg: &str, th: &Theme) {
        if msg.is_empty() {
            return;
        }
        if msg.contains("failed") || msg.contains("error") {
            self.status = msg.into();
            self.status_color = Some(theme::RED);
        } else if msg.contains("up to date") {
            self.status = "You're up to date".into();
            self.status_color = Some(theme::GREEN);
        } else if msg.contains("available") {
            self.status = msg.into();
            self.status_color = Some(th.primary);
        } else {
            self.status = msg.into();
            self.status_color = None;
        }
        self.reset_at = Some(ctx.input(|i| i.time) + 5.0);
        ctx.request_repaint_after(std::time::Duration::from_secs(5));
    }

    pub fn on_up_to_date(&mut self, ctx: &egui::Context) {
        self.status = "You're up to date".into();
        self.status_color = Some(theme::GREEN);
        self.uptodate_at = Some(ctx.input(|i| i.time) + 4.0);
        ctx.request_repaint_after(std::time::Duration::from_secs(4));
    }
}

pub fn show(app: &mut App, ui: &mut Ui, page: Rect) {
    let th = app.th;
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let c = Canvas::new(ui.painter());
    let st = app.updater.state();

    // timers
    if app.about.reset_at.map(|t| now >= t).unwrap_or(false) {
        app.about.reset_at = None;
        app.about.status.clear();
    }
    if app.about.uptodate_at.map(|t| now >= t).unwrap_or(false) {
        app.about.uptodate_at = None;
        app.about.status.clear();
    }

    // ── Animated background circles ────────────────────────────────────
    let s1 = anim::ping_pong(&ctx, 0.96, 1.08, 3000.0, Ease::InOutSine);
    c.faded(0.04).circle(page.center() + Vec2::new(-80.0, -60.0), 160.0 * s1, th.primary);
    let s2 = anim::ping_pong(&ctx, 0.9, 1.12, 4000.0, Ease::InOutSine);
    c.faded(0.05).circle(page.center() + Vec2::new(100.0, 80.0), 100.0 * s2, theme::SKY);

    // ── Main column ────────────────────────────────────────────────────
    let w = (page.width() * 0.85).min(420.0);
    let desc = "Shobdo Calok is a fast, keyboard-friendly text expander that\nlets you define short abbreviations which automatically expand\ninto full phrases, code blocks, or dynamic templates.\n\nSupports date/time tokens, clipboard insertion, fill-in fields,\nand folder-based snippet organisation — all running silently\nin your system tray.";
    let mut job = LayoutJob::default();
    let line_h = 13.0 * 1.33 * 1.5;
    job.append(desc, 0.0, TextFormat { font_id: f(13.0), color: th.text_secondary, line_height: Some(line_h), ..Default::default() });
    job.wrap.max_width = w;
    job.halign = Align::Center;
    let dg = c.p.layout_job(job);

    let name_h = c.measure("Shobdo Calok", fb(28.0)).y;
    let status_h = if app.about.status.is_empty() { 0.0 } else { c.measure("Ag", f(11.0)).y };
    let footer_h = c.measure("Ag", f(11.0)).y;
    let n_items = if status_h > 0.0 { 9.0 } else { 8.0 };
    let total = 100.0 + name_h + 26.0 + 1.0 + dg.size().y + 1.0 + 38.0 + status_h + footer_h + (n_items - 1.0) * 20.0;
    let cx = page.center().x;
    let mut y = page.center().y - total / 2.0;

    // Animated logo
    let logo = rect_xywh(cx - 50.0, y, 100.0, 100.0);
    let ring_op = anim::ping_pong(&ctx, 0.2, 0.8, 1200.0, Ease::InOutSine);
    let ring_sc = anim::ping_pong(&ctx, 0.95, 1.12, 1200.0, Ease::InOutSine);
    c.faded(ring_op).circle_stroke(logo.center(), 50.0 * ring_sc, 2.0 * ring_sc, th.primary);
    let float = anim::ping_pong(&ctx, 6.0, -6.0, 1800.0, Ease::InOutSine);
    let ib = Rect::from_center_size(logo.center() + Vec2::new(0.0, float), Vec2::splat(72.0));
    c.rect(ib, 22.0, th.primary);
    let rot = anim::looping(&ctx, -8.0, 8.0, 2400.0, Ease::InOutSine);
    c.icon_rot(ib.center(), "bolt", 36.0, Color32::WHITE, rot);
    y += 100.0 + 20.0;

    let r = c.text(Pos2::new(cx, y), Align2::CENTER_TOP, "Shobdo Calok", fb(28.0), th.text_primary);
    y = r.bottom() + 20.0;

    let vt = format!("Version {}", app.updater.current_version());
    let vw = c.measure(&vt, fb(12.0)).x + 24.0;
    let vr = rect_xywh(cx - vw / 2.0, y, vw, 26.0);
    c.rect(vr, 13.0, th.primary_light);
    c.text(vr.center(), Align2::CENTER_CENTER, &vt, fb(12.0), th.primary);
    y += 26.0 + 20.0;

    c.faded(0.2).hline(rect_xywh(cx - 100.0, y, 200.0, 1.0), th.text_secondary);
    y += 1.0 + 20.0;

    let dsize = dg.size();
    drop(dg);
    c.text_wrap_center(cx, y, w, desc, f(13.0), th.text_secondary, Some(line_h));
    y += dsize.y + 20.0;

    c.faded(0.2).hline(rect_xywh(cx - 100.0, y, 200.0, 1.0), th.text_secondary);
    y += 1.0 + 20.0;

    // Check for Updates button
    let br = rect_xywh(cx - 130.0, y, 260.0, 38.0);
    let enabled = !st.downloading && !st.applying && !st.checking;
    let resp = ui.interact(br, Id::new("about-update"), if enabled { Sense::CLICK } else { Sense::hover() });
    let bg = if resp.is_pointer_button_down_on() && enabled {
        th.primary_dark
    } else if resp.hovered() && enabled {
        th.primary_light
    } else if th.is_dark {
        theme::hex(0x1a1d23)
    } else {
        theme::hex(0xf1f5f9)
    };
    c.rect_fs(br, 12.0, bg, 1.0, th.primary);
    if st.downloading || st.applying {
        let pw = anim::anim(&ctx, Id::new("about-prog"), br.width() * st.progress, 100.0, Ease::Linear);
        c.clipped(br).faded(0.2).rect(rect_xywh(br.left(), br.top(), pw, br.height()), 12.0, if st.applying { theme::GREEN } else { th.primary });
    }
    let label = if st.applying {
        "Applying…".to_string()
    } else if st.downloading {
        format!("Downloading… {}%", (st.progress * 100.0).round() as i32)
    } else if st.checking {
        "Checking…".into()
    } else if st.update_available && st.download_ready {
        format!("Install v{}", st.latest_version)
    } else if st.update_available {
        format!("Update v{} (you have {})", st.latest_version, app.updater.current_version())
    } else {
        "Check for Updates".into()
    };
    let iw = crate::widgets::icon_text_size(&c, "system_update", 18.0).x;
    let lw = c.measure(&label, fb(12.0)).x;
    let lx = br.center().x - (iw + 8.0 + lw) / 2.0;
    c.icon_left(lx, br.center().y, "system_update", 18.0, th.primary);
    c.text(Pos2::new(lx + iw + 8.0, br.center().y), Align2::LEFT_CENTER, &label, fb(12.0), th.primary);
    if resp.clicked() && enabled {
        if st.update_available {
            app.updater.download_and_install();
        } else {
            app.updater.check_for_update();
        }
    }
    y += 38.0 + 20.0;

    if !app.about.status.is_empty() {
        let r = c.text(Pos2::new(cx, y), Align2::CENTER_TOP, &app.about.status, f(11.0), app.about.status_color.unwrap_or(th.text_secondary));
        y = r.bottom() + 20.0;
    }

    c.faded(0.6).text(Pos2::new(cx, y), Align2::CENTER_TOP, "© 2026 Shobdo Calok. All rights reserved.", f(11.0), th.text_secondary);

    // ── Applying overlay ───────────────────────────────────────────────
    if st.applying {
        c.rect(page, 0.0, if th.is_dark { theme::argb(0xcc0d0f12) } else { theme::argb(0xccf8fafc) });
        let _block = ui.interact(page, Id::new("about-applying-block"), Sense::CLICK);
        let msg_g = c.p.layout(st.status_message_text.clone(), f(13.0), th.text_secondary, 320.0);
        let title_h = c.measure("Applying Update", fb(18.0)).y;
        let pct_h = c.measure("0%", fb(12.0)).y;
        let total = 64.0 + title_h + msg_g.size().y + 6.0 + pct_h + 4.0 * 20.0;
        let mut y = page.center().y - total / 2.0;
        let sp = rect_xywh(page.center().x - 32.0, y, 64.0, 64.0);
        let ps = anim::ping_pong(&ctx, 0.95, 1.1, 800.0, Ease::InOutSine);
        c.faded(0.15).circle(sp.center(), 32.0 * ps, theme::GREEN);
        let spin = anim::looping(&ctx, 0.0, 360.0, 2000.0, Ease::Linear);
        c.icon_rot(sp.center(), "system_update", 32.0, theme::GREEN, spin);
        y += 64.0 + 20.0;
        let r = c.text(Pos2::new(page.center().x, y), Align2::CENTER_TOP, "Applying Update", fb(18.0), th.text_primary);
        y = r.bottom() + 20.0;
        let mh = msg_g.size().y;
        c.text_wrap_center(page.center().x, y, 320.0, &st.status_message_text, f(13.0), th.text_secondary, None);
        y += mh + 20.0;
        let pb = rect_xywh(page.center().x - 130.0, y, 260.0, 6.0);
        c.rect(pb, 3.0, if th.is_dark { theme::hex(0x1e293b) } else { theme::hex(0xe2e8f0) });
        let pw = anim::anim(&ctx, Id::new("about-apply-prog"), 260.0 * st.progress, 200.0, Ease::Linear);
        c.rect(rect_xywh(pb.left(), pb.top(), pw, 6.0), 3.0, theme::GREEN);
        y += 6.0 + 20.0;
        c.text(Pos2::new(page.center().x, y), Align2::CENTER_TOP, &format!("{}%", (st.progress * 100.0).round() as i32), fb(12.0), theme::GREEN);
    }
}
