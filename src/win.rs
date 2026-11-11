//! Win32 helpers: frameless window (port of framelesswindow/win), foreground
//! capture/restore, Run-at-startup registry key, system theme detection.

#![allow(clippy::missing_safety_doc)]

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Dwm::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Controls::MARGINS;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static OLD_PROC: AtomicIsize = AtomicIsize::new(0);
static MAIN_VISIBLE: AtomicBool = AtomicBool::new(true);

pub fn main_hwnd() -> HWND {
    MAIN_HWND.load(Ordering::SeqCst) as HWND
}

pub fn is_main_visible() -> bool {
    MAIN_VISIBLE.load(Ordering::SeqCst)
}

// ── Foreground helpers (port of main.py _capture/_restore_foreground) ───────

pub fn capture_foreground() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

pub fn restore_foreground(hwnd: isize) {
    let hwnd = hwnd as HWND;
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let cur_tid = GetCurrentThreadId();
        let tgt_tid = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let mut attached = false;
        if tgt_tid != 0 && tgt_tid != cur_tid {
            attached = AttachThreadInput(cur_tid, tgt_tid, 1) != 0;
        }
        // Only un-minimize; never resize.
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
        BringWindowToTop(hwnd);
        if attached {
            AttachThreadInput(cur_tid, tgt_tid, 0);
        }
    }
}

/// Bring one of *our* windows to the front even when another process owns
/// the foreground (used by the fill-in dialog and the tray actions).
pub fn force_foreground(hwnd: HWND) {
    unsafe {
        let fg = GetForegroundWindow();
        let cur_tid = GetCurrentThreadId();
        let fg_tid = if fg.is_null() { 0 } else { GetWindowThreadProcessId(fg, std::ptr::null_mut()) };
        let mut attached = false;
        if fg_tid != 0 && fg_tid != cur_tid {
            attached = AttachThreadInput(cur_tid, fg_tid, 1) != 0;
        }
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
        BringWindowToTop(hwnd);
        SetFocus(hwnd);
        if attached {
            AttachThreadInput(cur_tid, fg_tid, 0);
        }
    }
}

// ── Main window state ────────────────────────────────────────────────────

pub fn show_main() {
    let hwnd = main_hwnd();
    if hwnd.is_null() {
        return;
    }
    MAIN_VISIBLE.store(true, Ordering::SeqCst);
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
    }
    force_foreground(hwnd);
    crate::request_repaint();
}

/// The window was created hidden (start-up launch) and never shown yet.
pub fn mark_hidden() {
    MAIN_VISIBLE.store(false, Ordering::SeqCst);
    // eframe always shows the window after its first frame. Make it fully
    // transparent until then; the app hides it on the following frame.
    let hwnd = main_hwnd();
    if !hwnd.is_null() {
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex | WS_EX_LAYERED | WS_EX_TOOLWINDOW) as isize);
            SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
        }
    }
    PENDING_HIDE.store(true, Ordering::SeqCst);
}

static PENDING_HIDE: AtomicBool = AtomicBool::new(false);

/// Called every frame by the app: finish a start-up (tray-only) launch.
pub fn finish_startup_hide() {
    if !PENDING_HIDE.load(Ordering::SeqCst) {
        return;
    }
    let hwnd = main_hwnd();
    if hwnd.is_null() || unsafe { IsWindowVisible(hwnd) } == 0 {
        return; // eframe has not shown it yet
    }
    PENDING_HIDE.store(false, Ordering::SeqCst);
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex & !(WS_EX_LAYERED | WS_EX_TOOLWINDOW)) as isize);
    }
}


pub fn hide_main() {
    let hwnd = main_hwnd();
    if hwnd.is_null() {
        return;
    }
    MAIN_VISIBLE.store(false, Ordering::SeqCst);
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn is_maximized() -> bool {
    let hwnd = main_hwnd();
    !hwnd.is_null() && unsafe { IsZoomed(hwnd) != 0 }
}

pub fn toggle_maximize() {
    let hwnd = main_hwnd();
    if hwnd.is_null() {
        return;
    }
    unsafe {
        if IsZoomed(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_MAXIMIZE);
        }
    }
}

