//! System tray icon + context menu + balloon messages, implemented directly on
//! Shell_NotifyIcon so it keeps working while the main window is hidden.
//! Menu: Show Application │ Pause/Resume Expansion, Check for Updates │ Quit

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::win::wide;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    ToggleExpansion,
    CheckUpdates,
    Quit,
    SystemResumed,
}

const WM_TRAY: u32 = WM_APP + 1;
const ID_SHOW: usize = 1001;
const ID_TOGGLE: usize = 1002;
const ID_UPDATE: usize = 1003;
const ID_QUIT: usize = 1004;

static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static PAUSED: AtomicBool = AtomicBool::new(false);
static DARK: AtomicBool = AtomicBool::new(true);
static HANDLER: OnceLock<Box<dyn Fn(TrayAction) + Send + Sync>> = OnceLock::new();

pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
}

pub fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::SeqCst);
}

pub fn start(handler: impl Fn(TrayAction) + Send + Sync + 'static) {
    let _ = HANDLER.set(Box::new(handler));
    std::thread::Builder::new()
        .name("tray".into())
        .spawn(|| unsafe { tray_thread() })
        .ok();
}

unsafe fn nid(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = std::mem::zeroed();
    n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = hwnd;
    n.uID = 1;
    n
}

fn copy_wstr(dst: &mut [u16], s: &str) {
    let w: Vec<u16> = s.encode_utf16().collect();
    let n = w.len().min(dst.len() - 1);
    dst[..n].copy_from_slice(&w[..n]);
    dst[n] = 0;
}

unsafe fn add_icon(hwnd: HWND) {
    let hinst = GetModuleHandleW(std::ptr::null());
    let cx = GetSystemMetrics(SM_CXSMICON);
    let cy = GetSystemMetrics(SM_CYSMICON);
    let icon = LoadImageW(hinst, 1 as _, IMAGE_ICON, cx, cy, LR_DEFAULTCOLOR) as HICON;
    let mut n = nid(hwnd);
    n.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = if icon.is_null() { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) } else { icon };
    copy_wstr(&mut n.szTip, "Shobdo Calok");
    Shell_NotifyIconW(NIM_ADD, &n);
}

/// tray_icon.showMessage(title, msg, icon, timeout)
pub fn notify(title: &str, msg: &str, warning: bool, timeout_ms: u32) {
    let hwnd = TRAY_HWND.load(Ordering::SeqCst) as HWND;
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let mut n = nid(hwnd);
        n.uFlags = NIF_INFO;
        copy_wstr(&mut n.szInfoTitle, title);
        copy_wstr(&mut n.szInfo, msg);
        n.dwInfoFlags = if warning { NIIF_WARNING } else { NIIF_INFO };
        n.Anonymous.uTimeout = timeout_ms;
        Shell_NotifyIconW(NIM_MODIFY, &n);
    }
}

pub fn remove() {
    let hwnd = TRAY_HWND.load(Ordering::SeqCst) as HWND;
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let n = nid(hwnd);
        Shell_NotifyIconW(NIM_DELETE, &n);
    }
}

/// uxtheme private API: make popup menus follow the app theme.
unsafe fn apply_menu_theme(hwnd: HWND, dark: bool) {
    let lib = LoadLibraryW(wide("uxtheme.dll").as_ptr());
    if lib.is_null() {
        return;
    }
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type AllowDarkModeForWindow = unsafe extern "system" fn(HWND, i32) -> i32;
    type FlushMenuThemes = unsafe extern "system" fn();
    if let Some(f) = GetProcAddress(lib, 135 as _) {
        let f: SetPreferredAppMode = std::mem::transmute(f);
        f(if dark { 2 } else { 3 });
    }
    if let Some(f) = GetProcAddress(lib, 133 as _) {
        let f: AllowDarkModeForWindow = std::mem::transmute(f);
        f(hwnd, dark as i32);
    }
    if let Some(f) = GetProcAddress(lib, 136 as _) {
        let f: FlushMenuThemes = std::mem::transmute(f);
        f();
    }
}

/// 32-bit premultiplied top-down DIB for a menu item bitmap.
unsafe fn svg_bitmap(svg: &str, color: &str, size: u32) -> HBITMAP {
    let svg = crate::svg::recolor(svg, color);
    let Some(pm) = crate::svg::rasterize(&svg, size, size) else { return std::ptr::null_mut() };
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = size as i32;
    bmi.bmiHeader.biHeight = -(size as i32);
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let hdc = GetDC(std::ptr::null_mut());
    let bmp = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
    ReleaseDC(std::ptr::null_mut(), hdc);
    if bmp.is_null() || bits.is_null() {
        return std::ptr::null_mut();
    }
    let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (size * size * 4) as usize);
    for (i, px) in pm.data().chunks_exact(4).enumerate() {
        dst[i * 4] = px[2];
        dst[i * 4 + 1] = px[1];
        dst[i * 4 + 2] = px[0];
        dst[i * 4 + 3] = px[3];
    }
    bmp
}

