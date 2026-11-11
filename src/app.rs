//! Port of main.qml + TitleBar.qml + Sidebar.qml + AppMenu.qml and the
//! SnippetViewModel slots (viewmodels/main.py).

use egui::{Align2, Color32, Id, LayerId, Order, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};
use parking_lot::Mutex;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::anim::{self, Ease};
use crate::draw::{rect_xywh, Canvas};
use crate::drive::Drive;
use crate::engine::Engine;
use crate::events::Event;
use crate::fonts::{f, fb};
use crate::pages::{about, backup, editor, library, settings_page, tutorial};
use crate::settings::Settings;
use crate::store::SnippetStore;
use crate::svg::{self, SvgCache};
use crate::theme::{self, Theme};
use crate::updater::Updater;
use crate::widgets::{self, TipPos};
use crate::win;

pub enum Action {
    AddFolder(String),
    RenameFolder(String, String),
    DeleteFolder(String),
    ReorderFolders(Vec<String>),
    ReorderSnippets(String, Vec<String>),
    SelectSnippet(String, String),
    AddSnippetAndSelect(String, String),
    DeleteSnippet(String, String),
    DuplicateSnippet(String, String),
    MoveSnippet(String, String, String),
    EditorSave,
    EditorDelete,
    SetThemeMode(String),
    SetStartup(bool),
    Export,
    Import { toast: bool },
    ToggleAll,
}

