//! Port of viewmodels/updater.py — check GitHub Releases and install updates.
//!
//! Portable layout:
//!   ShobdoCalok.exe
//!   AppData/        <- user data (NEVER touched: snippets, settings, tokens)
//!
//!  1. Query the GitHub Releases API for the newest release.
//!  2. Compare its version against APP_VERSION.
//!  3. Download the ZIP asset into AppData/update/ (multi-connection, resumable).
//!  4. Verify SHA256 against the .sha256 asset.
//!  5. Extract with progress, then hand off to the GUI updater (this exe in
//!     `--updater` mode, run from a temp copy) which swaps files and relaunches.

use parking_lot::Mutex;
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::events::{emit, Event};
use crate::version::{APP_VERSION, GITHUB_OWNER, GITHUB_REPO};

const MAX_RETRIES: usize = 3;
const RETRY_BACKOFF: [u64; 3] = [1, 3, 8];
const UA: &str = "ShobdoCalok-Updater";

fn releases_api() -> String {
    format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest")
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(30)))
        .build()
        .into()
}

fn find_github_token() -> String {
    for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(t) = std::env::var(var) {
            if !t.trim().is_empty() {
                return t.trim().to_string();
            }
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        let hosts = Path::new(&home).join(".config").join("gh").join("hosts.yml");
        if let Ok(text) = std::fs::read_to_string(hosts) {
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("oauth_token:") {
                    return rest.trim().trim_matches(|c| c == '\'' || c == '"').to_string();
                }
            }
        }
    }
    use std::os::windows::process::CommandExt;
    if let Ok(out) = std::process::Command::new("gh")
        .args(["auth", "token"])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
    {
        let tok = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !tok.is_empty() {
            return tok;
        }
    }
    String::new()
}

fn api_get(url: &str) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    let mut req = agent()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", UA)
        .config()
        .timeout_global(Some(Duration::from_secs(30)))
        .build();
    let tok = find_github_token();
    if !tok.is_empty() {
        req = req.header("Authorization", format!("Bearer {tok}"));
    }
    req.call()
}

fn dl_get(url: &str, range: Option<String>) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    // No compression: the CDN must send the exact bytes or SHA256 breaks.
    let mut req = agent().get(url).header("User-Agent", UA).header("Accept-Encoding", "identity");
    if let Some(r) = range {
        req = req.header("Range", r);
    }
    req.call()
}

fn header(resp: &ureq::http::Response<ureq::Body>, name: &str) -> String {
    resp.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or("").to_string()
}

