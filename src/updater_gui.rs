//! Port of updater_gui.py — the standalone progress window that replaces the
//! program files after the main app exits, then relaunches it.
//!
//! Invoked as: ShobdoCalok-Updater.exe --updater <src_dir> <dst_dir> <app_exe>
//!             <extract_dir> <update_dir> [parent_pid]

use egui::{Align2, Color32, Pos2, Rect, Ui, Vec2};
use parking_lot::Mutex;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::draw::Canvas;
use crate::fonts::{f, fb};
use crate::theme::hex;

fn log(dst_dir: &Path, message: &str, kv: &[(&str, String)]) {
    let log_path = dst_dir.join("AppData").join("update_debug.log");
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let mut lines = vec![String::new(), "=".repeat(70), format!("[{ts}] GUI UPDATER"), format!("Message: {message}")];
    for (k, v) in kv {
        let mut sv = v.clone();
        if sv.chars().count() > 500 {
            sv = sv.chars().take(500).collect::<String>() + "...";
        }
        lines.push(format!("{k}: {sv}"));
    }
    lines.push("=".repeat(70));
    lines.push(String::new());
    let _ = std::fs::create_dir_all(dst_dir.join("AppData"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = f.write_all(lines.join("\n").as_bytes());
    }
}

#[derive(Default, Clone)]
struct Progress {
    pct: i32,
    status: String,
    detail: String,
}

struct UpdaterApp {
    p: Arc<Mutex<Progress>>,
}

impl eframe::App for UpdaterApp {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        hex(0x1a1b2e).to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = self.p.lock().clone();
        let screen = ctx.content_rect();
        let c = Canvas::new(ui.painter());
        c.rect(screen, 0.0, hex(0x1a1b2e));
        let inner = Rect::from_min_max(screen.min + Vec2::new(28.0, 22.0), screen.max - Vec2::new(28.0, 22.0));
        let mut y = inner.top();
        let r = c.text(Pos2::new(inner.center().x, y), Align2::CENTER_TOP, "ShobdoCalok Updater", fb(20.0), hex(0xa78bfa));
        y = r.bottom() + 14.0;
        let sr = c.text_wrap_center(inner.center().x, y, inner.width(), &p.status, f(13.3), hex(0xe2e8f0), None);
        y += sr.height() + 14.0;
        let bar = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 10.0));
        c.rect(bar, 5.0, hex(0x2a2b3e));
        let fw = bar.width() * (p.pct as f32 / 100.0);
        if fw > 0.0 {
            c.rect(Rect::from_min_size(bar.min, Vec2::new(fw, 10.0)), 5.0, hex(0xa78bfa));
        }
        y += 10.0 + 14.0;
        c.text(Pos2::new(inner.center().x, y), Align2::CENTER_TOP, &p.detail, f(10.7), hex(0x64748b));
        ctx.request_repaint_after(Duration::from_millis(100));
        let _ = Color32::WHITE;
    }
}

fn wait_for_pid(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if !h.is_null() {
            WaitForSingleObject(h, 15_000);
            CloseHandle(h);
        }
    }
}

