"""
viewmodels/updater.py — Check for updates on GitHub and install them.

Works with the portable (onedir) build produced by GitHub Actions:
  ShobdoCalok.exe
  _internal/          <- program files (replaced on update)
  AppData/            <- user data  (NEVER touched: snippets, settings, tokens)

The updater:
  1. Queries the GitHub Releases API for the newest release.
  2. Compares its version against the running APP_VERSION.
  3. Downloads the portable ZIP asset into AppData/update/ (resumes if interrupted).
  4. Verifies SHA256 checksum against .sha256 file in the release.
  5. Extracts with progress tracking, then hands off to a detached batch script
     that waits for the app to exit, swaps the program files, and relaunches.

Features:
  - Resume interrupted downloads (HTTP Range)
  - Automatic retry on failure (3 attempts with backoff)
  - SHA256 checksum verification
  - Extraction progress tracking
"""

import hashlib
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

_HEADERS = {
    "Accept": "application/vnd.github+json",
    "User-Agent": "ShobdoCalok-Updater",
    "Accept-Encoding": "gzip, deflate",
    "Connection": "keep-alive",
}

MAX_RETRIES = 3
RETRY_BACKOFF = [1, 3, 8]  # seconds between retries


def _find_github_token() -> str:
    """Try to find a GitHub token for authenticated API requests."""
    for var in ("GITHUB_TOKEN", "GH_TOKEN"):
        tok = os.environ.get(var, "").strip()
        if tok:
            return tok

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


def _sha256_file(path: str) -> str:
    """Compute SHA256 hex digest of a file."""
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


