"""
viewmodels/updater.py — Check for updates on GitHub and install them.

Works with the portable (onedir) build produced by GitHub Actions:
  ShobdoCalok.exe
  _internal/          <- program files (replaced on update)
  AppData/            <- user data  (NEVER touched: snippets, settings, tokens)

The updater:
  1. Queries the GitHub Releases API for the newest release.
  2. Compares its version against the running APP_VERSION.
  3. Downloads the portable ZIP asset into AppData/update/.
  4. Extracts it, then hands off to a detached batch script that waits for
     the app to exit, swaps the program files, and relaunches the app —
     preserving AppData/ at all times.
"""

import json
import os
import re
import subprocess
import sys
import threading
import time
import zipfile

from PySide6.QtCore import QObject, Signal, Slot, Property

try:
    from version import APP_VERSION, GITHUB_OWNER, GITHUB_REPO
except Exception:  # pragma: no cover - fallback for odd import setups
    APP_VERSION = "1.0.0"
    GITHUB_OWNER = "YOUR_GITHUB_USERNAME"
    GITHUB_REPO = "ShobdoCalok"

RELEASES_API = f"https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"

_HEADERS = {"Accept": "application/vnd.github+json", "User-Agent": "ShobdoCalok-Updater"}


def _find_github_token() -> str:
    """Try to find a GitHub token for authenticated API requests.

    Checks (in order):
      1. GITHUB_TOKEN / GH_TOKEN environment variables
      2. gh CLI config file  (~/.config/gh/hosts.yml)  — Linux / macOS
      3. gh auth token command  — works when token is in OS keyring (Windows)
    """
    for var in ("GITHUB_TOKEN", "GH_TOKEN"):
        tok = os.environ.get(var, "").strip()
        if tok:
            return tok

    # gh CLI stores its token in a YAML file on Linux/macOS; parse without PyYAML.
    hosts = os.path.join(
        os.path.expanduser("~"), ".config", "gh", "hosts.yml"
    )
    try:
        with open(hosts, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line.startswith("oauth_token:"):
                    return line.split(":", 1)[1].strip().strip("'\"")
    except Exception:
        pass

    # On Windows the token lives in the OS keyring; ask gh directly.
    try:
        result = subprocess.run(
            ["gh", "auth", "token"],
            capture_output=True, text=True, timeout=5,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        tok = result.stdout.strip()
        if result.returncode == 0 and tok:
            return tok
    except Exception:
        pass

    return ""


_token = _find_github_token()
if _token:
    _HEADERS["Authorization"] = f"Bearer {_token}"


def _version_tuple(v: str):
    """Turn 'v1.2.3' or '1.2.3' into a comparable tuple of ints."""
    match = re.search(r"(\d+)\.(\d+)(?:\.(\d+))?", v or "")
    if not match:
        return (0, 0, 0)
    return tuple(int(x) for x in match.groups() if x is not None)


class UpdaterViewModel(QObject):
    """Exposes update checking / downloading / installing to QML."""

    checkingChanged     = Signal(bool)
    downloadingChanged  = Signal(bool)
    progressChanged     = Signal(float)
    statusMessage       = Signal(str)
    updateAvailableChanged = Signal(bool)

    def __init__(self, data_dir: str, parent=None):
        super().__init__(parent)
        self._data_dir = data_dir
        self._update_dir = os.path.join(data_dir, "update")

        self._checking = False
        self._downloading = False
        self._progress = 0.0
        self._latest_version = ""
        self._update_available = False
        self._asset_url = ""
        self._release_url = ""

    # ── Properties ─────────────────────────────────────────────────────────────

    @Property(str, constant=True)
    def currentVersion(self) -> str:
        return APP_VERSION

    @Property(str, notify=updateAvailableChanged)
    def latestVersion(self) -> str:
        return self._latest_version

    @Property(bool, notify=updateAvailableChanged)
    def updateAvailable(self) -> bool:
        return self._update_available

    @Property(bool, notify=checkingChanged)
    def checking(self) -> bool:
        return self._checking

    @Property(bool, notify=downloadingChanged)
    def downloading(self) -> bool:
        return self._downloading

    @Property(float, notify=progressChanged)
    def progress(self) -> float:
        return self._progress

    # ── Helpers ────────────────────────────────────────────────────────────────

    @property
    def _is_frozen(self) -> bool:
        return bool(getattr(sys, "frozen", False))

    @property
    def _app_root(self) -> str:
        if self._is_frozen:
            return os.path.dirname(sys.executable)
        return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    def _emit_status(self, msg: str):
        self.statusMessage.emit(msg)

    # ── Check for updates ──────────────────────────────────────────────────────

    def _do_check(self):
        try:
            import requests
            self._set_checking(True)
            self._emit_status("Checking for updates…")
            resp = requests.get(RELEASES_API, headers=_HEADERS, timeout=30)
            if resp.status_code == 404:
                self._emit_status(
                    "No releases found yet. Updates are published as GitHub Releases."
                )
                return
            if resp.status_code != 200:
                self._emit_status(f"Update check failed (HTTP {resp.status_code}).")
                return

            data = resp.json()
            tag = data.get("tag_name", "")
            latest = tag.lstrip("v")
            assets = data.get("assets", [])
            zip_asset = next(
                (a for a in assets if a.get("name", "").lower().endswith(".zip")),
                None,
            )

            current = _version_tuple(APP_VERSION)
            remote = _version_tuple(latest)
            self._latest_version = latest
            self._release_url = data.get("html_url", "")

            if zip_asset:
                self._asset_url = zip_asset.get("browser_download_url", "")
            else:
                self._asset_url = ""

            if remote > current:
                self._update_available = True
                self.updateAvailableChanged.emit(True)
                self._emit_status(
                    f"Update available: version {latest} (you have {APP_VERSION})."
                )
            else:
                self._update_available = False
                self.updateAvailableChanged.emit(False)
                self._emit_status(f"You're up to date (version {APP_VERSION}).")
        except Exception as e:
            self._emit_status(f"Update check failed: {e}")
        finally:
            self._set_checking(False)

    # ── Download + install ─────────────────────────────────────────────────────

    def _do_download_and_install(self):
        try:
            import requests
            if not self._is_frozen:
                self._emit_status(
                    "Updates can only be installed in the packaged app. "
                    "Run from the built EXE to update."
                )
                return
            if not self._asset_url:
                self._emit_status("No update asset found to download.")
                return

            os.makedirs(self._update_dir, exist_ok=True)
            zip_path = os.path.join(self._update_dir, "ShobdoCalok-latest.zip")

            self._set_downloading(True)
            self._set_progress(0.0)
            self._emit_status("Downloading update…")

            with requests.get(self._asset_url, stream=True, timeout=120) as r:
                r.raise_for_status()
                total = int(r.headers.get("Content-Length", 0))
                done = 0
                with open(zip_path, "wb") as f:
                    for chunk in r.iter_content(chunk_size=1 << 20):
                        if not chunk:
                            continue
                        f.write(chunk)
                        done += len(chunk)
                        if total:
                            self._set_progress(min(done / total, 1.0))
            self._set_progress(1.0)

            self._emit_status("Extracting update…")
            extract_to = os.path.join(self._update_dir, "extracted")
            if os.path.exists(extract_to):
                import shutil
                shutil.rmtree(extract_to, ignore_errors=True)
            os.makedirs(extract_to, exist_ok=True)
            with zipfile.ZipFile(zip_path, "r") as zf:
                zf.extractall(extract_to)

            # Locate the packaged app folder inside the zip (ShobdoCalok/…)
            new_root = self._find_new_app_root(extract_to)
            if not new_root:
                self._emit_status("Update file did not contain the app. Please re-download.")
                return

            self._emit_status("Applying update…")
            self._launch_applier(new_root, extract_to, zip_path)
            self._emit_status("Update applied. Restarting…")
        except Exception as e:
            self._emit_status(f"Update failed: {e}")
            self._set_downloading(False)

    def _find_new_app_root(self, extract_to: str):
        """Find the folder that holds the new ShobdoCalok.exe inside the zip."""
        candidates = []
        for root, _dirs, files in os.walk(extract_to):
            if "ShobdoCalok.exe" in files:
                candidates.append(root)
        if not candidates:
            return None
        # Prefer the shallowest path
        return min(candidates, key=lambda p: p.count(os.sep))

    def _launch_applier(self, new_root: str, extract_to: str, zip_path: str):
        """Write a batch script that swaps program files after this app exits."""
        app_root = self._app_root
        bat = os.path.join(self._update_dir, "apply_update.bat")

        # The batch file lives in <app_root>/AppData/update/, so the app root is two levels up.
        script = f"""@echo off
setlocal enabledelayedexpansion
set "APP=%~dp0..\\.."
set "NEW={new_root}"
set "TMPEXTRACT={extract_to}"
set "ZIPFILE={zip_path}"
set "OLDINTERNAL=%APP%\\_internal"

rem Wait until the running app has fully exited (max 30 seconds)
set /a COUNT=0
:waitloop
tasklist /FI "IMAGENAME eq ShobdoCalok.exe" 2>nul | find /I "ShobdoCalok.exe" >nul
if not errorlevel 1 (
    set /a COUNT+=1
    if !COUNT! GEQ 30 (
        echo Timed out waiting for app to exit
        goto cleanup
    )
    timeout /t 1 /nobreak >nul
    goto waitloop
)

rem Small delay to ensure file handles are released
timeout /t 2 /nobreak >nul

rem Replace program files (never touch AppData)
if exist "%OLDINTERNAL%" rmdir /s /q "%OLDINTERNAL%"
xcopy "%NEW%\\*" "%APP%\\" /e /i /y /h /q

rem Relaunch the updated app
start "" "%APP%\\ShobdoCalok.exe"

:cleanup
rem Clean up the download + extract leftovers
if exist "%TMPEXTRACT%" rmdir /s /q "%TMPEXTRACT%"
if exist "%ZIPFILE%" del /q "%ZIPFILE%"
del "%~f0"
"""
        with open(bat, "w", encoding="ascii", errors="ignore") as f:
            f.write(script.replace("\n", "\r\n"))

        # Launch the batch script detached from this process, then quit the app.
        try:
            creationflags = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0) \
                | getattr(subprocess, "DETACHED_PROCESS", 0) \
                | getattr(subprocess, "CREATE_NO_WINDOW", 0)
            subprocess.Popen(
                ["cmd.exe", "/c", bat],
                cwd=self._update_dir,
                creationflags=creationflags,
                close_fds=True,
            )
        except Exception as e:
            raise RuntimeError(f"Could not start the updater: {e}")

        # Give the batch script a moment to take over, then exit.
        time.sleep(0.5)
        from PySide6.QtCore import QCoreApplication
        QCoreApplication.quit()

    # ── Internal state helpers ─────────────────────────────────────────────────

    def _set_checking(self, value: bool):
        self._checking = value
        self.checkingChanged.emit(value)

    def _set_downloading(self, value: bool):
        self._downloading = value
        self.downloadingChanged.emit(value)

    def _set_progress(self, value: float):
        self._progress = value
        self.progressChanged.emit(value)

    # ── QML slots ──────────────────────────────────────────────────────────────

    @Slot()
    def checkForUpdate(self):
        threading.Thread(target=self._do_check, daemon=True).start()

    @Slot()
    def downloadAndInstall(self):
        threading.Thread(target=self._do_download_and_install, daemon=True).start()

    @Slot(result=str)
    def releasePageUrl(self) -> str:
        return self._release_url or RELEASES_API
