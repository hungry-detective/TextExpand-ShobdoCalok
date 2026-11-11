//! Port of viewmodels/google_drive.py — Google login + snippet backup/restore
//! via the Drive API (appDataFolder, so nothing clutters the user's Drive).

use parking_lot::Mutex;
use rand::Rng;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::events::{emit, BackupItem, Event};
use crate::store::SnippetStore;

const CLIENT_SECRET_FILE: &str = "client_secret.json";
/// `client_secret.json` from the project root at build time (empty if absent),
/// so the packaged exe works like the PyInstaller bundle did.
const EMBEDDED_CLIENT_SECRET: &str = include_str!(concat!(env!("OUT_DIR"), "/embedded_client_secret.json"));
const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const AUTH_URI: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const SCOPE: &str = "https://www.googleapis.com/auth/drive.appdata";
const DRIVE_FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const DRIVE_ABOUT_URL: &str = "https://www.googleapis.com/drive/v3/about";
const BACKUP_FILE_NAME: &str = "ShobdoCalok.snippets.json";
const BACKUP_HISTORY_PREFIX: &str = "ShobdoCalok-backup-";
const BACKUP_META_NAME: &str = "ShobdoCalok.backup_meta.json";
const MAX_BACKUPS: usize = 10;

type R<T> = Result<T, String>;

fn now_secs() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(timeout)))
        .build()
        .into()
}

fn read_json(resp: &mut ureq::http::Response<ureq::Body>) -> Value {
    resp.body_mut().read_json::<Value>().unwrap_or(Value::Null)
}

fn local_stamp() -> String {
    chrono::Local::now().format("%b %d, %Y %I:%M %p").to_string()
}

/// datetime.fromisoformat(mt.replace("Z","+00:00")).strftime(fmt)
fn fmt_iso(mt: &str, fmt: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc3339(mt).ok().map(|d| d.format(fmt).to_string())
}

#[derive(Clone, Default, Debug)]
pub struct DriveState {
    pub logged_in: bool,
    pub account_name: String,
    pub last_backup: String,
    pub busy: bool,
}

pub struct Drive {
    data_dir: PathBuf,
    token_path: PathBuf,
    creds: Mutex<Option<Value>>,
    state: Mutex<DriveState>,
    login_cancelled: AtomicBool,
    store: Arc<Mutex<SnippetStore>>,
}

impl Drive {
    pub fn new(data_dir: &Path, store: Arc<Mutex<SnippetStore>>) -> Arc<Self> {
        let d = Arc::new(Self {
            data_dir: data_dir.to_path_buf(),
            token_path: data_dir.join("google_token.json"),
            creds: Mutex::new(None),
            state: Mutex::new(DriveState::default()),
            login_cancelled: AtomicBool::new(false),
            store,
        });
        d.load_last_backup();
        // Loading creds may refresh the token over the network — do it off the UI thread.
        let me = d.clone();
        std::thread::spawn(move || me.load_creds());
        d
    }

    pub fn state(&self) -> DriveState {
        self.state.lock().clone()
    }

    // ── Token persistence ───────────────────────────────────────────────────