class UpdaterViewModel(QObject):
    """Exposes update checking / downloading / installing to QML."""

    checkingChanged     = Signal(bool)
    downloadingChanged  = Signal(bool)
    applyingChanged     = Signal(bool)
    progressChanged     = Signal(float)
    statusMessage       = Signal(str)
    updateAvailableChanged = Signal(bool)

    def __init__(self, data_dir: str, parent=None):
        super().__init__(parent)
        self._data_dir = data_dir
        self._update_dir = os.path.join(data_dir, "update")

        self._checking = False
        self._downloading = False
        self._applying = False
        self._progress = 0.0
        self._latest_version = ""
        self._update_available = False
        self._asset_url = ""
        self._checksum_url = ""
        self._release_url = ""
        self._download_lock = threading.Lock()
        self._cached_zip_path = os.path.join(self._update_dir, "ShobdoCalok-latest.zip")
        self._cached_checksum_path = os.path.join(self._update_dir, "ShobdoCalok-latest.sha256")

        # If the cached ZIP matches current version, update was applied — clear stale state
        self._clear_applied_state()

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

    @Property(bool, notify=applyingChanged)
    def applying(self) -> bool:
        return self._applying

    @Property(float, notify=progressChanged)
    def progress(self) -> float:
        return self._progress

    # ── Helpers ────────────────────────────────────────────────────────────────

    def _clear_applied_state(self):
        """If cached ZIP's version matches current, the update was applied.
        Remove the ZIP so the next check starts fresh."""
        try:
            cached = os.path.join(self._update_dir, "ShobdoCalok-latest.zip")
            if not os.path.exists(cached):
                return
            # Quick heuristic: if the cached ZIP is from the current version,
            # delete it to avoid 'update available' false positives
            # We check by looking at the ZIP's internal version.py
            import zipfile
            with zipfile.ZipFile(cached, "r") as zf:
                for name in zf.namelist():
                    if name.endswith("version.py") or name.endswith("version.pyc"):
                        data = zf.read(name).decode("utf-8", errors="ignore")
                        match = re.search(r'APP_VERSION\s*=\s*["\']([^"\']+)', data)
                        if match:
                            zip_ver = match.group(1)
                            if _version_tuple(zip_ver) == _version_tuple(APP_VERSION):
                                # Same version — update was applied, remove cache
                                os.remove(cached)
                                checksum = os.path.join(self._update_dir, "ShobdoCalok-latest.sha256")
                                if os.path.exists(checksum):
                                    os.remove(checksum)
                        break
        except Exception:
            pass

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
            checksum_asset = next(
                (a for a in assets if a.get("name", "").lower().endswith(".sha256")),
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

            if checksum_asset:
                self._checksum_url = checksum_asset.get("browser_download_url", "")
            else:
                self._checksum_url = ""

            if remote > current:
                # Double-check: if we already have this ZIP cached, the user
                # may have already applied it manually or via a previous run
                if os.path.exists(self._cached_zip_path):
                    # Check if cached ZIP matches the remote version
                    try:
                        import zipfile as _zf
                        with _zf.ZipFile(self._cached_zip_path, "r") as zf:
                            for name in zf.namelist():
                                if name.endswith("version.py") or name.endswith("version.pyc"):
                                    data = zf.read(name).decode("utf-8", errors="ignore")
                                    match = re.search(r'APP_VERSION\s*=\s*["\']([^"\']+)', data)
                                    if match and _version_tuple(match.group(1)) == current:
                                        # Cached ZIP has same version as running — already applied
                                        os.remove(self._cached_zip_path)
                                        if os.path.exists(self._cached_checksum_path):
                                            os.remove(self._cached_checksum_path)
                                        self._emit_status(f"You're up to date (version {APP_VERSION}).")
                                        return
                                    break
                    except Exception:
                        pass

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

    # ── Download (with resume + retry) ─────────────────────────────────────────

    def _download_with_resume(self, url: str, dest: str) -> bool:
        """Download a file with resume support and retry logic.

        Returns True on success, False on failure after all retries.
        """
        import requests

        for attempt in range(MAX_RETRIES):
            try:
                existing_size = 0
                if os.path.exists(dest):
                    existing_size = os.path.getsize(dest)

                headers = dict(_HEADERS)
                if existing_size > 0:
                    headers["Range"] = f"bytes={existing_size}-"
                    self._emit_status(
                        f"Resuming download from {existing_size / (1<<20):.1f} MB…"
                    )

                with requests.get(url, headers=headers, stream=True, timeout=(30, 120)) as r:
                    if r.status_code == 416:
                        self._set_progress(1.0)
                        return True

                    if r.status_code == 200:
                        existing_size = 0
                    elif r.status_code == 206:
                        pass
                    else:
                        r.raise_for_status()

                    total = int(r.headers.get("Content-Length", 0)) + existing_size
                    done = existing_size

                    mode = "ab" if existing_size > 0 and r.status_code == 206 else "wb"
                    with open(dest, mode) as f:
                        for chunk in r.iter_content(chunk_size=16 << 20):  # 16MB chunks
                            if not chunk:
                                continue
                            f.write(chunk)
                            done += len(chunk)
                            if total:
                                self._set_progress(min(done / total, 1.0))

                return True

            except Exception as e:
                if attempt < MAX_RETRIES - 1:
                    wait = RETRY_BACKOFF[attempt]
                    self._emit_status(
                        f"Download failed: {e}. Retrying in {wait}s…"
                        f" ({attempt + 1}/{MAX_RETRIES})"
                    )
                    time.sleep(wait)
                else:
                    self._emit_status(f"Download failed after {MAX_RETRIES} attempts: {e}")
                    return False

        return False

    # ── Extract with progress ──────────────────────────────────────────────────

    def _extract_with_progress(self, zip_path: str, extract_to: str) -> bool:
        """Extract ZIP file with progress tracking. Returns True on success."""
        try:
            with zipfile.ZipFile(zip_path, "r") as zf:
                members = zf.namelist()
                total = len(members)
                if total == 0:
                    self._emit_status("Update archive is empty.")
                    return False

                for i, member in enumerate(members):
                    zf.extract(member, extract_to)
                    self._set_progress((i + 1) / total)
                    if (i + 1) % 50 == 0 or i + 1 == total:
                        self._emit_status(
                            f"Extracting… {i + 1}/{total} files"
                        )

            return True
        except zipfile.BadZipFile:
            self._emit_status("Downloaded file is corrupted. Please re-download.")
            return False
        except Exception as e:
            self._emit_status(f"Extraction failed: {e}")
            return False

    # ── Verify checksum ────────────────────────────────────────────────────────

    def _verify_checksum(self, zip_path: str) -> bool:
        """Download .sha256 file and verify the ZIP checksum. Returns True if valid."""
        if not self._checksum_url:
            self._emit_status("No checksum file available — skipping verification.")
            return True

        import requests

        try:
            self._emit_status("Verifying checksum…")
            resp = requests.get(self._checksum_url, headers=_HEADERS, timeout=30)
            if resp.status_code != 200:
                self._emit_status("Could not download checksum — skipping verification.")
                return True

            # Save the checksum file locally for future caching
            try:
                with open(self._cached_checksum_path, "w") as f:
                    f.write(resp.text)
            except Exception:
                pass

            # Parse the .sha256 file
            # Format: "<hash>" or "<hash>  <filename>" — take first token only
            text = resp.text.strip()
            if not text:
                self._emit_status("Checksum file is empty — skipping verification.")
                return True
            expected_hash = text.split()[0].strip().lower()

            # Validate hash format (should be 64 hex chars)
            if len(expected_hash) != 64 or not all(c in "0123456789abcdef" for c in expected_hash):
                self._emit_status(f"Invalid checksum format: {expected_hash[:16]}… — skipping.")
                return True

            actual_hash = _sha256_file(zip_path)

            if actual_hash == expected_hash:
                self._emit_status("Checksum verified")
                return True
            else:
                self._emit_status(
                    f"Checksum mismatch! Expected {expected_hash[:16]}…, "
                    f"got {actual_hash[:16]}…. Download may be corrupted."
                )
                return False

        except Exception as e:
            self._emit_status(f"Checksum verification failed: {e} — continuing anyway.")
            return True  # Don't block update on checksum failure

    # ── Download + install ─────────────────────────────────────────────────────

    def _do_download_and_install(self):
        if not self._download_lock.acquire(blocking=False):
            self._emit_status("An update is already in progress.")
            return
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
            zip_path = self._cached_zip_path

            # ── Phase 1: Check for cached ZIP ────────────────────────────
            cached_valid = False
            if os.path.exists(zip_path) and os.path.getsize(zip_path) > 1000:
                cached_checksum = self._cached_checksum_path
                if os.path.exists(cached_checksum):
                    try:
                        with open(cached_checksum, "r") as f:
                            expected = f.read().strip().split()[0].lower()
                        actual = _sha256_file(zip_path)
                        if actual == expected:
                            cached_valid = True
                            self._emit_status("Using cached update (checksum OK).")
                    except Exception:
                        pass

            # ── Phase 2: Download with resume + retry ────────────────────
            if not cached_valid:
                self._set_downloading(True)
                self._set_progress(0.0)
                self._emit_status("Downloading update…")

                if not self._download_with_resume(self._asset_url, zip_path):
                    return

                self._set_progress(1.0)
            else:
                self._set_progress(1.0)

            # ── Phase 3: Verify checksum ─────────────────────────────────
            if not self._verify_checksum(zip_path):
                self._emit_status("Update aborted due to checksum failure.")
                return

            self._set_downloading(False)

            # ── Phase 4: Extract with progress ───────────────────────────
            self._emit_status("Extracting update…")
            extract_to = os.path.join(self._update_dir, "extracted")
            if os.path.exists(extract_to):
                import shutil
                shutil.rmtree(extract_to, ignore_errors=True)
            os.makedirs(extract_to, exist_ok=True)

            self._set_progress(0.0)
            if not self._extract_with_progress(zip_path, extract_to):
                return

            self._set_progress(1.0)

            # ── Phase 5: Locate new app and apply ────────────────────────
            new_root = self._find_new_app_root(extract_to)
            if not new_root:
                self._emit_status("Update file did not contain the app. Please re-download.")
                return

            self._emit_status("Applying update — restarting…")
            self._set_applying(True)
            self._launch_applier(new_root, extract_to, zip_path)

        except Exception as e:
            self._emit_status(f"Update failed: {e}")
        finally:
            self._set_downloading(False)
            self._set_applying(False)
            self._set_checking(False)
            self._download_lock.release()

    def _find_new_app_root(self, extract_to: str):
        """Find the folder that holds the new ShobdoCalok.exe inside the zip."""
        candidates = []
        for root, _dirs, files in os.walk(extract_to):
            if "ShobdoCalok.exe" in files:
                candidates.append(root)
        if not candidates:
            return None
        return min(candidates, key=lambda p: p.count(os.sep))

    def _launch_applier(self, new_root: str, extract_to: str, zip_path: str):
        """Write a batch script + VBScript wrapper that swaps files silently after exit."""
        app_root = self._app_root
        bat = os.path.join(self._update_dir, "apply_update.bat")
        vbs = os.path.join(self._update_dir, "apply_update.vbs")
        log_file = os.path.join(self._update_dir, "update_log.txt")

        batch_script = f"""@echo off
setlocal enabledelayedexpansion
set "APP={app_root}"
set "NEW={new_root}"
set "TMPEXTRACT={extract_to}"
set "LOG={log_file}"

echo ===== ShobdoCalok Updater ===== >> "%LOG%"
echo Timestamp: %DATE% %TIME% >> "%LOG%"
echo APP=%APP% >> "%LOG%"
echo NEW=%NEW% >> "%LOG%"
echo. >> "%LOG%"

echo [1/4] Killing app process... >> "%LOG%"
taskkill /F /IM ShobdoCalok.exe 2>nul >> "%LOG%"
timeout /t 2 /nobreak >nul
echo [1/4] Process killed. >> "%LOG%"

echo [2/4] Checking source... >> "%LOG%"
if exist "%NEW%\\ShobdoCalok.exe" (
    echo [2/4] Source OK ^(packaged^) >> "%LOG%"
    set "LAUNCH_EXE=1"
) else if exist "%NEW%\\main.py" (
    echo [2/4] Source OK ^(source mode^) >> "%LOG%"
    set "LAUNCH_EXE=0"
) else (
    echo [2/4] ERROR: No app found in: %NEW% >> "%LOG%"
    goto cleanup
)

echo [3/4] Copying new files... >> "%LOG%"
robocopy "%NEW%" "%APP%" /E /XD AppData /NFL /NDL /NJH /NJS /NC /NS /NP >> "%LOG%"
set RC=!errorlevel!
echo [3/4] Copy finished ^(errorlevel !RC!^) >> "%LOG%"

echo [4/4] Starting app... >> "%LOG%"
if "!LAUNCH_EXE!"=="1" (
    start "" "%APP%\\ShobdoCalok.exe"
) else (
    start "" /D "%APP%" python main.py
)
echo [4/4] App started. >> "%LOG%"

:cleanup
echo Cleaning up... >> "%LOG%"
if exist "%TMPEXTRACT%" rmdir /s /q "%TMPEXTRACT%"
del "%~f0" 2>nul
del "{vbs}" 2>nul
"""
        with open(bat, "w", encoding="ascii", errors="ignore") as f:
            f.write(batch_script.replace("\n", "\r\n"))

        # VBScript wrapper — runs the batch silently (window style 0 = hidden)
        vbs_script = (
            'Set objShell = CreateObject("WScript.Shell")\n'
            f'objShell.Run "cmd /c ""{bat}""", 0, False\n'
        )
        with open(vbs, "w", encoding="ascii", errors="ignore") as f:
            f.write(vbs_script)

        # Launch VBScript (hidden, detached)
        try:
            import ctypes
            SW_SHOWNORMAL = 1
            result = ctypes.windll.shell32.ShellExecuteW(
                None, "open", vbs, None, self._update_dir, SW_SHOWNORMAL
            )
            if result <= 32:
                raise RuntimeError(f"ShellExecuteW returned {result}")
        except Exception:
            try:
                subprocess.Popen(
                    f'start "" "{vbs}"',
                    shell=True,
                    cwd=self._update_dir,
                    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
                )
            except Exception as e2:
                raise RuntimeError(f"Could not start the updater: {e2}")

        time.sleep(1)
        os._exit(0)

    # ── Internal state helpers ─────────────────────────────────────────────────

    def _set_checking(self, value: bool):
        self._checking = value
        self.checkingChanged.emit(value)

    def _set_downloading(self, value: bool):
        self._downloading = value
        self.downloadingChanged.emit(value)

    def _set_applying(self, value: bool):
        self._applying = value
        self.applyingChanged.emit(value)

    def _set_progress(self, value: float):
        self._progress = value
        self.progressChanged.emit(value)

    # ── QML slots ──────────────────────────────────────────────────────────────

    @Slot()
    def checkForUpdate(self):
        if self._checking or self._downloading or self._applying:
            return
        threading.Thread(target=self._do_check, daemon=True).start()

    @Slot()
    def downloadAndInstall(self):
        if self._checking or self._downloading or self._applying:
            return
        threading.Thread(target=self._do_download_and_install, daemon=True).start()

    @Slot(result=str)
    def releasePageUrl(self) -> str:
        return self._release_url or RELEASES_API
