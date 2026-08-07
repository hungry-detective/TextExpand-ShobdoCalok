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

    def __init__(self, data_dir: str, parent=None):
        super().__init__(parent)
        self._data_dir = data_dir
        self._token_path = os.path.join(data_dir, "google_token.json")
        self._creds = None                # {"access_token", "refresh_token", ...}
        self._account_name = ""
        self._last_backup = ""
        self._busy = False
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
                while time.time() < deadline and _RedirectHandler.result is None:
                    server.handle_request()
                    time.sleep(0.05)
            finally:
                server.server_close()

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
            self.statusMessage.emit(f"Signed in as {self._account_name}")
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
        return files[0]["id"] if files else None

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
            file_id = self._find_backup_file(token)
            self._upload(token, file_id, content)
            stamp = time.strftime("%b %d, %Y %I:%M %p")
            self._set_last_backup(stamp)
            self.statusMessage.emit("Backup uploaded to Google Drive")
        except Exception as e:
            self.statusMessage.emit(f"Backup error: {e}")
        finally:
            self._set_busy(False)

    def _do_restore(self):
        try:
            self._set_busy(True)
            if not self._creds:
                raise RuntimeError("Not signed in")
            if self._snippet_store is None:
                raise RuntimeError("Snippet store not wired")
            token = self._access_token()
            file_id = self._find_backup_file(token)
            if not file_id:
                raise RuntimeError("No backup found on Google Drive")
            content = self._download(token, file_id)
            data = json.loads(content)
            if not self._snippet_store.import_data(data):
                raise RuntimeError("Backup file is invalid")
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
        threading.Thread(target=self._start_login, daemon=True).start()

    @Slot()
    def logout(self):
        self._do_logout()

    @Slot()
    def backup(self):
        threading.Thread(target=self._do_backup, daemon=True).start()

    @Slot()
    def restore(self):
        threading.Thread(target=self._do_restore, daemon=True).start()