pub struct App {
    pub th: Theme,
    pub store: Arc<Mutex<SnippetStore>>,
    pub engine: Arc<Engine>,
    pub updater: Arc<Updater>,
    pub drive: Arc<Drive>,
    pub settings: Settings,
    pub theme_mode: String,
    pub is_dark: bool,
    pub startup_enabled: bool,
    rx: Receiver<Event>,
    /// Sidebar activeIndex (0 Tutorial, 1 Editor & Library, 2 About, 3 Backup, 4 Settings)
    pub page: usize,
    pub svg: SvgCache,
    pub actions: Vec<Action>,
    menu_open: Option<usize>,
    menu_rect: Option<Rect>,
    last_title_press: f64,
    start_time: Option<f64>,
    pub library: library::LibraryState,
    pub editor: editor::EditorState,
    pub settings_ui: settings_page::SettingsState,
    pub backup: backup::BackupState,
    pub about: about::AboutState,
    pub tutorial: tutorial::TutorialState,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: &egui::Context,
        store: Arc<Mutex<SnippetStore>>,
        engine: Arc<Engine>,
        updater: Arc<Updater>,
        drive: Arc<Drive>,
        settings: Settings,
        theme_mode: String,
        is_dark: bool,
        rx: Receiver<Event>,
    ) -> Self {
        let mut app = Self {
            th: Theme::new(is_dark),
            store,
            engine,
            updater,
            drive,
            settings,
            theme_mode,
            is_dark,
            startup_enabled: win::startup_enabled(),
            rx,
            // SHOBDO_PAGE is a developer aid for screenshots; normal start is page 1.
            page: std::env::var("SHOBDO_PAGE").ok().and_then(|v| v.parse().ok()).filter(|p| *p <= 4).unwrap_or(1),
            svg: SvgCache::default(),
            actions: vec![],
            menu_open: None,
            menu_rect: None,
            last_title_press: -10.0,
            start_time: None,
            library: Default::default(),
            editor: Default::default(),
            settings_ui: Default::default(),
            backup: Default::default(),
            about: Default::default(),
            tutorial: Default::default(),
        };
        app.apply_style(ctx);
        app.folders_changed();
        // Developer aid for screenshots: pre-select the first snippet.
        if std::env::var_os("SHOBDO_SELECT_FIRST").is_some() {
            let first = app.store.lock().folders().iter().find(|f| !f.snippets.is_empty()).map(|f| (f.name.clone(), f.snippets[0].abbreviation.clone()));
            if let Some((fo, ab)) = first {
                app.library.selected_folder = fo.clone();
                app.library.selected_abbrev = ab.clone();
                app.library.set_expanded(&fo, true);
                app.select_snippet(&fo, &ab);
            }
        }
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        app
    }

    fn apply_style(&self, ctx: &egui::Context) {
        let th = self.th;
        ctx.all_styles_mut(|s| {
            s.visuals = if th.is_dark { egui::Visuals::dark() } else { egui::Visuals::light() };
            s.visuals.selection.bg_fill = th.primary;
            s.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
            s.visuals.text_cursor.stroke = egui::Stroke::new(1.0, th.text_primary);
            s.visuals.text_cursor.blink = true;
            s.visuals.panel_fill = th.background;
            s.visuals.window_fill = th.surface;
            s.visuals.override_text_color = None;
            s.visuals.widgets.inactive.fg_stroke.color = th.text_secondary;
            s.visuals.widgets.hovered.fg_stroke.color = th.primary;
            s.visuals.widgets.active.fg_stroke.color = th.primary;
            s.visuals.extreme_bg_color = Color32::TRANSPARENT;
            s.spacing.scroll = egui::style::ScrollStyle {
                floating: true,
                bar_width: 8.0,
                floating_width: 4.0,
                floating_allocated_width: 0.0,
                bar_inner_margin: 4.0,
                bar_outer_margin: 0.0,
                foreground_color: true,
                dormant_background_opacity: 0.0,
                active_background_opacity: 0.0,
                interact_background_opacity: 0.0,
                dormant_handle_opacity: 0.4,
                active_handle_opacity: 0.4,
                interact_handle_opacity: 0.8,
                ..egui::style::ScrollStyle::floating()
            };
            s.interaction.selectable_labels = false;
        });
        if let Some(h) = Some(win::main_hwnd()).filter(|h| !h.is_null()) {
            win::set_dark_titlebar(h, th.is_dark);
        }
    }

    // ── SnippetViewModel signals ────────────────────────────────────────────

    pub fn folders_changed(&mut self) {
        let store = self.store.lock();
        self.library.rebuild_all(&store);
        self.editor.folder_names = store.folder_names();
    }

    pub fn snippets_changed(&mut self, folder: &str) {
        let store = self.store.lock();
        self.library.refresh_folder(&store, folder);
    }

    fn select_snippet(&mut self, folder: &str, abbrev: &str) {
        let sn = self.store.lock().get_snippet(folder, abbrev);
        if let Some(sn) = sn {
            self.editor.on_snippet_selected(folder, &sn.abbreviation, &sn.content);
        }
    }

    pub fn set_theme_mode(&mut self, ctx: &egui::Context, mode: &str) {
        if !["system", "dark", "light"].contains(&mode) {
            return;
        }
        if mode != self.theme_mode {
            self.theme_mode = mode.to_string();
            self.settings.set_value("appearance/themeMode", mode);
        }
        let dark = match mode {
            "dark" => true,
            "light" => false,
            _ => win::system_is_dark(),
        };
        if dark != self.is_dark {
            self.is_dark = dark;
            self.th = Theme::new(dark);
            self.engine.is_dark.store(dark, std::sync::atomic::Ordering::SeqCst);
            crate::tray::set_dark(dark);
            self.apply_style(ctx);
        }
    }

    fn process_actions(&mut self, ctx: &egui::Context) {
        let actions = std::mem::take(&mut self.actions);
        for a in actions {
            match a {
                Action::AddFolder(name) => {
                    let ok = self.store.lock().add_folder(name.trim());
                    if ok {
                        self.folders_changed();
                    }
                }
                Action::RenameFolder(old, new) => {
                    let new = new.trim().to_string();
                    if new.is_empty() || new == old {
                        continue;
                    }
                    let ok = self.store.lock().rename_folder(&old, &new);
                    if ok {
                        self.folders_changed();
                    }
                }
                Action::DeleteFolder(name) => {
                    let ok = self.store.lock().delete_folder(&name);
                    if ok {
                        self.folders_changed();
                    }
                }
                Action::ReorderFolders(names) => self.store.lock().reorder_folders(&names),
                Action::ReorderSnippets(folder, names) => self.store.lock().reorder_snippets(&folder, &names),
                Action::SelectSnippet(folder, abbrev) => self.select_snippet(&folder, &abbrev),
                Action::AddSnippetAndSelect(folder, abbrev) => {
                    let ok = self.store.lock().add_snippet(&folder, abbrev.trim(), "");
                    if ok {
                        self.snippets_changed(&folder);
                        self.engine.refresh_store();
                        self.library.set_expanded(&folder, true);
                        self.library.selected_folder = folder.clone();
                        self.library.selected_abbrev = abbrev.clone();
                        self.select_snippet(&folder, &abbrev);
                    }
                }
                Action::DeleteSnippet(folder, abbrev) => {
                    let ok = self.store.lock().delete_snippet(&folder, &abbrev);
                    if ok {
                        self.snippets_changed(&folder);
                        self.engine.refresh_store();
                    }
                }
                Action::DuplicateSnippet(folder, abbrev) => {
                    let mut store = self.store.lock();
                    let Some(sn) = store.get_snippet(&folder, &abbrev) else { continue };
                    let existing = store.abbreviations_map().clone();
                    let mut new_abbrev = format!("{abbrev}_copy");
                    let mut counter = 1;
                    while existing.contains_key(&new_abbrev) {
                        counter += 1;
                        new_abbrev = format!("{abbrev}_copy{counter}");
                    }
                    let ok = store.add_snippet(&folder, &new_abbrev, &sn.content);
                    drop(store);
                    if ok {
                        self.snippets_changed(&folder);
                        self.engine.refresh_store();
                    }
                }
                Action::MoveSnippet(abbrev, from, to) => {
                    let ok = self.store.lock().move_snippet(&abbrev, &from, &to);
                    if ok {
                        self.snippets_changed(&from);
                        self.snippets_changed(&to);
                        self.engine.refresh_store();
                    }
                }
                Action::EditorSave => self.editor_save(ctx),
                Action::EditorDelete => {
                    let (folder, abbrev) = (self.editor.current_folder.clone(), self.editor.current_abbrev.clone());
                    if !folder.is_empty() && !abbrev.is_empty() {
                        let ok = self.store.lock().delete_snippet(&folder, &abbrev);
                        if ok {
                            self.snippets_changed(&folder);
                            self.engine.refresh_store();
                        }
                        self.editor.clear();
                    }
                }
                Action::SetThemeMode(m) => self.set_theme_mode(ctx, &m),
                Action::SetStartup(v) => {
                    if win::set_startup(v) {
                        self.startup_enabled = v;
                    }
                }
                Action::Export => {
                    let r = self.export_snippets();
                    if !r.is_empty() {
                        self.backup.toast.show(ctx, &r);
                    }
                }
                Action::Import { toast } => {
                    let r = self.import_snippets();
                    if toast && !r.is_empty() {
                        self.backup.toast.show(ctx, &r);
                    }
                }
                Action::ToggleAll => self.library.toggle_all(),
            }
        }
    }

    /// EditorPanel.doSave()
    fn editor_save(&mut self, ctx: &egui::Context) {
        let ed = &self.editor;
        if ed.current_abbrev.is_empty() {
            return;
        }
        let final_abbrev = ed.abbrev_input.trim().to_string();
        let final_content = ed.text.clone();
        let mut current_folder = ed.current_folder.clone();
        let current_abbrev = ed.current_abbrev.clone();
        let pending = ed.pending_folder.clone();

        // 1. If folder changed, move first
        if !pending.is_empty() && pending != current_folder {
            let moved = self.store.lock().move_snippet(&current_abbrev, &current_folder, &pending);
            if moved {
                self.snippets_changed(&current_folder);
                self.snippets_changed(&pending);
                self.engine.refresh_store();
                current_folder = pending.clone();
                self.editor.current_folder = pending.clone();
            }
        }
        // 2. Update snippet content/abbreviation
        let saved = self.store.lock().update_snippet(&current_folder, &current_abbrev, &final_abbrev, &final_content);
        if saved {
            self.snippets_changed(&current_folder);
            self.engine.refresh_store();
            self.editor.current_abbrev = final_abbrev;
            self.editor.current_content = final_content;
            self.editor.flash_saved(ctx);
        }
    }

    fn export_snippets(&self) -> String {
        let path = rfd::FileDialog::new()
            .set_title("Export Snippets")
            .set_directory(std::env::var("USERPROFILE").unwrap_or_default())
            .set_file_name("shobdocalok_backup.json")
            .add_filter("JSON", &["json"])
            .save_file();
        let Some(path) = path else { return String::new() };
        let json = self.store.lock().export_json();
        match std::fs::write(&path, json) {
            Ok(_) => "Export complete!".into(),
            Err(e) => format!("Error: {e}"),
        }
    }

    fn import_snippets(&mut self) -> String {
        let path = rfd::FileDialog::new().set_title("Import Snippets").add_filter("JSON", &["json"]).pick_file();
        let Some(path) = path else { return String::new() };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return format!("Error: {e}"),
        };
        let v: serde_json::Value = match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
            Ok(v) => v,
            Err(e) => return format!("Error: {e}"),
        };
        if !self.store.lock().import_value(&v) {
            return "Invalid file: missing 'folders'.".into();
        }
        self.after_full_reload();
        "Import complete!".into()
    }

    fn after_full_reload(&mut self) {
        self.folders_changed();
        let names = self.store.lock().folder_names();
        for n in names {
            self.snippets_changed(&n);
        }
        self.engine.refresh_store();
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::UpdaterStatus(msg) => {
                    self.settings_ui.toast.show(ctx, &msg);
                    self.about.on_status(ctx, &msg, &self.th);
                }
                Event::UpdateAvailableChanged => {
                    let st = self.updater.state();
                    if !st.update_available && !st.checking {
                        self.about.on_up_to_date(ctx);
                    }
                }
                Event::DriveStatus(msg) => {
                    self.settings_ui.toast.show(ctx, &msg);
                    self.backup.toast.show(ctx, &msg);
                }
                Event::DriveLoginChanged(b) => {
                    let m = if b { "Signed in" } else { "Signed out" };
                    self.settings_ui.toast.show(ctx, m);
                    self.backup.toast.show(ctx, m);
                }
                Event::DriveBackupList(list) => self.backup.on_backup_list(list),
                Event::DriveRestoreComplete => self.after_full_reload(),
                Event::ShowAboutPage => self.page = 2,
            }
        }
    }

    // ── Title bar ───────────────────────────────────────────────────────────

    fn title_bar(&mut self, ui: &mut Ui, r: Rect) {
        let th = self.th;
        let ctx = ui.ctx().clone();
        let c = Canvas::new(ui.painter());

        // Background drag area (registered first so children win)
        let bg = ui.interact(r, Id::new("titlebar-drag"), (Sense::CLICK | Sense::DRAG));
        if bg.hovered() && ctx.input(|i| i.pointer.primary_pressed()) {
            let now = ctx.input(|i| i.time);
            let dbl = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() } as f64 / 1000.0;
            if now - self.last_title_press < dbl {
                self.last_title_press = -10.0;
                win::toggle_maximize();
            } else {
                self.last_title_press = now;
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }

        // 1. App icon
        let icon = self.svg.get_raw(&ctx, "app-icon", svg::APP_ICON, 28.0, 28.0);
        c.image(&icon, rect_xywh(r.left() + 12.0, r.center().y - 14.0, 28.0, 28.0), Color32::WHITE);

        // 2. Menu bar
        let titles = ["File", "Tasks", "Tools", "Help"];
        let mut x = r.left() + 12.0 + 28.0 + 8.0;
        let mut item_rects = vec![];
        for (i, t) in titles.iter().enumerate() {
            let w = c.measure(t, f(12.0)).x + 28.0;
            let ir = rect_xywh(x, r.center().y - 16.0, w, 32.0);
            let resp = ui.interact(ir, Id::new(("menubar", i)), Sense::CLICK);
            let open = self.menu_open == Some(i);
            if resp.clicked() {
                self.menu_open = if open { None } else { Some(i) };
            } else if resp.hovered() && self.menu_open.is_some() && !open {
                self.menu_open = Some(i);
            }
            let hl = resp.hovered() || self.menu_open == Some(i);
            if hl {
                c.rect(ir, 4.0, th.hover_bg);
            }
            c.text(Pos2::new(ir.left() + 12.0, ir.center().y), Align2::LEFT_CENTER, t, f(12.0), if hl { th.primary } else { th.text_primary });
            item_rects.push(ir);
            x += w;
        }

        // 5. Window control buttons (right)
        let bw = 46.0;
        let close_r = rect_xywh(r.right() - bw, r.top(), bw, 48.0);
        let max_r = rect_xywh(r.right() - bw * 2.0, r.top(), bw, 48.0);
        let min_r = rect_xywh(r.right() - bw * 3.0, r.top(), bw, 48.0);
        let maximized = win::is_maximized();
        for (k, br) in [(0, min_r), (1, max_r), (2, close_r)] {
            let resp = ui.interact(br, Id::new(("winbtn", k)), Sense::CLICK);
            let (name, svgsrc): (&'static str, &str) = match k {
                0 => ("min", svg::MINIMIZE),
                1 if maximized => ("closefs", svg::CLOSE_FULLSCREEN),
                1 => ("openfull", svg::OPEN_IN_FULL),
                _ => ("close", svg::CLOSE),
            };
            let mut tint = th.text_primary;
            if resp.hovered() {
                if k == 2 {
                    c.rect(br, 0.0, if th.is_dark { theme::hex(0xc42c1e) } else { theme::hex(0xe81123) });
                    tint = Color32::WHITE;
                } else {
                    c.rect(br, 0.0, th.hover_bg);
                }
            }
            let tex = self.svg.get(&ctx, name, svgsrc, 24.0);
            c.image(&tex, Rect::from_center_size(br.center(), Vec2::splat(24.0)), tint);
            if resp.clicked() {
                match k {
                    0 => win::hide_main(),
                    1 => win::toggle_maximize(),
                    _ => win::hide_main(),
                }
            }
        }

        // Theme toggle (32×32, 8px spacing + 4px right margin before buttons)
        let tr = rect_xywh(min_r.left() - 8.0 - 4.0 - 32.0, r.center().y - 16.0, 32.0, 32.0);
        let tresp = ui.interact(tr, Id::new("theme-toggle"), Sense::CLICK);
        let tbg = anim::anim_color(&ctx, Id::new("theme-toggle-bg"), if tresp.hovered() { th.hover_bg } else { th.hover_bg.gamma_multiply(0.0) }, 150.0);
        c.rect(tr, 8.0, tbg);
        let icol = anim::anim_color(&ctx, Id::new("theme-toggle-ic"), if th.is_dark { theme::hex(0xfbbf24) } else { theme::hex(0x7c3aed) }, 200.0);
        c.icon(tr.center(), if th.is_dark { "light_mode" } else { "dark_mode" }, 18.0, icol);
        if tresp.clicked() {
            self.actions.push(Action::SetThemeMode(if th.is_dark { "light".into() } else { "dark".into() }));
        }
        widgets::tooltip(
            ui,
            &th,
            Id::new("theme-tip"),
            tresp.hovered(),
            tr,
            if th.is_dark { "Switch to Light Mode" } else { "Switch to Dark Mode" },
            400.0,
            TipPos::Above,
            Vec2::new(120.0, 32.0),
        );

        // Centered title
        if r.width() > 500.0 {
            c.text(r.center(), Align2::CENTER_CENTER, "Shobdo Calok", fb(12.0), th.text_primary);
        }

        // Open menu popup
        if let Some(mi) = self.menu_open {
            self.draw_menu(ui, mi, item_rects[mi]);
        }
    }

    fn draw_menu(&mut self, ui: &mut Ui, mi: usize, anchor: Rect) {
        let th = self.th;
        let ctx = ui.ctx().clone();
        // (label, action id) — None label = separator
        let items: &[(&str, u8)] = match mi {
            0 => &[("New Snippet", 0), ("Save All", 0), ("", 255), ("Exit", 1)],
            1 => &[("View All", 2), ("Import...", 3)],
            2 => &[("Backup", 0), ("Analyze", 0)],
            _ => &[("Docs", 4), ("About", 5)],
        };
        let layer = LayerId::new(Order::Foreground, Id::new("appmenu"));
        let painter = ctx.layer_painter(layer);
        let c = Canvas::new(&painter);
        let max_text = items.iter().filter(|i| i.1 != 255).map(|i| c.measure(i.0, f(12.0)).x + 8.0).fold(0.0, f32::max);
        let w = max_text + 6.0 * 2.0 + 48.0;
        let h: f32 = items.iter().map(|i| if i.1 == 255 { 13.0 } else { 34.0 }).sum();
        let mr = rect_xywh(anchor.left(), anchor.bottom(), w, h);
        self.menu_rect = Some(mr);

        // background + "shadow" ring
        c.rect_stroke(mr.expand(1.0), 11.0, 1.0, if th.is_dark { theme::argb(0x60000000) } else { theme::argb(0x20000000) });
        c.rect_fs(mr, 10.0, th.surface, 1.0, if th.is_dark { theme::hex(0x2a2d35) } else { theme::hex(0xe2e8f0) });

        let mut ui_l = Ui::new(ctx.clone(), Id::new("appmenu-ui"), UiBuilder::new().layer_id(layer).max_rect(mr));
        let mut y = mr.top();
        let mut chosen = None;
        for (k, (label, act)) in items.iter().enumerate() {
            if *act == 255 {
                c.hline(rect_xywh(mr.left() + 6.0, y + 6.0, mr.width() - 12.0, 1.0), th.tip_border);
                y += 13.0;
                continue;
            }
            let ir = rect_xywh(mr.left(), y, mr.width(), 34.0);
            let resp = ui_l.interact(ir, Id::new(("appmenu-item", mi, k)), Sense::CLICK);
            if resp.hovered() {
                c.rect(ir, 6.0, th.hover_bg);
            }
            c.text(Pos2::new(ir.left() + 6.0 + 8.0, ir.center().y), Align2::LEFT_CENTER, label, f(12.0), if resp.hovered() { th.primary } else { th.text_primary });
            if resp.clicked() {
                chosen = Some(*act);
            }
            y += 34.0;
        }
        if let Some(a) = chosen {
            self.menu_open = None;
            match a {
                1 => win::hide_main(),
                2 => {
                    self.page = 1;
                    self.actions.push(Action::ToggleAll);
                }
                3 => {
                    self.page = 1;
                    self.actions.push(Action::Import { toast: false });
                }
                4 => self.page = 0,
                5 => self.page = 2,
                _ => {}
            }
        } else {
            // close on press outside / Esc
            let pressed_outside = ctx.input(|i| {
                i.pointer.any_pressed()
                    && i.pointer.interact_pos().map(|p| !mr.contains(p) && !anchor.contains(p)).unwrap_or(false)
            });
            if pressed_outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.menu_open = None;
            }
        }
    }

    // ── Sidebar ─────────────────────────────────────────────────────────────

    fn sidebar(&mut self, ui: &mut Ui, r: Rect) {
        let th = self.th;
        let c = Canvas::new(ui.painter());
        c.rect(r, 0.0, th.background);
        let items: [(&str, &str, usize); 5] = [
            ("school", "Tutorial", 0),
            ("code_blocks", "Editor & Library", 1),
            ("info", "About", 2),
            ("backup", "Backup", 3),
            ("settings", "Settings", 4),
        ];
        for (k, (icon, tip, idx)) in items.iter().enumerate() {
            let ir = if *idx == 4 {
                rect_xywh(r.left() + 10.0, r.bottom() - 16.0 - 44.0, 44.0, 44.0)
            } else {
                rect_xywh(r.left() + 10.0, r.top() + 16.0 + k as f32 * 52.0, 44.0, 44.0)
            };
            let resp = ui.interact(ir, Id::new(("sidebar", k)), Sense::CLICK);
            let active = self.page == *idx;
            let bg = if active {
                th.navigation_active_bg
            } else if resp.hovered() {
                th.hover_bg
            } else {
                Color32::TRANSPARENT
            };
            c.rect(ir, 12.0, bg);
            if active {
                c.rect(rect_xywh(ir.left() - 10.0, ir.center().y - 10.0, 3.0, 20.0), 2.0, th.primary);
            }
            c.icon(ir.center(), icon, 20.0, if active { th.primary } else { th.text_secondary });
            if resp.clicked() {
                self.page = *idx;
            }
            widgets::tooltip(ui, &th, Id::new(("sidebar-tip", k)), resp.hovered(), ir, tip, 450.0, TipPos::Right(8.0), Vec2::new(120.0, 30.0));
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.th.background.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        win::finish_startup_hide();
        self.drain_events(&ctx);

        let full = ctx.content_rect();
        let mut ui = ui.new_child(UiBuilder::new().max_rect(full));
        ui.painter().rect_filled(full, 0.0, self.th.background);

        let title = rect_xywh(full.left(), full.top(), full.width(), 48.0);
        let side = Rect::from_min_max(Pos2::new(full.left(), title.bottom()), Pos2::new(full.left() + 64.0, full.bottom()));
        let item = Rect::from_min_max(Pos2::new(side.right(), title.bottom()), full.max);
        // StackLayout with right/bottom margins of 16
        let content = Rect::from_min_max(item.min, Pos2::new(item.right() - 16.0, item.bottom() - 16.0));

        self.sidebar(&mut ui, side);

        {
            let mut page_ui = ui.new_child(UiBuilder::new().max_rect(item));
            page_ui.set_clip_rect(item);
            match self.page {
                0 => tutorial::show(self, &mut page_ui, content),
                1 => {
                    let lib_r = Rect::from_min_max(
                        Pos2::new(content.left() + 8.0, content.top() + 8.0),
                        Pos2::new(content.left() + 8.0 + 260.0, content.bottom() - 8.0),
                    );
                    let ed_r = Rect::from_min_max(Pos2::new(lib_r.right() + 8.0, content.top() + 8.0), Pos2::new(content.right() - 8.0, content.bottom() - 8.0));
                    let c = Canvas::new(page_ui.painter());
                    c.rect(lib_r, 20.0, self.th.library_bg);
                    c.rect(ed_r, 24.0, self.th.editor_bg);
                    editor::show(self, &mut page_ui, ed_r.shrink(8.0));
                    library::show(self, &mut page_ui, lib_r.shrink(16.0));
                }
                2 => about::show(self, &mut page_ui, content),
                3 => backup::show(self, &mut page_ui, content),
                _ => settings_page::show(self, &mut page_ui, content),
            }
        }

        // Title bar is z: 10 — above the pages
        self.title_bar(&mut ui, title);

        // Global overlays (dialogs that live at window level)
        backup::show_dialogs(self, &mut ui, full);
        library::show_overlays(self, &mut ui);

        self.process_actions(&ctx);

        // Startup fade-in: appFrame opacity 0 → 1 over 500 ms (InOutQuad)
        let now = ctx.input(|i| i.time);
        let t0 = *self.start_time.get_or_insert(now);
        let el = ((now - t0) * 1000.0) as f32;
        if el < 500.0 {
            let v = anim::ease(Ease::InOutQuad, el / 500.0);
            let layer = LayerId::new(Order::Debug, Id::new("startup-fade"));
            ctx.layer_painter(layer).rect_filled(full, 0.0, self.th.background.gamma_multiply(1.0 - v));
            ctx.request_repaint();
        }
    }
}