pub fn version_tuple(v: &str) -> Vec<u32> {
    let re = Regex::new(r"(\d+)\.(\d+)(?:\.(\d+))?").unwrap();
    match re.captures(v) {
        Some(c) => (1..=3).filter_map(|i| c.get(i)).map(|m| m.as_str().parse().unwrap_or(0)).collect(),
        None => vec![0, 0, 0],
    }
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[derive(Clone, Default, Debug)]
pub struct UpdaterState {
    pub checking: bool,
    pub downloading: bool,
    pub applying: bool,
    pub progress: f32,
    pub latest_version: String,
    pub update_available: bool,
    pub download_ready: bool,
    pub status_message_text: String,
}

#[derive(Default)]
struct Inner {
    asset_url: String,
    asset_name: String,
    checksum_url: String,
    release_url: String,
    zip_path: PathBuf,
    checksum_path: PathBuf,
}

pub struct Updater {
    data_dir: PathBuf,
    update_dir: PathBuf,
    state: Mutex<UpdaterState>,
    inner: Mutex<Inner>,
    download_lock: Mutex<()>,
}

impl Updater {
    pub fn new(data_dir: &Path) -> Arc<Self> {
        Arc::new(Self {
            data_dir: data_dir.to_path_buf(),
            update_dir: data_dir.join("update"),
            state: Mutex::new(UpdaterState::default()),
            inner: Mutex::new(Inner::default()),
            download_lock: Mutex::new(()),
        })
    }

    pub fn state(&self) -> UpdaterState {
        self.state.lock().clone()
    }

    pub fn current_version(&self) -> &'static str {
        APP_VERSION
    }

    fn is_frozen(&self) -> bool {
        !cfg!(debug_assertions)
    }

    fn app_root(&self) -> PathBuf {
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default()
    }

    fn emit_status(&self, msg: &str) {
        self.state.lock().status_message_text = msg.to_string();
        on_status_side_effects(msg);
        emit(Event::UpdaterStatus(msg.to_string()));
    }

    fn set_checking(&self, v: bool) {
        self.state.lock().checking = v;
        crate::request_repaint();
    }
    fn set_downloading(&self, v: bool) {
        self.state.lock().downloading = v;
        crate::request_repaint();
    }
    fn set_applying(&self, v: bool) {
        self.state.lock().applying = v;
        crate::request_repaint();
    }
    fn set_progress(&self, v: f32) {
        self.state.lock().progress = v;
        crate::request_repaint();
    }

    fn set_paths_for_version(&self, version: &str, asset_name: &str) {
        let safe = if asset_name.is_empty() {
            format!("ShobdoCalok-v{version}")
        } else {
            asset_name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_else(|| asset_name.to_string())
        };
        let mut inner = self.inner.lock();
        inner.zip_path = self.update_dir.join(format!("{safe}.zip"));
        inner.checksum_path = self.update_dir.join(format!("{safe}.sha256"));
    }

    fn cleanup_update_dir(&self) {
        let _ = std::fs::remove_dir_all(&self.update_dir);
    }

    fn write_update_log(&self, message: &str, kv: &[(&str, String)]) {
        let log_path = self.data_dir.join("update_debug.log");
        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let mut lines = vec![format!("\n{}", "=".repeat(70)), format!("[{ts}] v{APP_VERSION}"), format!("Message: {message}")];
        for (k, v) in kv {
            let mut sv = v.clone();
            if sv.chars().count() > 500 {
                sv = sv.chars().take(500).collect::<String>() + "...";
            }
            lines.push(format!("{k}: {sv}"));
        }
        lines.push("Runtime: Rust/egui".into());
        lines.push(format!("Frozen: {}", self.is_frozen()));
        lines.push(format!("App root: {}", self.app_root().display()));
        lines.push(format!("Update dir: {}", self.update_dir.display()));
        lines.push(format!("{}\n", "=".repeat(70)));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = f.write_all(lines.join("\n").as_bytes());
        }
    }

    /// Check ZIP exists, has reasonable size, and is not corrupt.
    fn validate_zip(&self, path: &Path) -> bool {
        let Ok(meta) = std::fs::metadata(path) else { return false };
        if meta.len() < 1000 {
            return false;
        }
        let Ok(f) = std::fs::File::open(path) else { return false };
        let Ok(mut zip) = zip::ZipArchive::new(f) else { return false };
        for i in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(i) else { return false };
            // Reading to the end verifies the CRC (like ZipFile.testzip()).
            if std::io::copy(&mut entry, &mut std::io::sink()).is_err() {
                return false;
            }
        }
        true
    }

    // ── Check for updates ────────────────────────────────────────────────────

    pub fn check_for_update(self: &Arc<Self>) {
        {
            let s = self.state.lock();
            if s.checking || s.downloading || s.applying {
                return;
            }
        }
        let me = self.clone();
        std::thread::spawn(move || me.do_check());
    }

    fn do_check(&self) {
        self.set_checking(true);
        self.emit_status("Checking for updates…");
        let result: Result<(), String> = (|| {
            let mut resp = api_get(&releases_api()).map_err(|e| e.to_string())?;
            let code = resp.status().as_u16();
            if code == 404 {
                self.emit_status("No releases found yet.");
                return Ok(());
            }
            if code != 200 {
                self.emit_status(&format!("Update check failed (HTTP {code})."));
                return Ok(());
            }
            let data: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
            let tag = data.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
            let latest = tag.trim_start_matches('v').to_string();
            let assets = data.get("assets").and_then(|a| a.as_array()).cloned().unwrap_or_default();
            let find = |ext: &str| {
                assets
                    .iter()
                    .find(|a| a.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase().ends_with(ext))
                    .cloned()
            };
            let zip_asset = find(".zip");
            let checksum_asset = find(".sha256");
            let s = |v: &Option<Value>, k: &str| {
                v.as_ref().and_then(|a| a.get(k)).and_then(|x| x.as_str()).unwrap_or("").to_string()
            };
            {
                let mut inner = self.inner.lock();
                inner.release_url = data.get("html_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                inner.asset_url = s(&zip_asset, "browser_download_url");
                inner.asset_name = s(&zip_asset, "name");
                inner.checksum_url = s(&checksum_asset, "browser_download_url");
            }
            self.state.lock().latest_version = latest.clone();
            let asset_name = self.inner.lock().asset_name.clone();
            self.set_paths_for_version(&latest, &asset_name);

            if version_tuple(&latest) > version_tuple(APP_VERSION) {
                let zip_path = self.inner.lock().zip_path.clone();
                if self.validate_zip(&zip_path) {
                    self.emit_status(&format!("Update v{latest} already downloaded — ready to install."));
                    self.state.lock().download_ready = true;
                } else {
                    self.cleanup_update_dir();
                    let _ = std::fs::create_dir_all(&self.update_dir);
                    self.state.lock().download_ready = false;
                }
                self.state.lock().update_available = true;
                emit(Event::UpdateAvailableChanged);
                self.emit_status(&format!("Update available: version {latest} (you have {APP_VERSION})."));
            } else {
                self.state.lock().update_available = false;
                emit(Event::UpdateAvailableChanged);
                self.emit_status(&format!("You're up to date (version {APP_VERSION})."));
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.emit_status(&format!("Update check failed: {e}"));
        }
        self.set_checking(false);
    }

    // ── Download (multi-connection) ─────────────────────────────────────────

    fn download_with_resume(&self, url: &str, dest: &Path) -> bool {
        const THREADS: u64 = 8;
        for attempt in 0..MAX_RETRIES {
            let res: Result<bool, String> = (|| {
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                }
                // GET to determine total size (follows redirects, unlike HEAD)
                let probe = dl_get(url, None).map_err(|e| e.to_string())?;
                let total_size: u64 = header(&probe, "content-length").parse().unwrap_or(0);
                let accept_ranges = header(&probe, "accept-ranges") == "bytes";
                drop(probe);

                if total_size == 0 || !accept_ranges {
                    return Ok(self.download_single_thread(url, dest));
                }

                let chunk_size = (4u64 << 20).max(total_size / THREADS + 1);
                let mut ranges = vec![];
                let mut start = 0u64;
                while start < total_size {
                    let end = (start + chunk_size - 1).min(total_size - 1);
                    ranges.push((start, end));
                    start = end + 1;
                }
                let tmp_dir = PathBuf::from(format!("{}.chunks", dest.display()));
                std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

                let mut existing = 0u64;
                for i in 0..ranges.len() {
                    existing += std::fs::metadata(tmp_dir.join(i.to_string())).map(|m| m.len()).unwrap_or(0);
                }
                self.emit_status(&format!("Downloading with {} connections…", ranges.len()));
                let done = Arc::new(AtomicU64::new(existing));
                let t0 = Instant::now();

                let handles: Vec<_> = ranges
                    .iter()
                    .enumerate()
                    .map(|(i, &(cs, ce))| {
                        let url = url.to_string();
                        let chunk_file = tmp_dir.join(i.to_string());
                        let done = done.clone();
                        std::thread::spawn(move || -> bool {
                            let sz = std::fs::metadata(&chunk_file).map(|m| m.len()).unwrap_or(0);
                            let local_start = cs + sz;
                            if local_start > ce {
                                return true;
                            }
                            let Ok(resp) = dl_get(&url, Some(format!("bytes={local_start}-{ce}"))) else { return false };
                            let code = resp.status().as_u16();
                            if code != 200 && code != 206 {
                                return false;
                            }
                            let file = if sz > 0 {
                                std::fs::OpenOptions::new().append(true).open(&chunk_file)
                            } else {
                                std::fs::File::create(&chunk_file)
                            };
                            let Ok(mut file) = file else { return false };
                            let mut reader = resp.into_body().into_reader();
                            let mut buf = vec![0u8; 256 * 1024];
                            loop {
                                match reader.read(&mut buf) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if file.write_all(&buf[..n]).is_err() {
                                            return false;
                                        }
                                        done.fetch_add(n as u64, Ordering::SeqCst);
                                    }
                                    Err(_) => return false,
                                }
                            }
                            true
                        })
                    })
                    .collect();

                let mut last_t = Instant::now();
                let mut last_b = existing;
                while !handles.iter().all(|h| h.is_finished()) {
                    let now = Instant::now();
                    let cur = done.load(Ordering::SeqCst);
                    let dt = now.duration_since(last_t).as_secs_f64();
                    let speed = if dt > 0.0 { (cur.saturating_sub(last_b)) as f64 / dt / (1u64 << 20) as f64 } else { 0.0 };
                    last_t = now;
                    last_b = cur;
                    self.set_progress((cur as f32 / total_size as f32).min(0.99));
                    self.emit_status(&format!(
                        "Downloading… {}/{} MB ({speed:.1} MB/s)",
                        cur >> 20,
                        total_size >> 20
                    ));
                    std::thread::sleep(Duration::from_millis(500));
                }
                let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap_or(false)).collect();
                self.set_progress((done.load(Ordering::SeqCst) as f32 / total_size as f32).min(0.99));

                if !results.iter().all(|r| *r) {
                    self.emit_status("Some chunks failed. Retrying…");
                    let _ = std::fs::remove_dir_all(&tmp_dir);
                    return Err("__retry__".into());
                }

                self.emit_status("Reassembling download…");
                {
                    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
                    for i in 0..ranges.len() {
                        let mut cf = std::fs::File::open(tmp_dir.join(i.to_string())).map_err(|e| e.to_string())?;
                        std::io::copy(&mut cf, &mut out).map_err(|e| e.to_string())?;
                    }
                }
                let _ = std::fs::remove_dir_all(&tmp_dir);

                if !self.validate_zip(dest) {
                    self.emit_status("Downloaded file is corrupted.");
                    let _ = std::fs::remove_file(dest);
                    return Err("__retry__".into());
                }
                let elapsed = t0.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 { total_size as f64 / elapsed / (1u64 << 20) as f64 } else { 0.0 };
                self.set_progress(1.0);
                self.emit_status(&format!("Download complete ({speed:.1} MB/s)"));
                Ok(true)
            })();
            match res {
                Ok(v) => return v,
                Err(e) if e == "__retry__" => {
                    if attempt < MAX_RETRIES - 1 {
                        std::thread::sleep(Duration::from_secs(RETRY_BACKOFF[attempt]));
                        continue;
                    }
                    return false;
                }
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        let wait = RETRY_BACKOFF[attempt];
                        self.emit_status(&format!(
                            "Download failed: {e}. Retrying in {wait}s… ({}/{MAX_RETRIES})",
                            attempt + 1
                        ));
                        std::thread::sleep(Duration::from_secs(wait));
                    } else {
                        self.emit_status(&format!("Download failed after {MAX_RETRIES} attempts: {e}"));
                        return false;
                    }
                }
            }
        }
        false
    }

    /// Fallback single-threaded download with resume.
    fn download_single_thread(&self, url: &str, dest: &Path) -> bool {
        for attempt in 0..MAX_RETRIES {
            let res: Result<bool, String> = (|| {
                let mut existing = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
                let range = (existing > 0).then(|| format!("bytes={existing}-"));
                let resp = dl_get(url, range).map_err(|e| e.to_string())?;
                let code = resp.status().as_u16();
                if code == 416 {
                    self.set_progress(1.0);
                    return Ok(true);
                }
                if code == 200 {
                    existing = 0;
                } else if code != 206 {
                    return Err(format!("HTTP {code}"));
                }
                let total = header(&resp, "content-length").parse::<u64>().unwrap_or(0) + existing;
                let mut done = existing;
                let mut t0 = Instant::now();
                let mut bytes_since = 0u64;
                let mut file = if existing > 0 && code == 206 {
                    std::fs::OpenOptions::new().append(true).open(dest)
                } else {
                    std::fs::File::create(dest)
                }
                .map_err(|e| e.to_string())?;
                let mut reader = resp.into_body().into_reader();
                let mut buf = vec![0u8; 256 * 1024];
                loop {
                    let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                    done += n as u64;
                    bytes_since += n as u64;
                    if total > 0 {
                        self.set_progress((done as f32 / total as f32).min(1.0));
                    }
                    let el = t0.elapsed().as_secs_f64();
                    if el >= 0.5 && bytes_since > 0 {
                        let speed = bytes_since as f64 / el / (1u64 << 20) as f64;
                        self.emit_status(&format!("Downloading… {}/{} MB ({speed:.1} MB/s)", done >> 20, total >> 20));
                        t0 = Instant::now();
                        bytes_since = 0;
                    }
                }
                drop(file);
                if !self.validate_zip(dest) {
                    self.emit_status("Downloaded file is corrupted.");
                    return Err("__retry__".into());
                }
                Ok(true)
            })();
            match res {
                Ok(v) => return v,
                Err(e) => {
                    if attempt < MAX_RETRIES - 1 {
                        std::thread::sleep(Duration::from_secs(RETRY_BACKOFF[attempt]));
                    } else {
                        if e != "__retry__" {
                            self.emit_status(&format!("Download failed: {e}"));
                        }
                        return false;
                    }
                }
            }
        }
        false
    }

    // ── Extract with progress ───────────────────────────────────────────────

    fn extract_with_progress(&self, zip_path: &Path, extract_to: &Path) -> bool {
        let res: Result<bool, String> = (|| {
            let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
            let mut zip = zip::ZipArchive::new(f).map_err(|_| "__bad__".to_string())?;
            let total = zip.len();
            if total == 0 {
                self.emit_status("Update archive is empty.");
                return Ok(false);
            }
            for i in 0..total {
                let mut entry = zip.by_index(i).map_err(|_| "__bad__".to_string())?;
                let Some(rel) = entry.enclosed_name() else { continue };
                let out = extract_to.join(rel);
                if entry.is_dir() {
                    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                } else {
                    if let Some(p) = out.parent() {
                        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                    }
                    let mut of = std::fs::File::create(&out).map_err(|e| e.to_string())?;
                    std::io::copy(&mut entry, &mut of).map_err(|e| e.to_string())?;
                }
                self.set_progress((i + 1) as f32 / total as f32);
                if (i + 1) % 50 == 0 || i + 1 == total {
                    self.emit_status(&format!("Extracting… {}/{total} files", i + 1));
                }
            }
            Ok(true)
        })();
        match res {
            Ok(v) => v,
            Err(e) if e == "__bad__" => {
                self.emit_status("Downloaded file is corrupted. Please re-download.");
                false
            }
            Err(e) => {
                self.emit_status(&format!("Extraction failed: {e}"));
                false
            }
        }
    }

    // ── Verify checksum ─────────────────────────────────────────────────────

    fn verify_checksum(&self, zip_path: &Path) -> bool {
        let (checksum_url, asset_url, checksum_path) = {
            let i = self.inner.lock();
            (i.checksum_url.clone(), i.asset_url.clone(), i.checksum_path.clone())
        };
        if checksum_url.is_empty() {
            self.emit_status("No checksum file available — skipping verification.");
            return true;
        }
        for verify_attempt in 0..2 {
            self.emit_status("Verifying checksum…");
            let text = match api_get(&checksum_url) {
                Ok(mut r) if r.status().as_u16() == 200 => r.body_mut().read_to_string().unwrap_or_default(),
                Ok(_) => {
                    self.emit_status("Could not download checksum — skipping verification.");
                    return true;
                }
                Err(e) => {
                    self.emit_status(&format!("Checksum verification failed: {e} — proceeding anyway."));
                    return true;
                }
            };
            let _ = std::fs::write(&checksum_path, &text);
            let text = text.trim();
            if text.is_empty() {
                self.emit_status("Checksum file is empty — skipping verification.");
                return true;
            }
            let expected = text.split_whitespace().next().unwrap_or("").trim().to_lowercase();
            if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
                self.emit_status("Invalid checksum format — skipping.");
                return true;
            }
            let actual = match sha256_file(zip_path) {
                Ok(a) => a,
                Err(e) => {
                    self.emit_status(&format!("Checksum verification failed: {e} — proceeding anyway."));
                    return true;
                }
            };
            if actual == expected {
                self.emit_status("Checksum verified");
                return true;
            }
            if verify_attempt == 0 {
                self.emit_status("Checksum mismatch — re-downloading…");
                let _ = std::fs::remove_file(zip_path);
                if self.download_with_resume(&asset_url, zip_path) {
                    continue;
                }
            }
            self.emit_status("Checksum mismatch but proceeding — file may still work.");
            return true;
        }
        true
    }

    // ── Download + install ──────────────────────────────────────────────────

    pub fn download_and_install(self: &Arc<Self>) {
        {
            let s = self.state.lock();
            if s.checking || s.downloading || s.applying {
                return;
            }
        }
        let me = self.clone();
        std::thread::spawn(move || me.do_download_and_install());
    }

    fn do_download_and_install(&self) {
        let Some(_guard) = self.download_lock.try_lock() else {
            self.emit_status("An update is already in progress.");
            return;
        };
        let res: Result<(), String> = (|| {
            if !self.is_frozen() {
                self.emit_status("Updates can only be installed in the packaged app.");
                return Ok(());
            }
            let (asset_url, asset_name, zip_path) = {
                let i = self.inner.lock();
                (i.asset_url.clone(), i.asset_name.clone(), i.zip_path.clone())
            };
            if asset_url.is_empty() {
                self.emit_status("No update asset found to download.");
                return Ok(());
            }
            self.write_update_log(
                &format!("Starting update from v{APP_VERSION}"),
                &[("asset_url", asset_url.clone()), ("asset_name", asset_name), ("zip_path", zip_path.display().to_string())],
            );
            std::fs::create_dir_all(&self.update_dir).map_err(|e| e.to_string())?;

            let ready = self.state.lock().download_ready;
            if ready && self.validate_zip(&zip_path) {
                self.emit_status("Using previously downloaded update.");
                self.set_progress(1.0);
            } else if zip_path.exists() && self.validate_zip(&zip_path) {
                self.emit_status("Using cached update file.");
                self.set_progress(1.0);
            } else {
                self.set_downloading(true);
                self.set_progress(0.0);
                self.emit_status("Downloading update…");
                if !self.download_with_resume(&asset_url, &zip_path) {
                    return Ok(());
                }
                self.set_progress(1.0);
            }

            if !self.verify_checksum(&zip_path) {
                self.emit_status("Update aborted due to checksum failure.");
                return Ok(());
            }
            self.set_downloading(false);

            self.emit_status("Extracting update…");
            let extract_to = self.update_dir.join("extracted");
            let _ = std::fs::remove_dir_all(&extract_to);
            std::fs::create_dir_all(&extract_to).map_err(|e| e.to_string())?;
            self.set_progress(0.0);
            if !self.extract_with_progress(&zip_path, &extract_to) {
                return Ok(());
            }
            self.set_progress(1.0);

            let Some(new_root) = find_new_app_root(&extract_to) else {
                self.emit_status("Update file did not contain the app. Please re-download.");
                self.write_update_log(
                    "FAILED: no app root found in extracted update",
                    &[("new_root", "<none>".into()), ("extract_to", extract_to.display().to_string())],
                );
                return Ok(());
            };

            self.emit_status("Applying update — restarting…");
            self.set_applying(true);
            self.set_progress(0.0);
            let file_count = walk_files(&new_root).len();
            self.write_update_log(
                &format!("Update ready to apply: {file_count} files in extracted update"),
                &[
                    ("new_root", new_root.display().to_string()),
                    ("extract_to", extract_to.display().to_string()),
                    ("file_count", file_count.to_string()),
                ],
            );
            self.emit_status("Preparing update files…");
            self.set_progress(0.2);
            std::thread::sleep(Duration::from_millis(300));
            self.emit_status("Launching updater…");
            self.set_progress(0.5);
            self.launch_applier(&new_root, &extract_to);
            Ok(())
        })();
        if let Err(e) = res {
            self.emit_status(&format!("Update failed: {e}"));
        }
        self.set_downloading(false);
        self.set_applying(false);
        self.set_checking(false);
    }

    fn launch_applier(&self, new_root: &Path, extract_to: &Path) {
        let app_root = self.app_root();
        let app_exe = std::env::current_exe().unwrap_or_default();
        // The GUI updater is this same exe in `--updater` mode, run from a temp
        // copy so the real exe can be replaced while it runs.
        let updater_exe = std::env::temp_dir().join("ShobdoCalok-Updater.exe");
        let copied = std::fs::copy(&app_exe, &updater_exe).is_ok();
        self.write_update_log(
            &format!("Launching applier: GUI={copied}"),
            &[
                ("updater_exe", updater_exe.display().to_string()),
                ("new_root", new_root.display().to_string()),
                ("app_root", app_root.display().to_string()),
                ("extract_to", extract_to.display().to_string()),
                ("update_dir", self.update_dir.display().to_string()),
            ],
        );
        let launched = copied
            && std::process::Command::new(&updater_exe)
                .arg("--updater")
                .arg(new_root)
                .arg(&app_root)
                .arg(&app_exe)
                .arg(extract_to)
                .arg(&self.update_dir)
                .arg(std::process::id().to_string())
                .current_dir(&app_root)
                .spawn()
                .is_ok();
        if launched {
            self.write_update_log("GUI updater launched successfully", &[]);
        } else {
            self.write_update_log("GUI updater not available, falling back to batch", &[]);
            self.launch_batch_applier(new_root, extract_to);
        }
        std::thread::sleep(Duration::from_secs(1));
        crate::tray::remove();
        std::process::exit(0);
    }

    /// Fallback batch script updater (identical flow to the original).
    fn launch_batch_applier(&self, new_root: &Path, extract_to: &Path) {
        let app_root = self.app_root();
        let ud = &self.update_dir;
        let bat = ud.join("apply_update.bat");
        let vbs = ud.join("apply_update.vbs");
        let log_file = ud.join("update_log.txt");
        let script = format!(
            r#"@echo off
setlocal enabledelayedexpansion
title ShobdoCalok Updater
mode con: cols=70 lines=30
color 0B

cls
echo.
echo  ============================================
echo       ShobdoCalok Updater v{ver}
echo  ============================================
echo.
set "APP={app}"
set "NEW={new}"
set "UPDATE={upd}"
set "TMPEXTRACT={ext}"
set "LOG={log}"

echo ===== ShobdoCalok Updater ===== >> "%LOG%"
echo Timestamp: %DATE% %TIME% >> "%LOG%"
echo APP=%APP% >> "%LOG%"
echo NEW=%NEW% >> "%LOG%"
echo. >> "%LOG%"

echo  [1/4] Stopping ShobdoCalok...
taskkill /F /IM ShobdoCalok.exe 2>nul >> "%LOG%"
echo  Waiting for files to unlock...
timeout /t 3 /nobreak >nul
echo  [1/4] Process stopped.
echo.

echo  [2/4] Checking update files...
echo  Source: %NEW%
echo [2/4] Checking source: %NEW% >> "%LOG%"
if not exist "%NEW%\ShobdoCalok.exe" goto no_files
echo  [2/4] Found: ShobdoCalok.exe ^(packaged build^)
echo [2/4] Source OK ^(packaged^) >> "%LOG%"
echo.

echo  [3/4] Installing update...
echo  Copying: %NEW%
echo       To: %APP%
echo [3/4] Robocopy: %NEW% --^> %APP% >> "%LOG%"
robocopy "%NEW%" "%APP%" /E /XD AppData /XF "*.log" /R:1 /W:1
set RC=!errorlevel!
echo  [3/4] Robocopy exit code: !RC!
echo [3/4] Robocopy exit code: !RC! >> "%LOG%"

if !RC! GEQ 8 (
    echo  [3/4] robocopy failed, trying xcopy as fallback...
    echo [3/4] Trying xcopy fallback... >> "%LOG%"
    xcopy "%NEW%\*" "%APP%\" /E /Y /Q /I
    set RC2=!errorlevel!
    echo  [3/4] xcopy exit code: !RC2!
    echo [3/4] xcopy exit code: !RC2! >> "%LOG%"
)

echo.
echo  Verifying copy...
if exist "%APP%\ShobdoCalok.exe" (
    echo  [OK] ShobdoCalok.exe found in app directory
) else (
    echo  [WARN] ShobdoCalok.exe NOT found in app directory!
    echo  Listing app directory:
    dir "%APP%" /b
)
echo.

echo  [4/4] Starting ShobdoCalok...
echo [4/4] Starting app... >> "%LOG%"
start "" "%APP%\ShobdoCalok.exe"
echo  [4/4] App started!
echo [4/4] App started. >> "%LOG%"

echo.
echo  ============================================
echo   Update complete!
echo   Closing in 5 seconds...
echo  ============================================
timeout /t 5 /nobreak >nul
goto cleanup

:no_files
echo  [2/4] ERROR: No update files found in:
echo  %NEW%
echo.
echo  Contents of extracted folder:
dir "%NEW%" /b 2>nul
echo.
echo [2/4] ERROR: No app found in: %NEW% >> "%LOG%"
dir "%NEW%" /b >> "%LOG%" 2>nul
echo.
echo  Press any key to close...
pause >nul
goto cleanup

:cleanup
echo Cleaning up... >> "%LOG%"
if exist "%TMPEXTRACT%" rmdir /s /q "%TMPEXTRACT%"
if exist "%UPDATE%" rmdir /s /q "%UPDATE%"
del "%~f0" 2>nul
"#,
            ver = APP_VERSION,
            app = app_root.display(),
            new = new_root.display(),
            upd = ud.display(),
            ext = extract_to.display(),
            log = log_file.display(),
        );
        let _ = std::fs::write(&bat, script.replace('\n', "\r\n"));
        let vbs_script = format!(
            "Set objShell = CreateObject(\"WScript.Shell\")\nobjShell.Run \"cmd /c \"\"{}\"\"\", 1, False\n",
            bat.display()
        );
        let _ = std::fs::write(&vbs, vbs_script);
        crate::win::open_url(&vbs.display().to_string());
    }

    pub fn release_page_url(&self) -> String {
        let r = self.inner.lock().release_url.clone();
        if r.is_empty() { releases_api() } else { r }
    }
}