/// Start a native window move (QML: mainWindow.startSystemMove()).
pub fn start_system_move() {
    let hwnd = main_hwnd();
    if hwnd.is_null() {
        return;
    }
    unsafe {
        ReleaseCapture();
        SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
    }
}

fn work_area_for(hwnd: HWND) -> RECT {
    unsafe {
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        mi.rcWork
    }
}

pub fn primary_work_area() -> RECT {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut _ as *mut _, 0);
        r
    }
}

pub fn dpi_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

/// Centre the window on the available area of the primary screen.
pub fn center_window(hwnd: HWND, logical_w: f32, logical_h: f32) {
    unsafe {
        let wa = primary_work_area();
        let s = dpi_scale(hwnd);
        let w = (logical_w * s).round() as i32;
        let h = (logical_h * s).round() as i32;
        let x = wa.left + ((wa.right - wa.left) - w) / 2;
        let y = wa.top + ((wa.bottom - wa.top) - h) / 2;
        SetWindowPos(hwnd, std::ptr::null_mut(), x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

// ── Frameless window (WindowsEventFilter + WindowsWindowEffect) ──────────────

const BORDER_WIDTH: i32 = 5;

unsafe extern "system" fn frameless_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCCALCSIZE => {
            if wparam != 0 && IsZoomed(hwnd) != 0 {
                let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
                params.rgrc[0] = work_area_for(hwnd);
            }
            return 0;
        }
        WM_NCHITTEST => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let mut r: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut r);
            if IsZoomed(hwnd) == 0 {
                let bw = (BORDER_WIDTH as f32 * dpi_scale(hwnd)).round() as i32;
                let w = r.right - r.left;
                let h = r.bottom - r.top;
                let xp = x - r.left;
                let yp = y - r.top;
                let lx = xp < bw;
                let rx = xp >= w - bw;
                let ty = yp < bw;
                let by = yp >= h - bw;
                let hit = if lx && ty {
                    HTTOPLEFT
                } else if rx && by {
                    HTBOTTOMRIGHT
                } else if rx && ty {
                    HTTOPRIGHT
                } else if lx && by {
                    HTBOTTOMLEFT
                } else if ty {
                    HTTOP
                } else if by {
                    HTBOTTOM
                } else if lx {
                    HTLEFT
                } else if rx {
                    HTRIGHT
                } else {
                    HTCLIENT
                };
                return hit as LRESULT;
            }
            return HTCLIENT as LRESULT;
        }
        WM_GETMINMAXINFO => {
            let old = OLD_PROC.load(Ordering::SeqCst);
            let res = call_old(old, hwnd, msg, wparam, lparam);
            if IsZoomed(hwnd) != 0 {
                let mut wr: RECT = std::mem::zeroed();
                GetWindowRect(hwnd, &mut wr);
                let mon = MonitorFromRect(&wr, MONITOR_DEFAULTTONEAREST);
                let mut mi: MONITORINFO = std::mem::zeroed();
                mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                if GetMonitorInfoW(mon, &mut mi) != 0 {
                    let info = &mut *(lparam as *mut MINMAXINFO);
                    let wa = mi.rcWork;
                    let mr = mi.rcMonitor;
                    info.ptMaxSize.x = wa.right - wa.left;
                    info.ptMaxSize.y = wa.bottom - wa.top;
                    info.ptMaxTrackSize.x = info.ptMaxSize.x;
                    info.ptMaxTrackSize.y = info.ptMaxSize.y;
                    info.ptMaxPosition.x = (wa.left - mr.left).abs();
                    info.ptMaxPosition.y = (wa.top - mr.top).abs();
                }
            }
            return res;
        }
        WM_SYSCOMMAND => {
            // The minimize animation (taskbar) should hide to tray like the
            // title-bar minimize button does.
            if (wparam & 0xFFF0) as u32 == SC_MINIMIZE {
                hide_main();
                return 0;
            }
        }
        _ => {}
    }
    let old = OLD_PROC.load(Ordering::SeqCst);
    call_old(old, hwnd, msg, wparam, lparam)
}

