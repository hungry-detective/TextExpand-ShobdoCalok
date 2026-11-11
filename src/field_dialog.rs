//! Port of viewmodels/field_dialog.py — the "Complete your form" dialog shown
//! for `{field:Label}` tokens.
//!
//! The expansion engine runs on background threads while the main window is
//! usually hidden in the tray, so the dialog is shown by re-launching this exe
//! with `--field-dialog`; labels/content go in on stdin and the values come
//! back as a JSON array on stdout (exit code 1 = cancelled).

use egui::{Align2, Color32, CursorIcon, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use crate::draw::{rect_xywh, Canvas};
use crate::fonts::{fb, fi, fm, icon_font};
use crate::theme::hex;
use crate::win;

// ── Parent side ─────────────────────────────────────────────────────────────

/// Shows the dialog and blocks until the user answers. Restores keyboard focus
/// to the app the user was typing in before returning (like main.py did).
pub fn request(labels: &[String], content: &str, is_dark: bool) -> Option<Vec<String>> {
    let fg = win::capture_foreground();
    let exe = std::env::current_exe().ok()?;
    let payload = serde_json::json!({ "labels": labels, "content": content }).to_string();
    let child = Command::new(exe)
        .arg("--field-dialog")
        .arg(if is_dark { "dark" } else { "light" })
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let result = match child {
        Ok(mut ch) => {
            if let Some(mut si) = ch.stdin.take() {
                let _ = si.write_all(payload.as_bytes());
            }
            match ch.wait_with_output() {
                Ok(out) if out.status.success() => serde_json::from_slice::<Vec<String>>(&out.stdout).ok(),
                _ => None,
            }
        }
        Err(_) => None,
    };
    // Restore focus to the app the user was typing in BEFORE the paste runs.
    win::restore_foreground(fg);
    std::thread::sleep(std::time::Duration::from_millis(120));
    result
}

// ── Child side (the dialog window) ──────────────────────────────────────────

struct Colors {
    bg: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    accent: Color32,
    accent_hover: Color32,
    title: Color32,
    close_hover: Color32,
    input_border: Color32,
    input_bg: Color32,
    input_focus_bg: Color32,
    input_text: Color32,
}

impl Colors {
    fn new(dark: bool) -> Self {
        let p = |d: u32, l: u32| if dark { hex(d) } else { hex(l) };
        Self {
            bg: p(0x1e293b, 0xf8fafc),
            text_primary: p(0xf1f5f9, 0x0f172a),
            text_secondary: p(0x94a3b8, 0x64748b),
            accent: p(0xa78bfa, 0x7c3aed),
            accent_hover: p(0xc4b5fd, 0x8b5cf6),
            title: p(0xffffff, 0x0f172a),
            close_hover: p(0x334155, 0xe2e8f0),
            input_border: p(0x475569, 0xcbd5e1),
            input_bg: p(0x1e293b, 0xffffff),
            input_focus_bg: p(0x0f172a, 0xf8fafc),
            input_text: p(0xffffff, 0x0f172a),
        }
    }
}

enum Row {
    Field { label: String, index: usize },
    Context(String),
    Spacer,
}

fn parse_rows(content: &str) -> (Vec<Row>, usize) {
    let re = &crate::engine::RE_FIELD;
    let mut rows = vec![];
    let mut count = 0;
    for line in content.split('\n') {
        let mut has_field = false;
        let mut last = 0;
        for m in re.find_iter(line) {
            has_field = true;
            // display_text = parts[i-1].strip() — the text right before this token
            let before = line[last..m.start()].trim().to_string();
            rows.push(Row::Field { label: before, index: count });
            count += 1;
            last = m.end();
        }
        if !has_field {
            if line.trim().is_empty() {
                rows.push(Row::Spacer);
            } else {
                rows.push(Row::Context(line.trim().to_string()));
            }
        }
    }
    (rows, count)
}

struct DialogApp {
    colors: Colors,
    rows: Vec<Row>,
    values: Vec<String>,
    hwnd: isize,
    frame: u32,
    focus_first: bool,
}

impl DialogApp {
    fn finish(&self, accepted: bool) -> ! {
        if accepted {
            let out = serde_json::to_string(&self.values).unwrap_or_else(|_| "[]".into());
            let mut so = std::io::stdout();
            let _ = so.write_all(out.as_bytes());
            let _ = so.flush();
            std::process::exit(0);
        }
        std::process::exit(1);
    }
}

impl eframe::App for DialogApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let col = &self.colors;
        let screen = ctx.content_rect();
        let c = Canvas::new(ui.painter());

        // ── Measure (QGridLayout + adjustSize) ──────────────────────────
        let title_sz = c.measure("Complete your form", fb(15.0));
        let mut label_w: f32 = 0.0;
        let mut grid_h = 0.0;
        let mut ctx_w: f32 = 0.0;
        for (i, r) in self.rows.iter().enumerate() {
            if i > 0 {
                grid_h += 10.0;
            }
            match r {
                Row::Field { label, .. } => {
                    label_w = label_w.max(c.measure(label, fm(13.0)).x);
                    grid_h += 28.0;
                }
                Row::Context(t) => {
                    let s = c.measure(t, fi(11.0));
                    ctx_w = ctx_w.max(s.x);
                    grid_h += s.y;
                }
                Row::Spacer => grid_h += 2.0,
            }
        }
        let grid_w = (label_w + 10.0 + 180.0).max(ctx_w);
        let btn_h = 10.0 * 2.0 + c.measure("Insert Snippet", fb(13.0)).y;
        let w = 40.0 + title_sz.x.max(grid_w).max(180.0);
        let h = 20.0 + title_sz.y + 12.0 + grid_h + 12.0 + 5.0 + btn_h + 20.0;

        if self.frame == 0 {
            // Size to content and centre on the primary screen, then take focus.
            let hwnd = win::hwnd_from(self.hwnd);
            if !hwnd.is_null() {
                let s = win::dpi_scale(hwnd);
                let wa = win::primary_work_area();
                let pw = (w * s).round() as i32;
                let ph = (h * s).round() as i32;
                let x = wa.left + ((wa.right - wa.left) - pw) / 2;
                let y = wa.top + ((wa.bottom - wa.top) - ph) / 2;
                unsafe {
                    use windows_sys::Win32::UI::WindowsAndMessaging::*;
                    SetWindowPos(hwnd, HWND_TOPMOST, x, y, pw, ph, SWP_SHOWWINDOW);
                }
                win::force_foreground(hwnd);
            }
            ctx.request_repaint();
        }
        self.frame += 1;

        let r = Rect::from_min_size(screen.min, Vec2::new(w, h));
        // Draggable background
        let bg = ui.interact(r, Id::new("fd-bg"), (Sense::CLICK | Sense::DRAG));
        if bg.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        c.rect(r, 16.0, col.bg);

        let mut y = r.top() + 20.0;
        c.text(Pos2::new(r.center().x, y), Align2::CENTER_TOP, "Complete your form", fb(15.0), col.title);
        y += title_sz.y + 12.0;

        // Grid
        let gx = r.left() + 20.0;
        let mut submit = false;
        let mut first_id = None;
        for (i, row) in self.rows.iter().enumerate() {
            if i > 0 {
                y += 10.0;
            }
            match row {
                Row::Field { label, index } => {
                    let er = rect_xywh(gx + label_w + 10.0, y, 180.0, 28.0);
                    c.text(Pos2::new(gx + label_w, er.center().y), Align2::RIGHT_CENTER, label, fm(13.0), col.input_text);
                    let id = Id::new(("fd-edit", *index));
                    if first_id.is_none() {
                        first_id = Some(id);
                    }
                    let focused = ctx.memory(|m| m.has_focus(id));
                    c.rect_fs(
                        er,
                        8.0,
                        if focused { col.input_focus_bg } else { col.input_bg },
                        1.0,
                        if focused { col.accent } else { col.input_border },
                    );
                    let inner = Rect::from_min_max(Pos2::new(er.left() + 11.0, er.top() + 5.0), Pos2::new(er.right() - 11.0, er.bottom() - 5.0));
                    let val = &mut self.values[*index];
                    let out = crate::widgets::line_edit(ui, id, inner, val, crate::fonts::f(13.0), col.input_text, Some(("Name", crate::fonts::f(13.0), col.text_secondary, false)));
                    if out.submitted {
                        submit = true;
                    }
                    y += 28.0;
                }
                Row::Context(t) => {
                    let rr = c.text(Pos2::new(gx, y), Align2::LEFT_TOP, t, fi(11.0), col.text_secondary);
                    y += rr.height();
                }
                Row::Spacer => y += 2.0,
            }
        }
        if self.focus_first {
            if let Some(id) = first_id {
                ctx.memory_mut(|m| m.request_focus(id));
            }
            self.focus_first = false;
        }
        y += 12.0 + 5.0;

        // Insert Snippet button
        let br = rect_xywh(r.center().x - 90.0, y, 180.0, btn_h);
        let bresp = ui.interact(br, Id::new("fd-ok"), Sense::CLICK).on_hover_cursor(CursorIcon::PointingHand);
        c.rect(br, 10.0, if bresp.hovered() { col.accent_hover } else { col.accent });
        c.text(br.center(), Align2::CENTER_CENTER, "Insert Snippet", fb(13.0), Color32::WHITE);
        if bresp.clicked() {
            submit = true;
        }

        // Close button (top-right)
        let cr = rect_xywh(r.right() - 34.0, r.top() + 10.0, 30.0, 30.0);
        let cresp = ui.interact(cr, Id::new("fd-close"), Sense::CLICK).on_hover_cursor(CursorIcon::PointingHand);
        if cresp.hovered() {
            c.rect(cr, 6.0, col.close_hover);
        }
        c.text(cr.center() - Vec2::new(0.0, 1.0), Align2::CENTER_CENTER, &crate::fonts::icon("close"), icon_font(16.0), if cresp.hovered() { col.text_primary } else { col.text_secondary });

        if submit || (self.rows.iter().all(|r| !matches!(r, Row::Field { .. })) && ctx.input(|i| i.key_pressed(egui::Key::Enter))) {
            self.finish(true);
        }
        if cresp.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) || ctx.input(|i| i.viewport().close_requested()) {
            self.finish(false);
        }
    }
}

