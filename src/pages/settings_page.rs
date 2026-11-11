//! Port of Components/SettingsPage.qml

use egui::{Align2, Color32, Id, Pos2, Rect, Sense, Ui, Vec2};

use crate::anim;
use crate::app::{Action, App};
use crate::draw::{rect_xywh, Canvas};
use crate::fonts::{f, fb};
use crate::theme::{self};
use crate::widgets::{self, RowSpec, Toast};

#[derive(Default)]
pub struct SettingsState {
    pub toast: Toast,
    scroll: f32,
}

/// Flickable: wheel-scroll offset clamped to the content height.
pub fn flick(ui: &Ui, page: Rect, scroll: &mut f32, content_h: f32) {
    let max = (content_h - page.height()).max(0.0);
    if ui.rect_contains_pointer(page) {
        *scroll -= ui.ctx().input(|i| i.smooth_scroll_delta.y);
    }
    *scroll = scroll.clamp(0.0, max);
}

pub fn show(app: &mut App, ui: &mut Ui, page: Rect) {
    let th = app.th;
    let ctx = ui.ctx().clone();
    let c = Canvas::new(ui.painter());

    // Background subtle decoration
    c.faded(0.03).circle(Pos2::new(page.right() + 60.0 - 140.0, page.bottom() + 60.0 - 140.0), 140.0, th.primary);

    let st = app.updater.state();
    let x0 = page.left() + 32.0;
    let w = page.width() - 64.0;
    // content height: header 29+4+16, divider 41, sections, spacer 40, footer 15
    let progress_visible = st.downloading || st.applying;
    let sec = |rows: f32, extra: f32| 15.0 + 12.0 + rows * 60.0 + (rows - 1.0).max(0.0) * 12.0 + extra;
    let content_h = 29.3 + 4.0 + 16.0 + 41.0 + sec(1.0, 0.0) + 24.0 + sec(1.0, 0.0) + 24.0
        + sec(1.0, if progress_visible { 12.0 + 20.0 } else { 0.0 })
        + 40.0
        + 15.0
        + 64.0;
    flick(ui, page, &mut app.settings_ui.scroll, content_h);
    let mut y = page.top() - app.settings_ui.scroll;

    // ── Page header ────────────────────────────────────────────────────
    let t = c.text(Pos2::new(x0, y), Align2::LEFT_TOP, "Settings", fb(22.0), th.text_primary);
    y = t.bottom() + 4.0;
    let t = c.text(Pos2::new(x0, y), Align2::LEFT_TOP, "Configure your Shobdo Calok preferences", f(12.0), th.text_secondary);
    y = t.bottom() + 20.0;
    c.faded(0.15).hline(rect_xywh(x0, y, w, 1.0), th.text_secondary);
    y += 1.0 + 20.0;

    // ── SYSTEM ─────────────────────────────────────────────────────────
    y += widgets::section_label(&c, x0, y, "SYSTEM", th.primary) + 12.0;
    let row = rect_xywh(x0, y, w, 60.0);
    if let Some(v) = widgets::settings_row(
        ui,
        &th,
        Id::new("set-startup"),
        row,
        &RowSpec {
            icon: "rocket_launch",
            title: "Run at Startup",
            description: "Launch automatically when Windows starts",
            description_color: None,
            active_color: th.primary,
            checked: app.startup_enabled,
            is_button: false,
            button_text: "",
            button_enabled: true,
        },
    ) {
        app.actions.push(Action::SetStartup(v));
    }
    y += 60.0 + 24.0;

    // ── APPEARANCE ─────────────────────────────────────────────────────
    y += widgets::section_label(&c, x0, y, "APPEARANCE", th.text_secondary) + 12.0;
    let row = rect_xywh(x0, y, w, 60.0);
    c.rect(row, 14.0, th.surface);
    let ir = rect_xywh(row.left() + 12.0, row.center().y - 16.0, 32.0, 32.0);
    c.rect(ir, 8.0, th.primary_light);
    c.icon(
        ir.center(),
        if th.is_dark { "dark_mode" } else { "light_mode" },
        18.0,
        if th.is_dark { theme::hex(0x818cf8) } else { theme::hex(0xf59e0b) },
    );
    let modes = ["system", "dark", "light"];
    let labels = ["System", "Dark", "Light"];
    let seg_w = 52.0 * 3.0 + 4.0 * 2.0;
    let seg_x = row.right() - 12.0 - seg_w;
    for k in 0..3 {
        let sr = rect_xywh(seg_x + k as f32 * 56.0, row.center().y - 14.0, 52.0, 28.0);
        let selected = app.theme_mode == modes[k];
        let col = anim::anim_color(&ctx, Id::new(("seg", k)), if selected { th.primary } else { th.hover_bg }, 150.0);
        c.rect(sr, 8.0, col);
        c.text(sr.center(), Align2::CENTER_CENTER, labels[k], fb(10.0), if selected { Color32::WHITE } else { th.text_secondary });
        if ui.interact(sr, Id::new(("seg-tap", k)), Sense::CLICK).clicked() {
            app.actions.push(Action::SetThemeMode(modes[k].into()));
        }
    }
    let tw = seg_x - 12.0 - (row.left() + 48.0);
    let desc = match app.theme_mode.as_str() {
        "system" => "Follows Windows (dark/light)",
        "dark" => "Dark Mode is active",
        _ => "Light Mode is active",
    };
    let tg = crate::draw::layout_elide(&c.p, "Theme", fb(13.0), th.text_primary, tw);
    let dg = crate::draw::layout_elide(&c.p, desc, f(11.0), th.text_secondary, tw);
    let top = row.center().y - (tg.size().y + 1.0 + dg.size().y) / 2.0;
    let th_h = tg.size().y;
    c.galley(Pos2::new(row.left() + 48.0, top), tg, th.text_primary);
    c.galley(Pos2::new(row.left() + 48.0, top + th_h + 1.0), dg, th.text_secondary);
    y += 60.0 + 24.0;

    // ── UPDATE ─────────────────────────────────────────────────────────
    y += widgets::section_label(&c, x0, y, "UPDATE", th.text_secondary) + 12.0;
    let row = rect_xywh(x0, y, w, 60.0);
    let desc = if st.update_available {
        format!("Version {} is available", st.latest_version)
    } else {
        format!("Installed: v{}", app.updater.current_version())
    };
    let btn = if st.applying {
        "Applying…".to_string()
    } else if st.downloading {
        format!("{}%", (st.progress * 100.0).round() as i32)
    } else if st.update_available {
        if st.download_ready { "Install".into() } else { "Download".into() }
    } else {
        "Check".into()
    };
    let enabled = !st.downloading && !st.checking && !st.applying;
    if widgets::settings_row(
        ui,
        &th,
        Id::new("set-update"),
        row,
        &RowSpec {
            icon: "system_update",
            title: "Check for Updates",
            description: &desc,
            description_color: Some(if st.update_available { theme::GREEN } else { th.text_secondary }),
            active_color: theme::GREEN,
            checked: st.update_available,
            is_button: true,
            button_text: &btn,
            button_enabled: enabled,
        },
    )
    .is_some()
    {
        if st.update_available {
            app.updater.download_and_install();
        } else {
            app.updater.check_for_update();
        }
    }
    y += 60.0;
    if progress_visible {
        y += 12.0;
        let pb = rect_xywh(x0, y, w, 20.0);
        c.rect(pb, 10.0, if th.is_dark { theme::hex(0x1a1d23) } else { theme::hex(0xf1f5f9) });
        let pw = anim::anim(&ctx, Id::new("set-progress-w"), pb.width() * st.progress, 100.0, anim::Ease::Linear);
        let cc = c.clipped(pb);
        cc.rect(rect_xywh(pb.left(), pb.top(), pw, 20.0), 10.0, if st.applying { theme::GREEN } else { th.primary });
        let label = if st.applying {
            "Applying update…".to_string()
        } else if !st.status_message_text.is_empty() {
            st.status_message_text.clone()
        } else {
            format!("Downloading… {}%", (st.progress * 100.0).round() as i32)
        };
        c.text(pb.center(), Align2::CENTER_CENTER, &label, fb(10.0), th.text_primary);
        y += 20.0;
    }

    y += 40.0;
    c.faded(0.5).text(
        Pos2::new(x0 + w / 2.0, y),
        Align2::CENTER_TOP,
        &format!("Shobdo Calok  v{}", app.updater.current_version()),
        f(11.0),
        th.text_secondary,
    );

    app.settings_ui.toast.paint(ui, &th, Id::new("settings-toast"), page);
    let _ = Vec2::ZERO;
}
