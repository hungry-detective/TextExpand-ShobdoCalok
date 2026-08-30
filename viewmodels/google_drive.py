"""
google_drive.py — Google login + snippet backup/restore via the Drive API.

Uses OAuth 2.0 (installed-app flow with a loopback redirect) to obtain
credentials once, then refreshes the access token automatically. The user's
snippets are stored as a single JSON file named "ShobdoCalok.snippets.json"
in an app-managed folder in Google Drive, so they never clutter the user's
normal Drive file list.
"""

import json
import os
import random
import socket
import string
import sys
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse, parse_qs

from PySide6.QtCore import QObject, Signal, Slot, Property

# ── Google constants ───────────────────────────────────────────────────────────
CLIENT_SECRET_FILE = "client_secret.json"
TOKEN_URI = "https://oauth2.googleapis.com/token"
AUTH_URI = "https://accounts.google.com/o/oauth2/v2/auth"
SCOPE = "https://www.googleapis.com/auth/drive.appdata"
DRIVE_FILES_URL = "https://www.googleapis.com/drive/v3/files"
DRIVE_ABOUT_URL = "https://www.googleapis.com/drive/v3/about"
BACKUP_FILE_NAME = "ShobdoCalok.snippets.json"
BACKUP_HISTORY_PREFIX = "ShobdoCalok-backup-"
MAX_BACKUPS = 10  # Keep up to 10 timestamped backups


class _RedirectHandler(BaseHTTPRequestHandler):
    """Captures the /?code=... redirect from the Google consent page."""

    result = None
    state = None

    def do_GET(self):  # noqa: N802 (std http.server API)
        parsed = urlparse(self.path)
        if parsed.path != "/":
            self.send_response(404)
            self.end_headers()
            return
        qs = parse_qs(parsed.query)
        code = qs.get("code", [None])[0]
        error = qs.get("error", [None])[0]
        state = qs.get("state", [None])[0]

        if error:
            _RedirectHandler.result = {"error": error}
        elif code:
            if state is not None and state != _RedirectHandler.state:
                _RedirectHandler.result = {"error": "state_mismatch"}
            else:
                _RedirectHandler.result = {"code": code}
        else:
            _RedirectHandler.result = {"error": "no_code"}

        body = (
            "<html><body style='font-family:sans-serif;text-align:center;"
            "padding-top:80px;background:#f3f4f6'><h2>Shobdo Calok</h2>"
            "<p>Login successful! You can close this tab and return to the app.</p>"
            "</body></html>"
        ).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


