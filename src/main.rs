#![windows_subsystem = "windows"]
#![allow(deprecated)]
//! Shobdo Calok — a fast, keyboard-friendly text expander for Windows.
//! Pure Rust / egui port of the original PySide6 + QML application.

mod anim;
mod app;
mod draw;
mod drive;
mod engine;
mod events;
mod field_dialog;
mod fonts;
mod pages;
mod settings;
mod store;
mod svg;
mod theme;
mod tray;
mod updater;
mod updater_gui;
mod version;
mod widgets;
mod win;

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

static CTX: OnceLock<egui::Context> = OnceLock::new();

/// Wake the UI from any thread.
pub fn request_repaint() {
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

pub fn app_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("--field-dialog") => {
            field_dialog::run_child(args.get(2).map(|s| s == "dark").unwrap_or(true));
            return;
        }
        Some("--updater") => {
            updater_gui::run(&args[2..]);
            return;
        }
        _ => {}
    }

    // ── Windows taskbar icon fix ────────────────────────────────────────────
    win::set_app_user_model_id();

    // ── Data folder: AppData/ next to the EXE — fully portable ──────────────
    let data_dir = app_root().join("AppData");
    let _ = std::fs::create_dir_all(&data_dir);

    // ── Run at startup follows the exe ──────────────────────────────────────
    // The app is portable: if it was moved, re-point the Run key at this exe.
    // (Skipped for developer screenshot runs so they never touch the registry.)
    if win::startup_enabled() && std::env::var_os("SHOBDO_NOACTIVATE").is_none() {
        win::set_startup(true);
    }
    // Launched by Windows at sign-in (Run key adds --startup): stay in the tray.
    let started_by_windows = args.iter().any(|a| a == "--startup");

    // ── Apply deferred updates from the previous run ────────────────────────
    updater::apply_deferred_updates(&data_dir);

    // ── Backend: snippet store + settings ───────────────────────────────────
    let store = Arc::new(Mutex::new(store::SnippetStore::new(&data_dir.join("snippets.json"))));
    let settings = settings::Settings::load(&data_dir.join("settings.ini"));
    let mut theme_mode = settings.value("appearance/themeMode", "system");
    if !["system", "dark", "light"].contains(&theme_mode.as_str()) {
        theme_mode = "system".into();
    }
    let is_dark = match theme_mode.as_str() {
        "dark" => true,
        "light" => false,
        _ => win::system_is_dark(),
    };

    let events_rx = events::channel();
    let drive = drive::Drive::new(&data_dir, store.clone());
    let engine = engine::Engine::new(store.clone(), is_dark);
    let updater = updater::Updater::new(&data_dir);

    // ── Start the keyboard hook, with retry ─────────────────────────────────
    // WH_KEYBOARD_LL can fail to install while Windows is still starting up.
    std::thread::sleep(Duration::from_millis(500));
    let mut started = false;
    for attempt in 0..5u64 {
        eprintln!("[Main] Starting expansion engine (attempt {}/5)", attempt + 1);
        if engine.start() {
            eprintln!("[Main] Expansion engine started successfully");
            started = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500 + attempt * 500));
    }
    if !started {
        eprintln!("[Main] WARNING: Expansion engine failed to start after all retries");
    }

    // ── System tray ─────────────────────────────────────────────────────────
    tray::set_dark(is_dark);
    {
        let engine = engine.clone();
        let updater = updater.clone();
        tray::start(move |action| match action {
            tray::TrayAction::Show => win::show_main(),
            tray::TrayAction::ToggleExpansion => {
                let enabled = !engine.is_enabled();
                engine.set_enabled(enabled);
                tray::set_paused(!enabled);
                tray::notify(
                    "Shobdo Calok",
                    if enabled { "Text expansion enabled" } else { "Text expansion paused" },
                    false,
                    2000,
                );
            }
            tray::TrayAction::CheckUpdates => {
                events::emit(events::Event::ShowAboutPage);
                win::show_main();
                updater.check_for_update();
            }
            tray::TrayAction::Quit => {
                engine.stop();
                tray::remove();
                std::process::exit(0);
            }
            tray::TrayAction::SystemResumed => {
                eprintln!("[Main] System resumed — restarting expansion engine");
                let e = engine.clone();
                std::thread::spawn(move || {
                    // give the input subsystem a moment after wake/unlock
                    std::thread::sleep(Duration::from_millis(1000));
                    if e.is_enabled() {
                        e.restart();
                    }
                });
            }
        });
    }

    // ── Periodic health check: restart the hook if it died ──────────────────
    // Fast checks (500 ms) for the first 30 s, then every 5 s.
    {
        let engine = engine.clone();
        std::thread::spawn(move || {
            let t0 = std::time::Instant::now();
            let mut last_wall = std::time::SystemTime::now();
            loop {
                let fast = t0.elapsed() < Duration::from_secs(30);
                let interval = Duration::from_millis(if fast { 500 } else { 5000 });
                std::thread::sleep(interval);
                // Sleep/hibernate detection: the wall clock jumped far more than
                // we slept. Windows silently drops low-level hooks across sleep
                // (the hook thread stays alive), so always re-install it.
                let now_wall = std::time::SystemTime::now();
                let gap = now_wall.duration_since(last_wall).unwrap_or_default();
                last_wall = now_wall;
                if engine.is_enabled() && gap > interval + Duration::from_secs(10) {
                    eprintln!("[Main] Wake from sleep detected — re-installing keyboard hook");
                    std::thread::sleep(Duration::from_millis(1500));
                    engine.restart();
                    continue;
                }
                if engine.is_enabled() && !engine.is_alive() {
                    eprintln!("[Main] Keyboard hook is dead — auto-restarting");
                    engine.restart();
                }
            }
        });
    }

    // ── UI ──────────────────────────────────────────────────────────────────
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Shobdo Calok")
            .with_app_id("ShobdoCalok")
            .with_inner_size([900.0, 650.0])
            .with_min_inner_size([800.0, 550.0])
            .with_visible(false)
            .with_icon(svg::icon_data(svg::APP_ICON, 64)),
        ..Default::default()
    };
    let res = eframe::run_native(
        "Shobdo Calok",
        options,
        Box::new(move |cc| {
            let _ = CTX.set(cc.egui_ctx.clone());
            fonts::install(&cc.egui_ctx);
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(h) = cc.window_handle() {
                if let RawWindowHandle::Win32(w) = h.as_raw() {
                    let hwnd = w.hwnd.get() as windows_sys::Win32::Foundation::HWND;
                    win::install_frameless(hwnd);
                    win::center_window(hwnd, 900.0, 650.0);
                    // SHOBDO_NOACTIVATE: developer aid (screenshots) — show without stealing focus.
                    let no_activate = std::env::var_os("SHOBDO_NOACTIVATE").is_some();
                    if started_by_windows {
                        // Hidden at boot; tray "Show Application" reveals it.
                        win::mark_hidden();
                    } else {
                        unsafe {
                            use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOW, SW_SHOWNOACTIVATE};
                            ShowWindow(hwnd, if no_activate { SW_SHOWNOACTIVATE } else { SW_SHOW });
                        }
                    }
                    if started_by_windows {
                    } else if no_activate {
                        // keep developer screenshot runs off the visible desktop
                        unsafe {
                            use windows_sys::Win32::UI::WindowsAndMessaging::*;
                            SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE);
                        }
                    }
                    if !no_activate && !started_by_windows {
                        win::force_foreground(hwnd);
                    }
                }
            }
            Ok(Box::new(app::App::new(
                &cc.egui_ctx,
                store,
                engine,
                updater,
                drive,
                settings,
                theme_mode,
                is_dark,
                events_rx,
            )))
        }),
    );
    if let Err(e) = res {
        eprintln!("UI error: {e}");
    }
    tray::remove();
}