    fn load_creds(&self) {
        let Ok(text) = std::fs::read_to_string(&self.token_path) else { return };
        let Ok(data) = serde_json::from_str::<Value>(&text) else { return };
        if data.get("refresh_token").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false) {
            let account = data.get("account").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let valid = data.get("access_token").is_some()
                && data.get("expires_at").and_then(|v| v.as_f64()).unwrap_or(0.0) > now_secs();
            *self.creds.lock() = Some(data);
            {
                let mut s = self.state.lock();
                s.account_name = account;
                s.logged_in = true;
            }
            if !valid && self.refresh_token().is_err() {
                *self.creds.lock() = None;
                self.state.lock().logged_in = false;
            }
            crate::request_repaint();
        }
    }

    fn save_creds(&self) {
        if let Some(c) = self.creds.lock().as_ref() {
            let _ = std::fs::write(&self.token_path, serde_json::to_string_pretty(c).unwrap_or_default());
        }
    }

    fn load_last_backup(&self) {
        let t = std::fs::read_to_string(self.data_dir.join("last_backup.txt")).unwrap_or_default();
        self.state.lock().last_backup = t.trim().to_string();
    }

    fn set_last_backup(&self, text: &str) {
        self.state.lock().last_backup = text.to_string();
        let _ = std::fs::write(self.data_dir.join("last_backup.txt"), text);
        crate::request_repaint();
    }

    fn set_busy(&self, b: bool) {
        self.state.lock().busy = b;
        crate::request_repaint();
    }

    fn status(&self, msg: &str) {
        emit(Event::DriveStatus(msg.to_string()));
    }

    // ── Auth primitives ─────────────────────────────────────────────────────

    fn parse_secret(text: &str) -> Option<Value> {
        let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        let inst = v.get("installed").or_else(|| v.get("web"))?;
        inst.get("client_id").and_then(|c| c.as_str()).filter(|c| !c.is_empty() && !c.starts_with("YOUR"))?;
        Some(inst.clone())
    }

    /// Locate the Google OAuth client. Search order: next to the exe (and
    /// `_internal\`, like the PyInstaller build), AppData\, the working
    /// directory, the copy compiled into the exe, and finally ask the user to
    /// pick the file once (it is then saved into AppData\).
    fn client_secrets(&self) -> R<Value> {
        let mut dirs = vec![];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(p) = exe.parent() {
                dirs.push(p.to_path_buf());
                dirs.push(p.join("_internal"));
            }
        }
        dirs.push(self.data_dir.clone());
        if let Ok(cwd) = std::env::current_dir() {
            dirs.push(cwd);
        }
        for d in &dirs {
            if let Ok(text) = std::fs::read_to_string(d.join(CLIENT_SECRET_FILE)) {
                if let Some(v) = Self::parse_secret(&text) {
                    return Ok(v);
                }
            }
        }
        if let Some(v) = Self::parse_secret(EMBEDDED_CLIENT_SECRET) {
            return Ok(v);
        }
        let picked = rfd::FileDialog::new()
            .set_title("Select your Google OAuth client_secret.json")
            .add_filter("JSON", &["json"])
            .pick_file();
        if let Some(path) = picked {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Some(v) = Self::parse_secret(&text) {
                    let _ = std::fs::write(self.data_dir.join(CLIENT_SECRET_FILE), &text);
                    return Ok(v);
                }
                return Err("That file is not a valid Google OAuth client_secret.json.".into());
            }
        }
        Err("client_secret.json not found. Put it next to ShobdoCalok.exe or in AppData.".into())
    }

    fn access_token(&self) -> R<String> {
        {
            let c = self.creds.lock();
            let Some(c) = c.as_ref() else { return Err("Not signed in".into()) };
            if c.get("expires_at").and_then(|v| v.as_f64()).unwrap_or(0.0) > now_secs() + 60.0 {
                return Ok(c.get("access_token").and_then(|v| v.as_str()).unwrap_or("").to_string());
            }
        }
        self.refresh_token()?;
        let c = self.creds.lock();
        Ok(c.as_ref().and_then(|c| c.get("access_token")).and_then(|v| v.as_str()).unwrap_or("").to_string())
    }

    fn refresh_token(&self) -> R<()> {
        let secrets = self.client_secrets()?;
        let refresh = self
            .creds
            .lock()
            .as_ref()
            .and_then(|c| c.get("refresh_token"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let s = |k: &str| secrets.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let (cid, csec) = (s("client_id"), s("client_secret"));
        let mut resp = agent(30)
            .post(TOKEN_URI)
            .send_form([
                ("client_id", cid.as_str()),
                ("client_secret", csec.as_str()),
                ("refresh_token", refresh.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() != 200 {
            return Err(format!("Token refresh failed: {}", resp.status().as_u16()));
        }
        let data = read_json(&mut resp);
        let mut c = self.creds.lock();
        if let Some(c) = c.as_mut() {
            c["access_token"] = data.get("access_token").cloned().unwrap_or(Value::Null);
            let exp = data.get("expires_in").and_then(|v| v.as_f64()).unwrap_or(3600.0);
            c["expires_at"] = json!(now_secs() + exp - 60.0);
        }
        drop(c);
        self.save_creds();
        Ok(())
    }

    fn exchange_code(&self, code: &str, port: u16) -> R<Value> {
        let secrets = self.client_secrets()?;
        let s = |k: &str| secrets.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let (cid, csec) = (s("client_id"), s("client_secret"));
        let redirect = format!("http://localhost:{port}");
        let mut resp = agent(30)
            .post(TOKEN_URI)
            .send_form([
                ("client_id", cid.as_str()),
                ("client_secret", csec.as_str()),
                ("code", code),
                ("redirect_uri", redirect.as_str()),
                ("grant_type", "authorization_code"),
            ])
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() != 200 {
            let code = resp.status().as_u16();
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            return Err(format!("Auth failed: {code} {body}"));
        }
        let mut data = read_json(&mut resp);
        let exp = data.get("expires_in").and_then(|v| v.as_f64()).unwrap_or(3600.0);
        data["expires_at"] = json!(now_secs() + exp - 60.0);
        Ok(data)
    }

    fn fetch_account(&self, token: &str) -> String {
        let r = agent(15)
            .get(DRIVE_ABOUT_URL)
            .query("fields", "user(emailAddress)")
            .header("Authorization", format!("Bearer {token}"))
            .call();
        if let Ok(mut resp) = r {
            if resp.status().as_u16() == 200 {
                let info = read_json(&mut resp);
                if let Some(e) = info.pointer("/user/emailAddress").and_then(|v| v.as_str()) {
                    return e.to_string();
                }
            }
        }
        String::new()
    }

    // ── Login flow (background thread) ──────────────────────────────────────

    fn start_login(&self) {
        self.set_busy(true);
        self.login_cancelled.store(false, Ordering::SeqCst);
        let res: R<bool> = (|| {
            let secrets = self.client_secrets().map_err(|e| format!("__plain__{e}"))?;
            let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
            let port = listener.local_addr().map_err(|e| e.to_string())?.port();
            listener.set_nonblocking(true).map_err(|e| e.to_string())?;
            let state: String = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(24)
                .map(char::from)
                .collect();
            let cid = secrets.get("client_id").and_then(|v| v.as_str()).unwrap_or("");
            let auth_url = format!(
                "{AUTH_URI}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
                urlencoding::encode(cid),
                urlencoding::encode(&format!("http://localhost:{port}")),
                urlencoding::encode(SCOPE),
                state
            );
            crate::win::open_url(&auth_url);

            let deadline = Instant::now() + Duration::from_secs(180);
            let mut result: Option<Result<String, String>> = None;
            while Instant::now() < deadline && result.is_none() && !self.login_cancelled.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut sock, _)) => {
                        let _ = sock.set_nonblocking(false);
                        let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
                        let mut buf = [0u8; 8192];
                        let n = sock.read(&mut buf).unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]).to_string();
                        let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                        let (p, q) = path.split_once('?').unwrap_or((path.as_str(), ""));
                        if p != "/" {
                            let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                            continue;
                        }
                        let mut code = None;
                        let mut error = None;
                        let mut st = None;
                        for kv in q.split('&') {
                            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                            let v = urlencoding::decode(v).map(|c| c.into_owned()).unwrap_or_default();
                            match k {
                                "code" => code = Some(v),
                                "error" => error = Some(v),
                                "state" => st = Some(v),
                                _ => {}
                            }
                        }
                        result = Some(if let Some(e) = error {
                            Err(e)
                        } else if let Some(c) = code {
                            if st.as_deref().map(|s| s != state).unwrap_or(false) {
                                Err("state_mismatch".into())
                            } else {
                                Ok(c)
                            }
                        } else {
                            Err("no_code".into())
                        });
                        let body = "<html><body style='font-family:sans-serif;text-align:center;padding-top:80px;background:#f3f4f6'><h2>Shobdo Calok</h2><p>Login successful! You can close this tab and return to the app.</p></body></html>";
                        let _ = sock.write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(),
                                body
                            )
                            .as_bytes(),
                        );
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(50)),
                }
            }
            if self.login_cancelled.load(Ordering::SeqCst) {
                self.status("Login cancelled.");
                return Ok(false);
            }
            let code = match result {
                None => return Err("Login timed out. Please try again.".into()),
                Some(Err(e)) => {
                    return Err(if e != "state_mismatch" {
                        "Login failed or was cancelled by Google.".into()
                    } else {
                        "Security check failed. Please try again.".into()
                    })
                }
                Some(Ok(c)) => c,
            };
            let mut creds = self.exchange_code(&code, port)?;
            let token = creds.get("access_token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let account = self.fetch_account(&token);
            if !account.is_empty() {
                creds["account"] = json!(account);
            }
            *self.creds.lock() = Some(creds);
            {
                let mut s = self.state.lock();
                s.account_name = if account.is_empty() { "Google account".into() } else { account };
                s.logged_in = true;
            }
            self.save_creds();
            emit(Event::DriveLoginChanged(true));
            self.sync_backup_status();
            Ok(true)
        })();
        if let Err(e) = res {
            if let Some(plain) = e.strip_prefix("__plain__") {
                self.status(plain);
            } else {
                self.status(&format!("Login error: {e}"));
            }
        }
        self.set_busy(false);
    }

    // ── Drive operations ────────────────────────────────────────────────────

    fn find_by_name(&self, token: &str, name: &str) -> R<Option<Value>> {
        let mut resp = agent(30)
            .get(DRIVE_FILES_URL)
            .query("q", format!("name='{name}' and trashed=false"))
            .query("spaces", "appDataFolder")
            .query("fields", "files(id, name, modifiedTime)")
            .query("pageSize", "1")
            .header("Authorization", format!("Bearer {token}"))
            .call()
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() != 200 {
            return Err(format!("Drive search failed: {}", resp.status().as_u16()));
        }
        let v = read_json(&mut resp);
        Ok(v.get("files").and_then(|f| f.as_array()).and_then(|a| a.first().cloned()))
    }

    fn find_backup_file(&self, token: &str) -> R<Option<Value>> {
        self.find_by_name(token, BACKUP_FILE_NAME)
    }

    fn find_meta_file(&self, token: &str) -> Option<Value> {
        self.find_by_name(token, BACKUP_META_NAME).ok().flatten()
    }

    fn load_meta(&self, token: &str) -> serde_json::Map<String, Value> {
        let Some(info) = self.find_meta_file(token) else { return Default::default() };
        let id = info.get("id").and_then(|v| v.as_str()).unwrap_or("");
        match self.download(token, id) {
            Ok(text) => serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default(),
            Err(_) => Default::default(),
        }
    }

    fn save_meta(&self, token: &str, meta: &serde_json::Map<String, Value>) {
        let content = serde_json::to_string_pretty(meta).unwrap_or_default();
        let id = self.find_meta_file(token).and_then(|f| f.get("id").and_then(|v| v.as_str()).map(String::from));
        let _ = self.upload_named(token, id.as_deref(), &content, BACKUP_META_NAME, "----ShobdoCalokMeta", false);
    }

    fn multipart(name: &str, with_parent: bool, content: &str, prefix: &str) -> (Vec<u8>, String) {
        let mut metadata = json!({ "name": name });
        if with_parent {
            metadata["parents"] = json!(["appDataFolder"]);
        }
        let boundary = format!("{prefix}{}", rand::thread_rng().gen_range(100_000_000..999_999_999));
        let body = format!(
            "--{b}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{m}\r\n--{b}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{c}\r\n--{b}--\r\n",
            b = boundary,
            m = metadata,
            c = content
        );
        (body.into_bytes(), format!("multipart/related; boundary={boundary}"))
    }

    fn upload_named(&self, token: &str, file_id: Option<&str>, content: &str, name: &str, prefix: &str, add_parents: bool) -> R<()> {
        let resp = match file_id {
            Some(fid) => {
                let (body, ct) = Self::multipart(name, false, content, prefix);
                let url = if add_parents {
                    format!("https://www.googleapis.com/upload/drive/v3/files/{fid}?uploadType=multipart&addParents=appDataFolder")
                } else {
                    format!("https://www.googleapis.com/upload/drive/v3/files/{fid}?uploadType=multipart")
                };
                agent(60)
                    .patch(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", ct)
                    .send(&body[..])
            }
            None => {
                let (body, ct) = Self::multipart(name, true, content, prefix);
                agent(60)
                    .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", ct)
                    .send(&body[..])
            }
        };
        let mut resp = resp.map_err(|e| e.to_string())?;
        let code = resp.status().as_u16();
        if code != 200 && code != 201 {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            return Err(format!("Upload failed: {code} {body}"));
        }
        Ok(())
    }

    fn list_query(&self, token: &str, fields: &str, order: bool) -> Vec<Value> {
        let mut req = agent(30)
            .get(DRIVE_FILES_URL)
            .query("q", format!("name contains '{BACKUP_HISTORY_PREFIX}' and trashed=false"))
            .query("spaces", "appDataFolder")
            .query("fields", fields)
            .query("pageSize", "50");
        if order {
            req = req.query("orderBy", "modifiedTime desc");
        }
        match req.header("Authorization", format!("Bearer {token}")).call() {
            Ok(mut r) if r.status().as_u16() == 200 => {
                read_json(&mut r).get("files").and_then(|f| f.as_array()).cloned().unwrap_or_default()
            }
            _ => vec![],
        }
    }

    /// All backup files (main + timestamped history).
    fn list_all_backups(&self, token: &str) -> Vec<Value> {
        let mut history = self.list_query(token, "files(id, name, modifiedTime)", true);
        if let Ok(Some(main)) = self.find_backup_file(token) {
            history.insert(0, main);
        }
        history.retain(|f| f.get("name").and_then(|v| v.as_str()) != Some(BACKUP_META_NAME));
        history
    }

    fn rename_backup(&self, token: &str, file_id: &str) -> String {
        let dt = chrono::Local::now().format("%Y-%m-%d-%H-%M").to_string();
        let mut new_name = format!("{BACKUP_HISTORY_PREFIX}{dt}.json");
        let url = format!("{DRIVE_FILES_URL}/{file_id}");
        let ok = agent(30)
            .patch(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send_json(json!({ "name": new_name }))
            .map(|r| matches!(r.status().as_u16(), 200 | 201))
            .unwrap_or(false);
        if !ok {
            new_name = format!("{BACKUP_HISTORY_PREFIX}{dt}-{}.json", rand::thread_rng().gen_range(100..=999));
            let _ = agent(30)
                .patch(&url)
                .header("Authorization", format!("Bearer {token}"))
                .send_json(json!({ "name": new_name }));
        }
        new_name
    }

    fn delete_file(&self, token: &str, file_id: &str) {
        let _ = agent(30)
            .delete(&format!("{DRIVE_FILES_URL}/{file_id}"))
            .header("Authorization", format!("Bearer {token}"))
            .call();
    }

    fn trim_backup_history(&self, token: &str) {
        let files = self.list_query(token, "files(id, name, modifiedTime)", true);
        for f in files.iter().skip(MAX_BACKUPS) {
            if let Some(id) = f.get("id").and_then(|v| v.as_str()) {
                self.delete_file(token, id);
            }
        }
    }

    fn sync_backup_status(&self) {
        let Ok(token) = self.access_token() else { return };
        match self.find_backup_file(&token) {
            Ok(Some(info)) => {
                let mt = info.get("modifiedTime").and_then(|v| v.as_str()).unwrap_or("");
                let stamp = if mt.is_empty() {
                    "Yes (unknown date)".to_string()
                } else {
                    fmt_iso(mt, "%b %d, %Y %I:%M %p").unwrap_or_else(|| mt.chars().take(10).collect())
                };
                self.set_last_backup(&stamp);
            }
            Ok(None) => self.set_last_backup(""),
            Err(_) => {}
        }
    }

    fn download(&self, token: &str, file_id: &str) -> R<String> {
        let mut resp = agent(30)
            .get(&format!("{DRIVE_FILES_URL}/{file_id}?alt=media"))
            .header("Authorization", format!("Bearer {token}"))
            .call()
            .map_err(|e| e.to_string())?;
        if resp.status().as_u16() != 200 {
            return Err(format!("Download failed: {}", resp.status().as_u16()));
        }
        resp.body_mut().with_config().limit(256 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())
    }

    fn do_backup(&self, name: &str) {
        self.set_busy(true);
        let res: R<()> = (|| {
            if self.creds.lock().is_none() {
                return Err("Not signed in".into());
            }
            let token = self.access_token()?;
            let content = self.store.lock().export_json();
            let mut meta = self.load_meta(&token);
            let file_info = self.find_backup_file(&token)?;
            let fid = file_info.as_ref().and_then(|f| f.get("id")).and_then(|v| v.as_str()).map(String::from);
            let display = if name.is_empty() { local_stamp() } else { name.to_string() };
            if let Some(fid) = &fid {
                let actual = self.rename_backup(&token, fid);
                meta.insert(actual, json!(display));
            }
            self.upload_named(&token, fid.as_deref(), &content, BACKUP_FILE_NAME, "----ShobdoCalok", true)?;
            meta.insert(BACKUP_FILE_NAME.into(), json!(display));
            self.save_meta(&token, &meta);
            let keys: Vec<String> = meta.keys().map(|k| format!("'{k}'")).collect();
            self.status(&format!(
                "Backup '{}' saved (meta keys: [{}])",
                if name.is_empty() { "auto" } else { name },
                keys.join(", ")
            ));
            self.trim_backup_history(&token);
            self.set_last_backup(&local_stamp());
            self.status("Backup uploaded to Google Drive");
            Ok(())
        })();
        if let Err(e) = res {
            self.status(&format!("Backup error: {e}"));
        }
        self.set_busy(false);
    }

    fn do_restore(&self, file_id: Option<String>) {
        self.set_busy(true);
        let res: R<()> = (|| {
            if self.creds.lock().is_none() {
                return Err("Not signed in".into());
            }
            let token = self.access_token()?;
            let info = match file_id {
                Some(id) => Some(json!({ "id": id, "modifiedTime": "" })),
                None => self.find_backup_file(&token)?,
            };
            let Some(info) = info else { return Err("No backup found on Google Drive".into()) };
            let id = info.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let content = self.download(&token, id)?;
            let data: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
            if !self.store.lock().import_value(&data) {
                return Err("Backup file is invalid".into());
            }
            let mt = info.get("modifiedTime").and_then(|v| v.as_str()).unwrap_or("");
            if !mt.is_empty() {
                let stamp = fmt_iso(mt, "%b %d, %Y %I:%M %p").unwrap_or_else(|| mt.chars().take(10).collect());
                self.set_last_backup(&stamp);
            }
            emit(Event::DriveRestoreComplete);
            self.status("Snippets restored from Google Drive");
            Ok(())
        })();
        if let Err(e) = res {
            self.status(&format!("Restore error: {e}"));
        }
        self.set_busy(false);
    }

    fn do_logout(&self) {
        *self.creds.lock() = None;
        {
            let mut s = self.state.lock();
            s.account_name.clear();
            s.logged_in = false;
        }
        let _ = std::fs::remove_file(&self.token_path);
        emit(Event::DriveLoginChanged(false));
        self.status("Signed out");
    }

    // ── Public slots ────────────────────────────────────────────────────────

    pub fn login(self: &Arc<Self>) {
        if self.state.lock().busy {
            // Cancel the previous login attempt, then start a new one
            self.login_cancelled.store(true, Ordering::SeqCst);
            let me = self.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    if !me.state.lock().busy {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if !me.state.lock().busy && !me.login_cancelled.load(Ordering::SeqCst) {
                    me.start_login();
                }
            });
            return;
        }
        self.login_cancelled.store(false, Ordering::SeqCst);
        let me = self.clone();
        std::thread::spawn(move || me.start_login());
    }

    pub fn logout(&self) {
        if self.state.lock().busy {
            return;
        }
        self.do_logout();
    }

    pub fn backup_with_name(self: &Arc<Self>, name: String) {
        let me = self.clone();
        std::thread::spawn(move || me.do_backup(&name));
    }

    pub fn restore(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || me.do_restore(None));
    }

    pub fn restore_from_file(self: &Arc<Self>, file_id: String) {
        let me = self.clone();
        std::thread::spawn(move || me.do_restore(Some(file_id)));
    }

    pub fn list_backups(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            me.set_busy(true);
            let res: R<()> = (|| {
                if me.creds.lock().is_none() {
                    emit(Event::DriveBackupList(vec![]));
                    return Ok(());
                }
                let token = me.access_token()?;
                // File listing and metadata loading in parallel for speed
                let (m1, t1) = (me.clone(), token.clone());
                let files_h = std::thread::spawn(move || m1.list_all_backups(&t1));
                let (m2, t2) = (me.clone(), token.clone());
                let meta_h = std::thread::spawn(move || m2.load_meta(&t2));
                let files = files_h.join().unwrap_or_default();
                let meta = meta_h.join().unwrap_or_default();
                me.status(&format!("Loaded {} backup(s)", files.len()));
                let mut result = vec![];
                for f in &files {
                    let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let mt = f.get("modifiedTime").and_then(|v| v.as_str()).unwrap_or("");
                    let meta_name = meta.get(&name).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let backup_name = if name == BACKUP_FILE_NAME {
                        if meta_name.is_empty() { "Latest Backup".to_string() } else { meta_name }
                    } else if let Some(dt_str) = name.strip_prefix(BACKUP_HISTORY_PREFIX) {
                        if !meta_name.is_empty() {
                            meta_name
                        } else {
                            let dt_str = dt_str.replace(".json", "");
                            let parts: Vec<&str> = dt_str.split('-').collect();
                            let parsed = (parts.len() >= 5)
                                .then(|| {
                                    let n: Vec<u32> = parts[..5].iter().filter_map(|p| p.parse().ok()).collect();
                                    if n.len() == 5 {
                                        chrono::NaiveDate::from_ymd_opt(n[0] as i32, n[1], n[2])
                                            .and_then(|d| d.and_hms_opt(n[3], n[4], 0))
                                    } else {
                                        None
                                    }
                                })
                                .flatten();
                            match parsed {
                                Some(dt) => dt.format("Backup — %d %B %Y, %I:%M %p").to_string(),
                                None => format!("Backup: {dt_str}"),
                            }
                        }
                    } else if meta_name.is_empty() {
                        name.clone()
                    } else {
                        meta_name
                    };
                    let time_label = if mt.is_empty() {
                        String::new()
                    } else {
                        fmt_iso(mt, "%b %d, %Y %I:%M %p").unwrap_or_else(|| mt.chars().take(10).collect())
                    };
                    result.push(BackupItem {
                        id: f.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        name,
                        label: backup_name.clone(),
                        backup_name,
                        time_label,
                    });
                }
                let preview: Vec<String> =
                    result.iter().take(3).map(|r| r.backup_name.chars().take(30).collect()).collect();
                me.status(&format!("Backups: {}", preview.join(", ")));
                emit(Event::DriveBackupList(result));
                Ok(())
            })();
            if let Err(e) = res {
                me.status(&format!("Error listing backups: {e}"));
                emit(Event::DriveBackupList(vec![]));
            }
            me.set_busy(false);
        });
    }

    pub fn delete_backup(self: &Arc<Self>, file_id: String) {
        let me = self.clone();
        std::thread::spawn(move || {
            me.set_busy(true);
            let res: R<()> = (|| {
                if me.creds.lock().is_none() {
                    return Err("Not signed in".into());
                }
                let token = me.access_token()?;
                let mut file_name = String::new();
                if let Ok(mut r) = agent(30)
                    .get(&format!("{DRIVE_FILES_URL}/{file_id}"))
                    .query("fields", "id, name")
                    .header("Authorization", format!("Bearer {token}"))
                    .call()
                {
                    if r.status().as_u16() == 200 {
                        file_name = read_json(&mut r).get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    }
                }
                me.delete_file(&token, &file_id);
                if !file_name.is_empty() && file_name != BACKUP_FILE_NAME {
                    let mut meta = me.load_meta(&token);
                    if meta.remove(&file_name).is_some() {
                        me.save_meta(&token, &meta);
                    }
                }
                me.status("Backup deleted");
                Ok(())
            })();
            if let Err(e) = res {
                me.status(&format!("Delete error: {e}"));
            }
            me.set_busy(false);
        });
    }

    /// Delete all timestamped history backups, keep only the latest.
    pub fn delete_old_backups(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            me.set_busy(true);
            let res: R<()> = (|| {
                if me.creds.lock().is_none() {
                    return Err("Not signed in".into());
                }
                let token = me.access_token()?;
                let mut r = agent(30)
                    .get(DRIVE_FILES_URL)
                    .query("q", format!("name contains '{BACKUP_HISTORY_PREFIX}' and trashed=false"))
                    .query("spaces", "appDataFolder")
                    .query("fields", "files(id, name)")
                    .query("pageSize", "50")
                    .header("Authorization", format!("Bearer {token}"))
                    .call()
                    .map_err(|e| e.to_string())?;
                if r.status().as_u16() != 200 {
                    return Err(format!("Drive search failed: {}", r.status().as_u16()));
                }
                let files = read_json(&mut r).get("files").and_then(|f| f.as_array()).cloned().unwrap_or_default();
                let mut count = 0;
                for f in &files {
                    if let Some(id) = f.get("id").and_then(|v| v.as_str()) {
                        me.delete_file(&token, id);
                        count += 1;
                    }
                }
                if !files.is_empty() {
                    let mut meta = me.load_meta(&token);
                    for f in &files {
                        if let Some(n) = f.get("name").and_then(|v| v.as_str()) {
                            meta.remove(n);
                        }
                    }
                    if !meta.is_empty() {
                        me.save_meta(&token, &meta);
                    }
                }
                me.status(&format!("Deleted {count} old backup(s)"));
                Ok(())
            })();
            if let Err(e) = res {
                me.status(&format!("Delete error: {e}"));
            }
            me.set_busy(false);
        });
    }

    /// Delete the metadata file and rebuild it with default names.
    pub fn clear_metadata(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            me.set_busy(true);
            let res: R<()> = (|| {
                if me.creds.lock().is_none() {
                    return Err("Not signed in".into());
                }
                let token = me.access_token()?;
                if let Some(info) = me.find_meta_file(&token) {
                    if let Some(id) = info.get("id").and_then(|v| v.as_str()) {
                        me.delete_file(&token, id);
                    }
                }
                let files = me.list_all_backups(&token);
                let mut meta = serde_json::Map::new();
                for f in &files {
                    let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    if name.is_empty() || name == BACKUP_META_NAME {
                        continue;
                    }
                    let mt = f.get("modifiedTime").and_then(|v| v.as_str()).unwrap_or("");
                    let fallback = if name == BACKUP_FILE_NAME { "Latest Backup" } else { "Backup" };
                    let label = if mt.is_empty() {
                        fallback.to_string()
                    } else {
                        fmt_iso(mt, "Snippets of %d %B %Y at %I:%M %p").unwrap_or_else(|| fallback.to_string())
                    };
                    meta.insert(name.to_string(), json!(label));
                }
                me.save_meta(&token, &meta);
                me.status(&format!("Names reset. {} backup(s) now have default names.", meta.len()));
                Ok(())
            })();
            if let Err(e) = res {
                me.status(&format!("Clear error: {e}"));
            }
            me.set_busy(false);
        });
    }
}