unsafe fn show_menu(hwnd: HWND) {
    let dark = DARK.load(Ordering::SeqCst);
    apply_menu_theme(hwnd, dark);
    let menu = CreatePopupMenu();
    let color = if dark { "#f1f5f9" } else { "#0f172a" };
    let dpi = windows_sys::Win32::UI::HiDpi::GetDpiForWindow(hwnd).max(96);
    let px = (20 * dpi / 96).max(16);
    let paused = PAUSED.load(Ordering::SeqCst);
    let mut bitmaps = vec![];
    let mut add = |id: usize, text: &str, svg: &str| {
        let bmp = svg_bitmap(svg, color, px);
        bitmaps.push(bmp);
        let w = wide(text);
        let mut mii: MENUITEMINFOW = std::mem::zeroed();
        mii.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
        mii.fMask = MIIM_ID | MIIM_STRING | MIIM_BITMAP | MIIM_FTYPE;
        mii.fType = MFT_STRING;
        mii.wID = id as u32;
        mii.dwTypeData = w.as_ptr() as *mut u16;
        mii.hbmpItem = bmp;
        InsertMenuItemW(menu, GetMenuItemCount(menu) as u32, 1, &mii);
    };
    add(ID_SHOW, "Show Application", crate::svg::TRAY_SHOW);
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    if paused {
        add(ID_TOGGLE, "Resume Expansion", crate::svg::TRAY_RESUME);
    } else {
        add(ID_TOGGLE, "Pause Expansion", crate::svg::TRAY_PAUSE);
    }
    add(ID_UPDATE, "Check for Updates", crate::svg::TRAY_UPDATE);
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    add(ID_QUIT, "Quit", crate::svg::TRAY_QUIT);

    let mut pt: POINT = std::mem::zeroed();
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, std::ptr::null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);
    for b in bitmaps {
        if !b.is_null() {
            DeleteObject(b as _);
        }
    }
}

fn dispatch(a: TrayAction) {
    if let Some(h) = HANDLER.get() {
        h(a);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            match (lparam & 0xFFFF) as u32 {
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                WM_LBUTTONDBLCLK => dispatch(TrayAction::Show),
                _ => {}
            }
            return 0;
        }
        WM_COMMAND => {
            match wparam & 0xFFFF {
                ID_SHOW => dispatch(TrayAction::Show),
                ID_TOGGLE => dispatch(TrayAction::ToggleExpansion),
                ID_UPDATE => dispatch(TrayAction::CheckUpdates),
                ID_QUIT => dispatch(TrayAction::Quit),
                _ => {}
            }
            return 0;
        }
        WM_POWERBROADCAST => {
            // PBT_APMRESUMEAUTOMATIC (0x12), PBT_APMRESUMESUSPEND (0x7),
            // PBT_APMRESUMECRITICAL (0x6)
            if wparam == 0x12 || wparam == 0x7 || wparam == 0x6 {
                dispatch(TrayAction::SystemResumed);
            }
            return 1;
        }
        // WM_WTSSESSION_CHANGE: waking from sleep normally ends at the lock
        // screen; re-install the hook once the user unlocks (WTS_SESSION_UNLOCK).
        0x02B1 => {
            if wparam == 0x8 {
                dispatch(TrayAction::SystemResumed);
            }
            return 0;
        }
        _ => {}
    }
    if msg != 0 && msg == TASKBAR_CREATED.load(Ordering::SeqCst) {
        add_icon(hwnd);
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn tray_thread() {
    let hinst = GetModuleHandleW(std::ptr::null());
    let class = wide("ShobdoCalokTrayWindow");
    let mut wc: WNDCLASSW = std::mem::zeroed();
    wc.lpfnWndProc = Some(wndproc);
    wc.hInstance = hinst;
    wc.lpszClassName = class.as_ptr();
    RegisterClassW(&wc);
    // A real (hidden) top-level window — message-only windows do not receive
    // WM_POWERBROADCAST, which we need to restart the hook after sleep.
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class.as_ptr(),
        wide("Shobdo Calok Tray").as_ptr(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return;
    }
    TRAY_HWND.store(hwnd as isize, Ordering::SeqCst);
    TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), Ordering::SeqCst);
    add_icon(hwnd);
    // Modern Standby (S0) PCs only deliver resume notifications to windows
    // that explicitly register for them — the Python version never did,
    // which is why expansion stopped working after sleep.
    windows_sys::Win32::System::Power::RegisterSuspendResumeNotification(hwnd as _, 0 /* DEVICE_NOTIFY_WINDOW_HANDLE */);
    windows_sys::Win32::System::RemoteDesktop::WTSRegisterSessionNotification(hwnd, 0 /* NOTIFY_FOR_THIS_SESSION */);
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}