fn copy_worker(src: PathBuf, dst: PathBuf, p: Arc<Mutex<Progress>>) -> (bool, String) {
    let set = |pct: i32, msg: &str| {
        let mut g = p.lock();
        g.pct = pct;
        g.status = msg.to_string();
    };
    set(5, "Preparing to copy files...");
    log(&dst, "CopyWorker started", &[("src_dir", src.display().to_string()), ("dst_dir", dst.display().to_string())]);
    std::thread::sleep(Duration::from_millis(300));
    if !src.is_dir() {
        log(&dst, "ERROR: source not found", &[("src_dir", src.display().to_string())]);
        return (false, format!("Source not found: {}", src.display()));
    }
    let files = crate::updater::walk_files(&src);
    let total = files.len();
    if total == 0 {
        log(&dst, "ERROR: no files in update package", &[]);
        return (false, "No files found in update package!".into());
    }
    set(10, &format!("Found {total} files to copy..."));
    log(&dst, &format!("Found {total} files to copy"), &[]);
    std::thread::sleep(Duration::from_millis(300));

    let self_exe = std::env::current_exe().unwrap_or_default();
    let mut copied = 0usize;
    let mut errors: Vec<String> = vec![];
    let mut deferred: Vec<(PathBuf, PathBuf)> = vec![];
    for src_file in files {
        let rel = src_file.strip_prefix(&src).unwrap_or(&src_file).to_path_buf();
        let mut dst_file = dst.join(&rel);
        if let Some(parent) = dst_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let fname = rel.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let is_self = dst_file == self_exe;
        if is_self {
            dst_file = PathBuf::from(format!("{}.new", dst_file.display()));
        }
        let mut ok = false;
        let mut last_err = String::new();
        for _ in 0..3 {
            match std::fs::copy(&src_file, &dst_file) {
                Ok(_) => {
                    ok = true;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32) => {
                    last_err = e.to_string();
                    std::thread::sleep(Duration::from_millis(500));
                }
                Err(e) => {
                    last_err = e.to_string();
                    errors.push(format!("{fname}: {e}"));
                    break;
                }
            }
        }
        if !ok {
            errors.push(format!("{fname}: locked after 3 retries - {last_err}"));
            deferred.push((src_file.clone(), dst_file.clone()));
        } else if is_self {
            deferred.push((dst_file.clone(), self_exe.clone()));
        }
        copied += 1;
        let pct = 10 + ((copied as f32 / total as f32) * 80.0) as i32;
        if copied % 5 == 0 || copied == total {
            set(pct, &format!("Copying: {fname} ({copied}/{total})"));
        }
    }
    if !errors.is_empty() {
        set(95, &format!("Copied {copied}/{total} files ({} issues)", errors.len()));
        log(&dst, &format!("Copy completed with errors: {copied}/{total} copied, {} failed", errors.len()), &[("errors", errors.iter().take(10).cloned().collect::<Vec<_>>().join("; "))]);
    } else {
        set(95, &format!("All {copied} files copied!"));
        log(&dst, &format!("Copy completed successfully: {copied} files"), &[]);
    }
    if !deferred.is_empty() {
        let dl = dst.join("AppData").join("deferred_updates.txt");
        let body: String = deferred.iter().map(|(s, d)| format!("{}|{}\n", s.display(), d.display())).collect();
        if std::fs::write(&dl, body).is_ok() {
            log(&dst, &format!("Wrote {} deferred file(s) to {}", deferred.len(), dl.display()), &[]);
        }
    }
    std::thread::sleep(Duration::from_millis(500));
    (true, format!("Update complete! {copied} files installed."))
}

pub fn run(args: &[String]) {
    if args.len() < 5 {
        eprintln!("Usage: --updater <src_dir> <dst_dir> <app_exe> <extract_dir> <update_dir>");
        std::process::exit(1);
    }
    let src = PathBuf::from(&args[0]);
    let dst = PathBuf::from(&args[1]);
    let app_exe = PathBuf::from(&args[2]);
    let extract_dir = PathBuf::from(&args[3]);
    let update_dir = PathBuf::from(&args[4]);
    let parent_pid: Option<u32> = args.get(5).and_then(|s| s.parse().ok());
    log(
        &dst,
        "GUI updater started",
        &[
            ("src_dir", src.display().to_string()),
            ("dst_dir", dst.display().to_string()),
            ("app_exe", app_exe.display().to_string()),
            ("extract_dir", extract_dir.display().to_string()),
            ("update_dir", update_dir.display().to_string()),
        ],
    );

    let p = Arc::new(Mutex::new(Progress { pct: 0, status: "Preparing update...".into(), detail: String::new() }));
    {
        let p = p.clone();
        std::thread::spawn(move || {
            if let Some(pid) = parent_pid {
                wait_for_pid(pid);
            }
            std::thread::sleep(Duration::from_millis(500));
            let (success, msg) = copy_worker(src, dst.clone(), p.clone());
            {
                let mut g = p.lock();
                g.pct = 100;
                g.status = msg.clone();
                g.detail = "Starting ShobdoCalok...".into();
            }
            log(&dst, &format!("Copy finished: success={success}, msg={msg}"), &[]);
            if extract_dir.is_dir() {
                let _ = std::fs::remove_dir_all(&extract_dir);
                log(&dst, &format!("Removed extract dir: {}", extract_dir.display()), &[]);
            }
            std::thread::sleep(Duration::from_secs(1));
            log(&dst, &format!("Launching app: {}", app_exe.display()), &[]);
            let _ = std::process::Command::new(&app_exe).current_dir(app_exe.parent().unwrap_or(Path::new("."))).spawn();
            if update_dir.is_dir() {
                let _ = std::fs::remove_dir_all(&update_dir);
                log(&dst, &format!("Removed update dir: {}", update_dir.display()), &[]);
            }
            std::process::exit(0);
        });
    }

    let wa = crate::win::primary_work_area();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ShobdoCalok Updater")
            .with_inner_size([460.0, 220.0])
            .with_resizable(false)
            .with_minimize_button(false)
            .with_maximize_button(false)
            .with_close_button(false)
            .with_always_on_top()
            .with_position([
                (wa.left + ((wa.right - wa.left) - 460) / 2) as f32,
                (wa.top + ((wa.bottom - wa.top) - 220) / 2) as f32,
            ])
            .with_icon(crate::svg::icon_data(crate::svg::APP_ICON, 64)),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "ShobdoCalok Updater",
        options,
        Box::new(move |cc| {
            crate::fonts::install(&cc.egui_ctx);
            Ok(Box::new(UpdaterApp { p }))
        }),
    );
}