/// main.py `_on_updater_status`: tray balloons + bring the window forward.
fn on_status_side_effects(msg: &str) {
    if msg.is_empty() {
        return;
    }
    let lower = msg.to_lowercase();
    if lower.contains("available") {
        crate::win::show_main();
        crate::tray::notify("Shobdo Calok — Update Available", msg, false, 8000);
    } else if lower.contains("up to date") {
        crate::tray::notify("Shobdo Calok", msg, false, 3000);
    } else if lower.contains("failed") || lower.contains("error") {
        crate::win::show_main();
        crate::tray::notify("Shobdo Calok — Update Error", msg, true, 5000);
    }
}

pub fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().map(|n| n == "AppData").unwrap_or(false) {
                        continue;
                    }
                    stack.push(p);
                } else {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn find_new_app_root(extract_to: &Path) -> Option<PathBuf> {
    let mut candidates = vec![];
    let mut stack = vec![extract_to.to_path_buf()];
    while let Some(d) = stack.pop() {
        if d.join("ShobdoCalok.exe").is_file() {
            candidates.push(d.clone());
        }
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                }
            }
        }
    }
    candidates.into_iter().min_by_key(|p| p.components().count())
}

/// Apply deferred updates from a previous run (files that were locked while
/// the updater copied them). Format: `src|dst` per line.
pub fn apply_deferred_updates(data_dir: &Path) {
    let file = data_dir.join("deferred_updates.txt");
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let mut applied = 0;
    for line in text.lines() {
        let line = line.trim();
        let Some((src, dst)) = line.split_once('|') else { continue };
        if !Path::new(src).exists() {
            continue;
        }
        let real_dst = dst.strip_suffix(".new").unwrap_or(dst);
        let _ = std::fs::remove_file(real_dst);
        match std::fs::copy(src, real_dst) {
            Ok(_) => {
                if dst != real_dst {
                    let _ = std::fs::remove_file(dst);
                }
                applied += 1;
                eprintln!("Applied deferred update: {real_dst}");
            }
            Err(e) => eprintln!("Failed to apply deferred update {dst}: {e}"),
        }
    }
    if applied > 0 {
        eprintln!("Applied {applied} deferred update(s)");
    }
    let _ = std::fs::remove_file(file);
}