unsafe fn call_old(old: isize, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if old == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let f: WNDPROC = std::mem::transmute(old);
    CallWindowProcW(f, hwnd, msg, wparam, lparam)
}

/// Turn the winit window into a frameless one that keeps native resize,
/// snapping, minimize/maximize animations and the DWM shadow.
pub fn install_frameless(hwnd: HWND) {
    MAIN_HWND.store(hwnd as isize, Ordering::SeqCst);
    unsafe {
        let old = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, frameless_proc as usize as isize);
        OLD_PROC.store(old, Ordering::SeqCst);

        // addWindowAnimation
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        SetWindowLongPtrW(
            hwnd,
            GWL_STYLE,
            (style | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_CAPTION | WS_THICKFRAME | WS_SYSMENU) as isize,
        );

        // addShadowEffect
        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        DwmExtendFrameIntoClientArea(hwnd, &margins);
        let pref: i32 = 2; // DWMWCP_ROUND
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &pref as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

pub fn set_dark_titlebar(hwnd: HWND, dark: bool) {
    unsafe {
        let v: i32 = dark as i32;
        DwmSetWindowAttribute(hwnd, 20, &v as *const _ as *const _, 4); // DWMWA_USE_IMMERSIVE_DARK_MODE
    }
}

pub fn set_round_corners(hwnd: HWND) {
    unsafe {
        let pref: i32 = 2;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &pref as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

// ── Registry: theme + run at startup ──────────────────────────────────────

unsafe fn reg_read_dword(root: HKEY, path: &str, name: &str) -> Option<u32> {
    let mut key: HKEY = std::ptr::null_mut();
    if RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut key) != 0 {
        return None;
    }
    let mut data: u32 = 0;
    let mut size = 4u32;
    let mut ty = 0u32;
    let r = RegQueryValueExW(key, wide(name).as_ptr(), std::ptr::null(), &mut ty, &mut data as *mut u32 as *mut u8, &mut size);
    RegCloseKey(key);
    (r == 0).then_some(data)
}

/// Ask Windows whether apps should use dark mode.
pub fn system_is_dark() -> bool {
    unsafe {
        reg_read_dword(HKEY_CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "AppsUseLightTheme")
            .map(|v| v == 0)
            .unwrap_or(false)
    }
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn startup_enabled() -> bool {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return false;
        }
        let r = RegQueryValueExW(
            key,
            wide(crate::version::APP_NAME).as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        RegCloseKey(key);
        r == 0
    }
}

pub fn set_startup(enabled: bool) -> bool {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut key) != 0 {
            eprintln!("Startup toggle error: cannot open Run key");
            return false;
        }
        let name = wide(crate::version::APP_NAME);
        let ok = if enabled {
            let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
            // `--startup`: launched by Windows at sign-in → start silently in the tray.
            let val = wide(&format!("\"{exe}\" --startup"));
            RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, val.as_ptr() as *const u8, (val.len() * 2) as u32) == 0
        } else {
            let r = RegDeleteValueW(key, name.as_ptr());
            r == 0 || r == ERROR_FILE_NOT_FOUND
        };
        RegCloseKey(key);
        ok
    }
}

pub fn set_app_user_model_id() {
    unsafe {
        windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(wide("com.shobdocalok.v1").as_ptr());
    }
}

/// Open a URL with the default browser (Python: webbrowser.open).
pub fn open_url(url: &str) {
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Make a raw HWND (isize) usable across threads.
pub fn hwnd_from(v: isize) -> HWND {
    v as HWND
}

pub fn is_key_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}
