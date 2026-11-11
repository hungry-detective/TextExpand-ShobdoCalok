//! Port of viewmodels/expansion_engine.py — system-wide keyboard hook for
//! text expansion.
//!
//! A `WH_KEYBOARD_LL` hook (on its own thread with a message loop) watches
//! every keypress. When the user types a registered abbreviation (bounded by a
//! word boundary) the abbreviation is deleted and the expanded content is
//! inserted via the clipboard (falling back to typed unicode input).
//!
//! Tokens:
//!   {date} {time} {datetime} {clipboard} {cursor} {field:Label}
//!   {date:fmt} {time:fmt} {datetime:fmt}   (strftime patterns)

use parking_lot::Mutex;
use regex::Regex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::Duration;

use chrono::format::{Item, StrftimeItems};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::store::SnippetStore;

/// Marker placed in `dwExtraInfo` of every event we inject ourselves.
const INJECT_MAGIC: usize = 0x5348_4243; // "SHBC"

static ENGINE: OnceLock<Arc<Engine>> = OnceLock::new();

pub struct Engine {
    store: Arc<Mutex<SnippetStore>>,
    enabled: AtomicBool,
    buffer: Mutex<Vec<char>>,
    /// True while we are programmatically injecting keystrokes (backspace,
    /// typing, paste). The global hook must ignore these to avoid feedback.
    injecting: AtomicBool,
    hook_tid: AtomicU32,
    hook_alive: AtomicBool,
    restart_lock: Mutex<()>,
    pub is_dark: AtomicBool,
}

impl Engine {
    pub fn new(store: Arc<Mutex<SnippetStore>>, is_dark: bool) -> Arc<Self> {
        let e = Arc::new(Self {
            store,
            enabled: AtomicBool::new(true),
            buffer: Mutex::new(Vec::new()),
            injecting: AtomicBool::new(false),
            hook_tid: AtomicU32::new(0),
            hook_alive: AtomicBool::new(false),
            restart_lock: Mutex::new(()),
            is_dark: AtomicBool::new(is_dark),
        });
        let _ = ENGINE.set(e.clone());
        e
    }

    // ── Public API ───────────────────────────────────────────────────────────

    /// Start the keyboard listener. Returns true if it started successfully.
    pub fn start(&self) -> bool {
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        std::thread::Builder::new()
            .name("keyboard-hook".into())
            .spawn(move || unsafe { hook_thread(tx) })
            .ok();
        let ok = rx.recv_timeout(Duration::from_millis(2000)).unwrap_or(false);
        // Give the listener thread a moment to initialize
        std::thread::sleep(Duration::from_millis(200));
        if !ok || !self.is_alive() {
            eprintln!("[ExpansionEngine] Listener thread died immediately after start");
            return false;
        }
        true
    }