pub fn run_child(dark: bool) {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let v: serde_json::Value = serde_json::from_str(&input).unwrap_or_default();
    let content = v.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
    let (rows, count) = parse_rows(&content);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Shobdo Calok — Fill In Fields")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_position([-32000.0, -32000.0])
            .with_inner_size([420.0, 260.0])
            .with_icon(crate::svg::icon_data(crate::svg::APP_ICON, 64)),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Shobdo Calok — Fill In Fields",
        options,
        Box::new(move |cc| {
            crate::fonts::install(&cc.egui_ctx);
            cc.egui_ctx.all_styles_mut(|s| {
                s.visuals = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
                s.visuals.selection.bg_fill = if dark { hex(0xa78bfa) } else { hex(0x7c3aed) };
                s.visuals.text_cursor.stroke = egui::Stroke::new(1.0, if dark { Color32::WHITE } else { hex(0x0f172a) });
            });
            let mut hwnd = 0isize;
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(h) = cc.window_handle() {
                if let RawWindowHandle::Win32(w) = h.as_raw() {
                    hwnd = w.hwnd.get();
                }
            }
            Ok(Box::new(DialogApp { colors: Colors::new(dark), rows, values: vec![String::new(); count], hwnd, frame: 0, focus_first: true }))
        }),
    );
    std::process::exit(1);
}