class GoogleDriveViewModel(QObject):
    """Exposes Google login + Drive backup/restore to QML."""

    loginStateChanged = Signal(bool)      # signed in / out
    accountNameChanged = Signal(str)      # user email
    lastBackupChanged  = Signal(str)      # human-readable timestamp or ""
    busyChanged        = Signal(bool)
    statusMessage      = Signal(str)      # transient toast message
    restoreComplete    = Signal()         # emitted after restore to refresh UI
    backupListReady    = Signal(list)     # list of {id, name, modifiedTime, label}

    def __init__(self, data_dir: str, parent=None):
        super().__init__(parent)
        self._data_dir = data_dir
        self._token_path = os.path.join(data_dir, "google_token.json")
        self._creds = None                # {"access_token", "refresh_token", ...}
        self._account_name = ""
        self._last_backup = ""
        self._busy = False
        self._login_cancelled = False
        self._snippet_store = None        # set later from main.py

        self._load_creds()
        self._load_last_backup()

    # ── Store wiring ───────────────────────────────────────────────────────────

    def set_snippet_store(self, store):
        self._snippet_store = store

    # ── Properties ─────────────────────────────────────────────────────────────

    @Property(bool, notify=loginStateChanged)
    def loggedIn(self) -> bool:
        return self._creds is not None

    @Property(str, notify=accountNameChanged)
    def accountName(self) -> str:
        return self._account_name

    @Property(str, notify=lastBackupChanged)
    def lastBackup(self) -> str:
        return self._last_backup

    @Property(bool, notify=busyChanged)
    def busy(self) -> bool:
        return self._busy

    # ── Token persistence ──────────────────────────────────────────────────────

    def _load_creds(self):
        try:
            with open(self._token_path, "r", encoding="utf-8") as f:
                data = json.load(f)
            if data.get("refresh_token"):
                self._creds = data
                self._account_name = data.get("account", "")
                if self._creds.get("access_token") and self._creds.get("expires_at", 0) > time.time():
                    pass  # still valid
                else:
                    self._refresh_token()
        except Exception:
            self._creds = None

    def _save_creds(self):
        try:
            with open(self._token_path, "w", encoding="utf-8") as f:
                json.dump(self._creds, f, indent=2)
        except Exception:
            pass

    def _load_last_backup(self):
        try:
            with open(os.path.join(self._data_dir, "last_backup.txt"), "r", encoding="utf-8") as f:
                self._last_backup = f.read().strip()
        except Exception:
            self._last_backup = ""

    def _set_last_backup(self, text: str):
        self._last_backup = text
        try:
            with open(os.path.join(self._data_dir, "last_backup.txt"), "w", encoding="utf-8") as f:
                f.write(text)
        except Exception:
            pass
        self.lastBackupChanged.emit(text)

    # ── Auth primitives ────────────────────────────────────────────────────────

    def _client_secrets(self) -> dict:
        """Locate client_secret.json next to the bundle / project root."""
        search_dirs = []
        if getattr(sys, "frozen", False):
            search_dirs.append(getattr(sys, "_MEIPASS", ""))
        search_dirs.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
        search_dirs.append(os.getcwd())
        for d in search_dirs:
            path = os.path.join(d, CLIENT_SECRET_FILE)
            if os.path.exists(path):
                try:
                    with open(path, "r", encoding="utf-8") as f:
                        data = json.load(f)
                    inst = data.get("installed") or data.get("web")
                    if inst:
                        return inst
                except Exception:
                    continue
        raise FileNotFoundError(
            "client_secret.json not found. Ask the developer to include the "
            "Google OAuth credentials with the app."
        )

    def _access_token(self) -> str:
        """Return a valid access token, refreshing if necessary."""
        if not self._creds:
            raise RuntimeError("Not signed in")
        if self._creds.get("expires_at", 0) > time.time() + 60:
            return self._creds["access_token"]
        self._refresh_token()
        return self._creds["access_token"]

    def _refresh_token(self):
        import requests
        secrets = self._client_secrets()
        resp = requests.post(
            TOKEN_URI,
            data={
                "client_id": secrets["client_id"],
                "client_secret": secrets["client_secret"],
                "refresh_token": self._creds["refresh_token"],
                "grant_type": "refresh_token",
            },
            timeout=30,
        )
        if resp.status_code != 200:
            raise RuntimeError(f"Token refresh failed: {resp.status_code}")
        data = resp.json()
        self._creds["access_token"] = data["access_token"]
        self._creds["expires_at"] = time.time() + data.get("expires_in", 3600) - 60
        self._save_creds()

    def _exchange_code(self, code: str) -> dict:
        import requests
        secrets = self._client_secrets()
        resp = requests.post(
            TOKEN_URI,
            data={
                "client_id": secrets["client_id"],
                "client_secret": secrets["client_secret"],
                "code": code,
                "redirect_uri": f"http://localhost:{self._port}",
                "grant_type": "authorization_code",
            },
            timeout=30,
        )
        if resp.status_code != 200:
            raise RuntimeError(f"Auth failed: {resp.status_code} {resp.text}")
        data = resp.json()
        data["expires_at"] = time.time() + data.get("expires_in", 3600) - 60
        return data

    def _fetch_account(self, token: str) -> str:
        """Return the signed-in user's email via the Drive 'about' endpoint."""
        import requests
        try:
            resp = requests.get(
                DRIVE_ABOUT_URL,
                params={"fields": "user(emailAddress)"},
                headers={"Authorization": f"Bearer {token}"},
                timeout=15,
            )
            if resp.status_code == 200:
                info = resp.json()
                email = info.get("user", {}).get("emailAddress", "")
                if email:
                    return email
        except Exception:
            pass
        return ""

    # ── Login flow (runs on a background thread) ───────────────────────────────

    def _start_login(self):
        try:
            self._set_busy(True)
            self._login_cancelled = False
            secrets = self._client_secrets()

            # Pick a free loopback port
            s = socket.socket()
            s.bind(("127.0.0.1", 0))
            self._port = s.getsockname()[1]
            s.close()

            state = "".join(random.choices(string.ascii_letters + string.digits, k=24))
            _RedirectHandler.state = state
            _RedirectHandler.result = None

            auth_params = (
                f"client_id={secrets['client_id']}"
                f"&redirect_uri=http://localhost:{self._port}"
                f"&response_type=code"
                f"&scope={SCOPE}"
                f"&access_type=offline"
                f"&prompt=consent"
                f"&state={state}"
            )
            auth_url = f"{AUTH_URI}?{auth_params}"

            server = HTTPServer(("127.0.0.1", self._port), _RedirectHandler)
            server.timeout = 0.2
            try:
                webbrowser.open(auth_url)
                deadline = time.time() + 180
                while (time.time() < deadline
                       and _RedirectHandler.result is None
                       and not self._login_cancelled):
                    server.handle_request()
                    time.sleep(0.05)
            finally:
                server.server_close()

            if self._login_cancelled:
                self.statusMessage.emit("Login cancelled.")
                return

            result = _RedirectHandler.result
            if not result:
                raise RuntimeError("Login timed out. Please try again.")
            if result.get("error"):
                raise RuntimeError(
                    "Login failed or was cancelled by Google."
                    if result["error"] != "state_mismatch"
                    else "Security check failed. Please try again."
                )

            creds = self._exchange_code(result["code"])
            self._creds = creds
            account = self._fetch_account(creds["access_token"])
            if account:
                self._creds["account"] = account
            self._account_name = account or "Google account"
            self._save_creds()

            self.accountNameChanged.emit(self._account_name)
            self.loginStateChanged.emit(True)
            self._sync_backup_status()
        except FileNotFoundError as e:
            self.statusMessage.emit(str(e))
        except Exception as e:
            self.statusMessage.emit(f"Login error: {e}")
        finally:
            self._set_busy(False)

    # ── Drive operations ───────────────────────────────────────────────────────

    def _find_backup_file(self, token: str):
        import requests
        url = DRIVE_FILES_URL
        params = {
            "q": f"name='{BACKUP_FILE_NAME}' and trashed=false",
            "spaces": "appDataFolder",
            "fields": "files(id, modifiedTime)",
            "pageSize": "1",
        }
        resp = requests.get(
            url,
            params=params,
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )
        if resp.status_code != 200:
            raise RuntimeError(f"Drive search failed: {resp.status_code}")
        files = resp.json().get("files", [])
        return files[0] if files else None

    def _list_all_backups(self, token: str) -> list:
        """List all backup files (main + timestamped history)."""
        import requests
        url = DRIVE_FILES_URL
        params = {
            "q": f"name contains '{BACKUP_HISTORY_PREFIX}' and trashed=false",
            "spaces": "appDataFolder",
            "fields": "files(id, name, modifiedTime)",
            "pageSize": "50",
            "orderBy": "modifiedTime desc",
        }
        resp = requests.get(
            url, params=params,
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )
        if resp.status_code != 200:
            return []
        history = resp.json().get("files", [])
        # Also include the main backup file
        main = self._find_backup_file(token)
        if main:
            history.insert(0, main)
        return history

    def _rename_backup(self, token: str, file_id: str, old_name: str):
        """Rename a backup file to add a timestamp suffix."""
        import requests
        from datetime import datetime
        dt = datetime.now().strftime("%Y-%m-%d-%H-%M")
        new_name = f"{BACKUP_HISTORY_PREFIX}{dt}.json"
        url = f"{DRIVE_FILES_URL}/{file_id}"
        resp = requests.patch(
            url,
            json={"name": new_name},
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )
        # If rename fails (e.g., name collision), try with seconds suffix
        if resp.status_code not in (200, 201):
            new_name = f"{BACKUP_HISTORY_PREFIX}{dt}-{random.randint(100,999)}.json"
            requests.patch(
                url, json={"name": new_name},
                headers={"Authorization": f"Bearer {token}"},
                timeout=30,
            )

    def _delete_file(self, token: str, file_id: str):
        """Delete a file from Drive."""
        import requests
        requests.delete(
            f"{DRIVE_FILES_URL}/{file_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )

    def _trim_backup_history(self, token: str):
        """Keep only MAX_BACKUPS timestamped backups, delete oldest."""
        import requests
        params = {
            "q": f"name contains '{BACKUP_HISTORY_PREFIX}' and trashed=false",
            "spaces": "appDataFolder",
            "fields": "files(id, name, modifiedTime)",
            "pageSize": "50",
            "orderBy": "modifiedTime desc",
        }
        resp = requests.get(
            url=DRIVE_FILES_URL, params=params,
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )
        if resp.status_code != 200:
            return
        files = resp.json().get("files", [])
        # Delete oldest if over limit
        for f in files[MAX_BACKUPS:]:
            self._delete_file(token, f["id"])

    def _sync_backup_status(self):
        """Query Drive for backup existence and update UI. Called after login."""
        try:
            if not self._creds:
                return
            token = self._access_token()
            file_info = self._find_backup_file(token)
            if file_info:
                mt = file_info.get("modifiedTime", "")
                if mt:
                    from datetime import datetime
                    try:
                        dt = datetime.fromisoformat(mt.replace("Z", "+00:00"))
                        stamp = dt.strftime("%b %d, %Y %I:%M %p")
                    except Exception:
                        stamp = mt[:10]
                else:
                    stamp = "Yes (unknown date)"
                self._set_last_backup(stamp)
            else:
                self._set_last_backup("")
        except Exception:
            pass

    def _upload(self, token: str, file_id: str, content: str):
        import requests

        def _multipart(fid: str):
            metadata = {"name": BACKUP_FILE_NAME}
            if not fid:
                metadata["parents"] = ["appDataFolder"]
            boundary = "----ShobdoCalok" + str(random.randint(10**8, 10**9 - 1))
            body = (
                f"--{boundary}\r\n"
                "Content-Type: application/json; charset=UTF-8\r\n\r\n"
                + json.dumps(metadata)
                + "\r\n"
                f"--{boundary}\r\n"
                "Content-Type: application/json; charset=UTF-8\r\n\r\n"
                + content
                + "\r\n"
                f"--{boundary}--\r\n"
            ).encode("utf-8")
            headers = {
                "Authorization": f"Bearer {token}",
                "Content-Type": f"multipart/related; boundary={boundary}",
            }
            return body, headers

        if file_id:
            url = (
                f"https://www.googleapis.com/upload/drive/v3/files/{file_id}"
                f"?uploadType=multipart&addParents=appDataFolder"
            )
            body, headers = _multipart(file_id)
            resp = requests.patch(url, headers=headers, data=body, timeout=60)
        else:
            url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart"
            body, headers = _multipart(None)
            resp = requests.post(url, headers=headers, data=body, timeout=60)

        if resp.status_code not in (200, 201):
            raise RuntimeError(f"Upload failed: {resp.status_code} {resp.text}")

    def _download(self, token: str, file_id: str) -> str:
        import requests
        resp = requests.get(
            f"{DRIVE_FILES_URL}/{file_id}?alt=media",
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )
        if resp.status_code != 200:
            raise RuntimeError(f"Download failed: {resp.status_code}")
        return resp.text

    def _do_backup(self):
        try:
            self._set_busy(True)
            if not self._creds:
                raise RuntimeError("Not signed in")
            if self._snippet_store is None:
                raise RuntimeError("Snippet store not wired")
            token = self._access_token()
            content = json.dumps(
                self._snippet_store.export_data(),
                indent=2, ensure_ascii=False,
            )
            file_info = self._find_backup_file(token)
            if file_info:
                # Rename old backup with timestamp before overwriting
                self._rename_backup(token, file_info["id"], BACKUP_FILE_NAME)
            self._upload(token, file_info["id"] if file_info else None, content)
            # Trim history to keep only MAX_BACKUPS
            self._trim_backup_history(token)
            stamp = time.strftime("%b %d, %Y %I:%M %p")
            self._set_last_backup(stamp)
            self.statusMessage.emit("Backup uploaded to Google Drive")
        except Exception as e:
            self.statusMessage.emit(f"Backup error: {e}")
        finally:
            self._set_busy(False)

    def _do_restore(self, file_id: str = None):
        try:
            self._set_busy(True)
            if not self._creds:
                raise RuntimeError("Not signed in")
            if self._snippet_store is None:
                raise RuntimeError("Snippet store not wired")
            token = self._access_token()
            if file_id:
                # Restore from specific backup
                file_info = {"id": file_id, "modifiedTime": ""}
            else:
                file_info = self._find_backup_file(token)
            if not file_info:
                raise RuntimeError("No backup found on Google Drive")
            content = self._download(token, file_info["id"])
            data = json.loads(content)
            if not self._snippet_store.import_data(data):
                raise RuntimeError("Backup file is invalid")
            # Update lastBackup from Drive's modifiedTime
            mt = file_info.get("modifiedTime", "")
            if mt:
                from datetime import datetime
                try:
                    dt = datetime.fromisoformat(mt.replace("Z", "+00:00"))
                    stamp = dt.strftime("%b %d, %Y %I:%M %p")
                except Exception:
                    stamp = mt[:10]
                self._set_last_backup(stamp)
            self.restoreComplete.emit()
            self.statusMessage.emit("Snippets restored from Google Drive")
        except Exception as e:
            self.statusMessage.emit(f"Restore error: {e}")
        finally:
            self._set_busy(False)

    def _do_logout(self):
        try:
            self._creds = None
            self._account_name = ""
            if os.path.exists(self._token_path):
                try:
                    os.remove(self._token_path)
                except Exception:
                    pass
            self.accountNameChanged.emit("")
            self.loginStateChanged.emit(False)
            self.statusMessage.emit("Signed out")
        except Exception as e:
            self.statusMessage.emit(f"Logout error: {e}")

    # ── Helpers ────────────────────────────────────────────────────────────────

    def _set_busy(self, busy: bool):
        self._busy = busy
        self.busyChanged.emit(busy)

    # ── QML slots ──────────────────────────────────────────────────────────────

    @Slot()
    def login(self):
        if self._busy:
            # Cancel the previous login attempt, then start a new one
            self._login_cancelled = True
            # Wait briefly for old thread to finish, then start new one
            threading.Thread(target=self._retry_login_after_cancel, daemon=True).start()
            return
        self._login_cancelled = False
        threading.Thread(target=self._start_login, daemon=True).start()

    def _retry_login_after_cancel(self):
        """Wait for the old login to finish, then start a new one."""
        for _ in range(100):
            if not self._busy:
                break
            time.sleep(0.05)
        if not self._busy and not self._login_cancelled:
            self._login_cancelled = False
            threading.Thread(target=self._start_login, daemon=True).start()

    @Slot()
    def logout(self):
        if self._busy:
            return
        self._do_logout()

    @Slot()
    def backup(self):
        threading.Thread(target=self._do_backup, daemon=True).start()

    @Slot()
    def restore(self):
        threading.Thread(target=self._do_restore, daemon=True).start()

    @Slot(str)
    def restoreFromFile(self, file_id: str):
        threading.Thread(target=self._do_restore, args=(file_id,), daemon=True).start()

    @Slot()
    def listBackups(self):
        """List all available backups and emit backupListReady."""
        def _worker():
            try:
                self._set_busy(True)
                if not self._creds:
                    self.backupListReady.emit([])
                    return
                token = self._access_token()
                files = self._list_all_backups(token)
                result = []
                for f in files:
                    name = f.get("name", "")
                    mt = f.get("modifiedTime", "")
                    # Create human-readable label
                    if name == BACKUP_FILE_NAME:
                        label = "Latest backup"
                    elif name.startswith(BACKUP_HISTORY_PREFIX):
                        # Extract date from filename
                        dt_str = name.replace(BACKUP_HISTORY_PREFIX, "").replace(".json", "")
                        label = f"Backup: {dt_str}"
                    else:
                        label = name
                    # Format modifiedTime
                    if mt:
                        from datetime import datetime
                        try:
                            dt = datetime.fromisoformat(mt.replace("Z", "+00:00"))
                            label += f" ({dt.strftime('%b %d, %Y %I:%M %p')})"
                        except Exception:
                            label += f" ({mt[:10]})"
                    result.append({
                        "id": f.get("id", ""),
                        "name": name,
                        "label": label,
                    })
                self.backupListReady.emit(result)
            except Exception as e:
                self.statusMessage.emit(f"Error listing backups: {e}")
                self.backupListReady.emit([])
            finally:
                self._set_busy(False)
        threading.Thread(target=_worker, daemon=True).start()

    @Slot()
    def deleteOldBackups(self):
        """Delete all timestamped history backups, keep only the latest."""
        def _worker():
            try:
                self._set_busy(True)
                if not self._creds:
                    raise RuntimeError("Not signed in")
                token = self._access_token()
                # Find all history backups
                import requests
                params = {
                    "q": f"name contains '{BACKUP_HISTORY_PREFIX}' and trashed=false",
                    "spaces": "appDataFolder",
                    "fields": "files(id, name)",
                    "pageSize": "50",
                }
                resp = requests.get(
                    url=DRIVE_FILES_URL, params=params,
                    headers={"Authorization": f"Bearer {token}"},
                    timeout=30,
                )
                if resp.status_code != 200:
                    raise RuntimeError(f"Drive search failed: {resp.status_code}")
                files = resp.json().get("files", [])
                count = 0
                for f in files:
                    self._delete_file(token, f["id"])
                    count += 1
                self.statusMessage.emit(f"Deleted {count} old backup(s)")
            except Exception as e:
                self.statusMessage.emit(f"Delete error: {e}")
            finally:
                self._set_busy(False)
        threading.Thread(target=_worker, daemon=True).start()