    pub fn stop(&self) {
        let tid = self.hook_tid.swap(0, Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                PostThreadMessageW(tid, WM_QUIT, 0, 0);
            }
            for _ in 0..50 {
                if !self.hook_alive.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    /// Stop the current listener and start a new one (after sleep/resume the
    /// OS silently drops low-level hooks).
    pub fn restart(&self) -> bool {
        // Wake-from-sleep can trigger several restarts at once (resume
        // notification, unlock, watchdog) — never let two hooks coexist.
        let _guard = self.restart_lock.lock();
        eprintln!("[ExpansionEngine] Restarting keyboard listener");
        self.stop();
        self.buffer.lock().clear();
        std::thread::sleep(Duration::from_millis(100));
        self.start()
    }

    pub fn is_alive(&self) -> bool {
        self.hook_alive.load(Ordering::SeqCst)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        if enabled {
            self.buffer.lock().clear();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Call after snippets are changed so the engine picks up new data.
    pub fn refresh_store(&self) {
        self.buffer.lock().clear();
    }

    /// Resolve tokens for a live preview; {field:Label} shows as [Label].
    pub fn preview(&self, content: &str) -> String {
        resolve_tokens(content, true, false)
    }

    // ── Keyboard listener ────────────────────────────────────────────────────

    fn on_key_down(self: &Arc<Self>, kb: &KBDLLHOOKSTRUCT) {
        if kb.dwExtraInfo == INJECT_MAGIC || self.injecting.load(Ordering::SeqCst) {
            return;
        }
        if !self.enabled.load(Ordering::SeqCst) {
            return;
        }
        let vk = kb.vkCode as u16;
        // Modifier keys on their own never change the buffer.
        if matches!(
            vk,
            VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU
                | VK_RMENU | VK_LWIN | VK_RWIN | VK_CAPITAL | VK_NUMLOCK | VK_SCROLL
        ) {
            return;
        }
        let mut buf = self.buffer.lock();
        if vk == VK_BACK {
            buf.pop();
            return;
        }
        match unsafe { key_to_chars(kb) } {
            KeyChars::Chars(chars) => {
                buf.extend(chars);
                // Trim buffer — no abbreviation should be longer than 40 chars
                let len = buf.len();
                if len > 40 {
                    buf.drain(0..len - 40);
                }
                if buf.len() >= 2 {
                    self.try_expand_immediate(&mut buf);
                }
            }
            KeyChars::Dead => {}
            KeyChars::None => buf.clear(),
        }
    }

    /// Finds a boundary-bounded abbreviation match and expands it.
    fn try_expand_immediate(self: &Arc<Self>, buf: &mut Vec<char>) {
        let found = {
            let mut store = self.store.lock();
            let abbrevs = store.abbreviations_map();
            let n = buf.len();
            let mut found = None;
            for length in (2..=n.min(30)).rev() {
                let target: String = buf[n - length..].iter().collect();
                if let Some(content) = abbrevs.get(&target) {
                    // Only expand when the character before the abbreviation is
                    // a word boundary — prevents false triggers inside words.
                    let prev = if length < n { Some(buf[n - length - 1]) } else { None };
                    if prev.map(is_word_char).unwrap_or(false) {
                        continue;
                    }
                    found = Some((target, content.clone()));
                    break;
                }
            }
            found
        };
        let Some((abbrev, content)) = found else { return };
        buf.clear();
        let me = self.clone();
        std::thread::spawn(move || me.perform_expansion(&abbrev, &content));
    }

    fn perform_expansion(&self, abbrev: &str, content: &str) {
        // Let the last typed character be fully registered before deleting it
        std::thread::sleep(Duration::from_millis(20));
        self.injecting.store(true, Ordering::SeqCst);
        press_backspace(abbrev.chars().count());
        let expanded = resolve_tokens(content, false, self.is_dark.load(Ordering::SeqCst));
        insert_text(&expanded);
        self.injecting.store(false, Ordering::SeqCst);
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

enum KeyChars {
    Chars(Vec<char>),
    Dead,
    None,
}

unsafe fn key_to_chars(kb: &KBDLLHOOKSTRUCT) -> KeyChars {
    let fg = GetForegroundWindow();
    let tid = if fg.is_null() { 0 } else { GetWindowThreadProcessId(fg, std::ptr::null_mut()) };
    let hkl = GetKeyboardLayout(tid);
    let mut state = [0u8; 256];
    for vk in [VK_SHIFT, VK_LSHIFT, VK_RSHIFT, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_MENU, VK_LMENU, VK_RMENU] {
        if (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 {
            state[vk as usize] = 0x80;
        }
    }
    if (GetKeyState(VK_CAPITAL as i32) & 1) != 0 {
        state[VK_CAPITAL as usize] = 0x01;
    }
    if (GetKeyState(VK_NUMLOCK as i32) & 1) != 0 {
        state[VK_NUMLOCK as usize] = 0x01;
    }
    let mut out = [0u16; 8];
    // flag 0x4: do not change keyboard state (keeps dead keys working for the target app)
    let n = ToUnicodeEx(kb.vkCode, kb.scanCode, state.as_ptr(), out.as_mut_ptr(), out.len() as i32, 0x4, hkl);
    if n < 0 {
        return KeyChars::Dead;
    }
    if n == 0 {
        return KeyChars::None;
    }
    let s = String::from_utf16_lossy(&out[..n as usize]);
    KeyChars::Chars(s.chars().collect())
}

unsafe extern "system" fn ll_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && (wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN) {
        if let Some(e) = ENGINE.get() {
            let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
            e.on_key_down(kb);
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe fn hook_thread(tx: std::sync::mpsc::Sender<bool>) {
    let Some(engine) = ENGINE.get() else {
        let _ = tx.send(false);
        return;
    };
    let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_proc), GetModuleHandleW(std::ptr::null()), 0);
    if hook.is_null() {
        let _ = tx.send(false);
        return;
    }
    engine.hook_tid.store(GetCurrentThreadId(), Ordering::SeqCst);
    engine.hook_alive.store(true, Ordering::SeqCst);
    let _ = tx.send(true);
    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    UnhookWindowsHookEx(hook);
    engine.hook_alive.store(false, Ordering::SeqCst);
}

// ── Input injection ─────────────────────────────────────────────────────────

fn key_input(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: INJECT_MAGIC },
        },
    }
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

fn tap_vk(vk: u16) {
    send(&[key_input(vk, 0, 0), key_input(vk, 0, KEYEVENTF_KEYUP)]);
}

fn press_backspace(n: usize) {
    for _ in 0..n {
        tap_vk(VK_BACK);
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// Insert `text`, honouring any {cursor} marker. Non-destructive.
fn insert_text(text: &str) {
    let mut text = text.to_string();
    let mut cursor_index = None;
    if let Some(byte_idx) = text.find("{cursor}") {
        cursor_index = Some(text[..byte_idx].chars().count());
        text = text.replace("{cursor}", "");
    }
    if text.is_empty() {
        return;
    }
    // Clipboard paste is atomic and never drops characters; typing is only a
    // fallback when the clipboard is unavailable.
    if paste_clipboard(&text).is_err() {
        type_reliable(&text);
    }
    if let Some(ci) = cursor_index {
        let n_left = text.chars().count().saturating_sub(ci);
        for _ in 0..n_left {
            tap_vk(VK_LEFT);
            std::thread::sleep(Duration::from_millis(4));
        }
    }
}

/// Type `text` character-by-character with small delays.
fn type_reliable(text: &str) {
    for ch in text.chars() {
        let mut buf = [0u16; 2];
        for unit in ch.encode_utf16(&mut buf).iter() {
            if *unit == '\n' as u16 {
                tap_vk(VK_RETURN);
            } else {
                send(&[
                    key_input(0, *unit, KEYEVENTF_UNICODE),
                    key_input(0, *unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                ]);
            }
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}

pub fn clipboard_text() -> String {
    arboard::Clipboard::new().and_then(|mut c| c.get_text()).unwrap_or_default()
}

/// Paste via the clipboard, restoring the previous contents afterwards.
fn paste_clipboard(text: &str) -> Result<(), arboard::Error> {
    let old_clip = clipboard_text();
    {
        let mut cb = arboard::Clipboard::new()?;
        cb.set_text(text.to_string())?;
    }
    std::thread::sleep(Duration::from_millis(60));
    // Press Ctrl+V with Ctrl held down for a stable paste
    send(&[key_input(VK_CONTROL, 0, 0)]);
    std::thread::sleep(Duration::from_millis(10));
    send(&[key_input(0x56, 0, 0)]);
    std::thread::sleep(Duration::from_millis(10));
    send(&[key_input(0x56, 0, KEYEVENTF_KEYUP)]);
    std::thread::sleep(Duration::from_millis(10));
    send(&[key_input(VK_CONTROL, 0, KEYEVENTF_KEYUP)]);
    std::thread::sleep(Duration::from_millis(60));
    if !old_clip.is_empty() {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(old_clip);
        }
    }
    Ok(())
}

// ── Token resolution ────────────────────────────────────────────────────────

static RE_DATETIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{datetime:([^}]*)\}").unwrap());
static RE_DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{date:([^}]*)\}").unwrap());
static RE_TIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{time:([^}]*)\}").unwrap());
pub static RE_FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{field:([^}]+)\}").unwrap());

pub fn strftime(now: &chrono::DateTime<chrono::Local>, fmt: &str, default: &str) -> String {
    let fmt = if fmt.trim().is_empty() { default } else { fmt.trim() };
    let items: Vec<Item> = StrftimeItems::new(fmt).collect();
    if items.iter().any(|i| matches!(i, Item::Error)) {
        return now.format(default).to_string();
    }
    now.format_with_items(items.into_iter()).to_string()
}

pub fn resolve_tokens(content: &str, preview: bool, is_dark: bool) -> String {
    let now = chrono::Local::now();

    // Parameterized tokens
    let content = RE_DATETIME.replace_all(content, |c: &regex::Captures| strftime(&now, &c[1], "%Y-%m-%d %H:%M"));
    let content = RE_DATE.replace_all(&content, |c: &regex::Captures| strftime(&now, &c[1], "%d %B %Y"));
    let content = RE_TIME.replace_all(&content, |c: &regex::Captures| strftime(&now, &c[1], "%H:%M"));
    let mut content = content.into_owned();

    // Plain tokens
    content = content.replace("{date}", &now.format("%d %B %Y").to_string());
    content = content.replace("{time}", &now.format("%I:%M %p").to_string());
    content = content.replace("{datetime}", &now.format("%Y-%m-%d %H:%M").to_string());

    if content.contains("{clipboard}") {
        content = content.replace("{clipboard}", &clipboard_text());
    }

    if preview {
        // Hide the caret marker in previews
        content = content.replace("{cursor}", "");
    }

    // {field:Label} — batch all fields (independent replacements)
    let matches: Vec<(usize, usize, String)> =
        RE_FIELD.captures_iter(&content).map(|c| {
            let m = c.get(0).unwrap();
            (m.start(), m.end(), c[1].to_string())
        }).collect();
    if !matches.is_empty() {
        if preview {
            content = RE_FIELD.replace_all(&content, "[$1]").into_owned();
        } else {
            let labels: Vec<String> = matches.iter().map(|m| m.2.clone()).collect();
            let results = crate::field_dialog::request(&labels, &content, is_dark);
            match results {
                Some(values) if !values.is_empty() => {
                    for (i, m) in matches.iter().enumerate().rev() {
                        let val = values.get(i).cloned().unwrap_or_default();
                        content.replace_range(m.0..m.1, &val);
                    }
                }
                _ => {
                    // User cancelled — drop the field tokens entirely
                    for m in matches.iter().rev() {
                        content.replace_range(m.0..m.1, "");
                    }
                }
            }
        }
    }
    content
}
