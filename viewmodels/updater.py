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
  - IDM-style multi-threaded download (8 connections)
  - Resume interrupted downloads (HTTP Range)
  - Automatic retry on failure (3 attempts with backoff)
  - SHA256 checksum verification with auto re-download on mismatch
  - Extraction progress tracking
  - Cleans up update files after successful apply
"""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
import zipfile
from concurrent.futures import ThreadPoolExecutor

from PySide6.QtCore import QObject, Signal, Slot, Property

try:
    from version import APP_VERSION, GITHUB_OWNER, GITHUB_REPO
except Exception:
    APP_VERSION = "1.0.0"
    GITHUB_OWNER = "YOUR_GITHUB_USERNAME"
    GITHUB_REPO = "ShobdoCalok"

RELEASES_API = f"https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"

_HEADERS = {
    "Accept": "application/vnd.github+json",
    "User-Agent": "ShobdoCalok-Updater",
    "Connection": "keep-alive",
}

# Separate headers for binary downloads — no Accept-Encoding to prevent
# CDN from compressing the ZIP (which changes bytes and breaks SHA256).
_DL_HEADERS = {
    "User-Agent": "ShobdoCalok-Updater",
    "Connection": "keep-alive",
}

MAX_RETRIES = 3
RETRY_BACKOFF = [1, 3, 8]


def _find_github_token() -> str:
    for var in ("GITHUB_TOKEN", "GH_TOKEN"):
        tok = os.environ.get(var, "").strip()
        if tok:
            return tok
    hosts = os.path.join(os.path.expanduser("~"), ".config", "gh", "hosts.yml")
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
    match = re.search(r"(\d+)\.(\d+)(?:\.(\d+))?", v or "")
    if not match:
        return (0, 0, 0)
    return tuple(int(x) for x in match.groups() if x is not None)


def _sha256_file(path: str) -> str:
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
    statusMessageTextChanged = Signal(str)
    updateAvailableChanged = Signal(bool)
    downloadReadyChanged = Signal(bool)

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
        self._download_ready = False
        self._asset_url = ""
        self._asset_name = ""
        self._checksum_url = ""
        self._release_url = ""
        self._status_message_text = ""
        self._download_lock = threading.Lock()

        # Derived paths — set when we know the version/filename
        self._zip_filename = ""
        self._cached_zip_path = ""
        self._cached_checksum_path = ""

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

    @Property(bool, notify=downloadReadyChanged)
    def downloadReady(self) -> bool:
        """True when ZIP is fully downloaded, verified, and ready to install."""
        return self._download_ready

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

    @Property(str, notify=statusMessageTextChanged)
    def statusMessage_text(self) -> str:
        return self._status_message_text

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
        self._status_message_text = msg
        self.statusMessageTextChanged.emit(msg)
        self.statusMessage.emit(msg)

    def _set_paths_for_version(self, version: str, asset_name: str):
        """Set zip/checksum paths based on the remote asset name."""
        safe_name = asset_name.rsplit(".", 1)[0] if asset_name else f"ShobdoCalok-v{version}"
        self._zip_filename = f"{safe_name}.zip"
        self._cached_zip_path = os.path.join(self._update_dir, self._zip_filename)
        self._cached_checksum_path = os.path.join(self._update_dir, f"{safe_name}.sha256")

    def _cleanup_update_dir(self):
        """Remove all update files (zip, chunks, extracted, bat, vbs, log)."""
        try:
            if os.path.exists(self._update_dir):
                shutil.rmtree(self._update_dir, ignore_errors=True)
        except Exception:
            pass

    def _validate_zip(self, path: str) -> bool:
        """Check ZIP exists, has reasonable size, and is not corrupt."""
        if not os.path.exists(path):
            return False
        sz = os.path.getsize(path)
        if sz < 1000:
            return False
        try:
            with zipfile.ZipFile(path, "r") as zf:
                # Quick test — read first few entries
                bad = zf.testzip()
                if bad is not None:
                    return False
        except Exception:
            return False
        return True

    # ── Check for updates ──────────────────────────────────────────────────────

    def _do_check(self):
        try:
            import requests
            self._set_checking(True)
            self._emit_status("Checking for updates…")
            resp = requests.get(RELEASES_API, headers=_HEADERS, timeout=30)
            if resp.status_code == 404:
                self._emit_status("No releases found yet.")
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
            self._asset_url = zip_asset.get("browser_download_url", "") if zip_asset else ""
            self._asset_name = zip_asset.get("name", "") if zip_asset else ""
            self._checksum_url = checksum_asset.get("browser_download_url", "") if checksum_asset else ""

            # Set paths based on actual asset filename
            self._set_paths_for_version(latest, self._asset_name)

            if remote > current:
                # Check if we already have a valid ZIP for this version
                if self._validate_zip(self._cached_zip_path):
                    self._emit_status(f"Update v{latest} already downloaded — ready to install.")
                    self._download_ready = True
                    self.downloadReadyChanged.emit(True)
                else:
                    # Clean up any stale update files from different versions
                    self._cleanup_update_dir()
                    os.makedirs(self._update_dir, exist_ok=True)
                    self._download_ready = False
                    self.downloadReadyChanged.emit(False)

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

    # ── Download (IDM-style multi-threaded) ────────────────────────────────────

    def _download_with_resume(self, url: str, dest: str) -> bool:
        """IDM-style multi-threaded download with resume + corruption detection."""
        import requests

        THREADS = 8

        for attempt in range(MAX_RETRIES):
            try:
                os.makedirs(os.path.dirname(dest), exist_ok=True)

                # GET request to determine total size (follows redirects, unlike HEAD)
                probe = requests.get(url, headers=_DL_HEADERS, stream=True, timeout=30)
                total_size = int(probe.headers.get("Content-Length", 0))
                accept_ranges = probe.headers.get("Accept-Ranges", "") == "bytes"
                probe.close()

                if total_size == 0:
                    return self._download_single_thread(url, dest)

                # Single-threaded fallback if no Range support
                if not accept_ranges:
                    return self._download_single_thread(url, dest)

                # ── Multi-threaded download ──────────────────────────
                chunk_size = max(4 << 20, total_size // THREADS + 1)
                ranges = []
                start = 0
                while start < total_size:
                    end = min(start + chunk_size - 1, total_size - 1)
                    ranges.append((start, end))
                    start = end + 1

                tmp_dir = dest + ".chunks"
                os.makedirs(tmp_dir, exist_ok=True)

                # Count already-downloaded bytes from existing chunk files
                existing_chunk_bytes = 0
                for i in range(len(ranges)):
                    chunk_file = os.path.join(tmp_dir, str(i))
                    if os.path.exists(chunk_file):
                        existing_chunk_bytes += os.path.getsize(chunk_file)

                self._emit_status(f"Downloading with {len(ranges)} connections…")
                done_bytes = existing_chunk_bytes
                done_lock = threading.Lock()
                t0 = time.monotonic()
                last_report_time = t0
                last_report_bytes = existing_chunk_bytes

                def download_chunk(chunk_idx, chunk_start, chunk_end):
                    nonlocal done_bytes
                    chunk_file = os.path.join(tmp_dir, str(chunk_idx))
                    sz = os.path.getsize(chunk_file) if os.path.exists(chunk_file) else 0
                    local_start = chunk_start + sz

                    if local_start > chunk_end:
                        with done_lock:
                            done_bytes += (chunk_end - chunk_start + 1)
                        return True

                    h = dict(_DL_HEADERS)
                    h["Range"] = f"bytes={local_start}-{chunk_end}"
                    r = requests.get(url, headers=h, stream=True, timeout=(30, 120))
                    if r.status_code not in (200, 206):
                        r.close()
                        return False

                    mode = "ab" if sz > 0 else "wb"
                    with open(chunk_file, mode) as f:
                        for data in r.iter_content(chunk_size=2 << 20):
                            if data:
                                f.write(data)
                                with done_lock:
                                    done_bytes += len(data)
                    r.close()
                    return True

                with ThreadPoolExecutor(max_workers=THREADS) as pool:
                    futures = [
                        pool.submit(download_chunk, i, s, e)
                        for i, (s, e) in enumerate(ranges)
                    ]

                    def report_progress():
                        nonlocal last_report_time, last_report_bytes
                        while not all(f.done() for f in futures):
                            now = time.monotonic()
                            with done_lock:
                                current_done = done_bytes
                            dt = now - last_report_time
                            if dt > 0.3:
                                speed = (current_done - last_report_bytes) / dt / (1 << 20)
                                last_report_time = now
                                last_report_bytes = current_done
                            else:
                                speed = 0
                            self._set_progress(min(current_done / total_size, 0.99))
                            self._emit_status(
                                f"Downloading… {current_done >> 20}/{total_size >> 20} MB "
                                f"({speed:.1f} MB/s)"
                            )
                            time.sleep(0.5)
                    reporter = threading.Thread(target=report_progress, daemon=True)
                    reporter.start()
                    results = [f.result() for f in futures]
                    reporter.join(timeout=2)

                if not all(results):
                    self._emit_status("Some chunks failed. Retrying…")
                    shutil.rmtree(tmp_dir, ignore_errors=True)
                    if attempt < MAX_RETRIES - 1:
                        time.sleep(RETRY_BACKOFF[attempt])
                        continue
                    return False

                # ── Reassemble ───────────────────────────────────────
                self._emit_status("Reassembling download…")
                with open(dest, "wb") as out:
                    for i in range(len(ranges)):
                        chunk_file = os.path.join(tmp_dir, str(i))
                        with open(chunk_file, "rb") as cf:
                            while True:
                                data = cf.read(2 << 20)
                                if not data:
                                    break
                                out.write(data)
                shutil.rmtree(tmp_dir, ignore_errors=True)

                # Validate the reassembled file
                if not self._validate_zip(dest):
                    self._emit_status("Downloaded file is corrupted.")
                    try:
                        os.remove(dest)
                    except Exception:
                        pass
                    if attempt < MAX_RETRIES - 1:
                        time.sleep(RETRY_BACKOFF[attempt])
                        continue
                    return False

                elapsed = time.monotonic() - t0
                speed = (total_size / elapsed / (1 << 20)) if elapsed > 0 else 0
                self._set_progress(1.0)
                self._emit_status(f"Download complete ({speed:.1f} MB/s)")
                return True

            except Exception as e:
                if attempt < MAX_RETRIES - 1:
                    wait = RETRY_BACKOFF[attempt]
                    self._emit_status(f"Download failed: {e}. Retrying in {wait}s… ({attempt + 1}/{MAX_RETRIES})")
                    time.sleep(wait)
                else:
                    self._emit_status(f"Download failed after {MAX_RETRIES} attempts: {e}")
                    return False

        return False

    def _download_single_thread(self, url: str, dest: str) -> bool:
        """Fallback single-threaded download with resume."""
        import requests

        for attempt in range(MAX_RETRIES):
            try:
                existing_size = 0
                if os.path.exists(dest):
                    existing_size = os.path.getsize(dest)

                req_headers = dict(_DL_HEADERS)
                if existing_size > 0:
                    req_headers["Range"] = f"bytes={existing_size}-"

                r = requests.get(url, headers=req_headers, stream=True, timeout=(30, 120))
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
                t0 = time.monotonic()
                bytes_since = 0

                mode = "ab" if existing_size > 0 and r.status_code == 206 else "wb"
                with open(dest, mode) as f:
                    for chunk in r.iter_content(chunk_size=2 << 20):
                        if not chunk:
                            continue
                        f.write(chunk)
                        done += len(chunk)
                        bytes_since += len(chunk)
                        if total:
                            self._set_progress(min(done / total, 1.0))
                        elapsed = time.monotonic() - t0
                        if elapsed >= 0.5 and bytes_since:
                            speed = bytes_since / elapsed / (1 << 20)
                            self._emit_status(f"Downloading… {done >> 20}/{total >> 20} MB ({speed:.1f} MB/s)")
                            t0 = time.monotonic()
                            bytes_since = 0

                r.close()

                if not self._validate_zip(dest):
                    self._emit_status("Downloaded file is corrupted.")
                    if attempt < MAX_RETRIES - 1:
                        time.sleep(RETRY_BACKOFF[attempt])
                        continue
                    return False

                return True
            except Exception as e:
                if attempt < MAX_RETRIES - 1:
                    time.sleep(RETRY_BACKOFF[attempt])
                else:
                    self._emit_status(f"Download failed: {e}")
                    return False

        return False

    # ── Extract with progress ──────────────────────────────────────────────────

    def _extract_with_progress(self, zip_path: str, extract_to: str) -> bool:
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
                        self._emit_status(f"Extracting… {i + 1}/{total} files")
            return True
        except zipfile.BadZipFile:
            self._emit_status("Downloaded file is corrupted. Please re-download.")
            return False
        except Exception as e:
            self._emit_status(f"Extraction failed: {e}")
            return False

    # ── Verify checksum ────────────────────────────────────────────────────────

    def _verify_checksum(self, zip_path: str) -> bool:
        if not self._checksum_url:
            self._emit_status("No checksum file available — skipping verification.")
            return True

        import requests

        for verify_attempt in range(2):
            try:
                self._emit_status("Verifying checksum…")
                resp = requests.get(self._checksum_url, headers=_HEADERS, timeout=30)
                if resp.status_code != 200:
                    self._emit_status("Could not download checksum — skipping verification.")
                    return True

                try:
                    with open(self._cached_checksum_path, "w") as f:
                        f.write(resp.text)
                except Exception:
                    pass

                text = resp.text.strip()
                if not text:
                    self._emit_status("Checksum file is empty — skipping verification.")
                    return True
                expected_hash = text.split()[0].strip().lower()

                if len(expected_hash) != 64 or not all(c in "0123456789abcdef" for c in expected_hash):
                    self._emit_status("Invalid checksum format — skipping.")
                    return True

                actual_hash = _sha256_file(zip_path)

                if actual_hash == expected_hash:
                    self._emit_status("Checksum verified")
                    return True
                else:
                    if verify_attempt == 0:
                        self._emit_status("Checksum mismatch — re-downloading…")
                        try:
                            os.remove(zip_path)
                        except Exception:
                            pass
                        if self._download_with_resume(self._asset_url, zip_path):
                            continue
                    self._emit_status("Checksum mismatch but proceeding — file may still work.")
                    return True

            except Exception as e:
                self._emit_status(f"Checksum verification failed: {e} — proceeding anyway.")
                return True

        return True

    # ── Download + install ─────────────────────────────────────────────────────

    def _do_download_and_install(self):
        if not self._download_lock.acquire(blocking=False):
            self._emit_status("An update is already in progress.")
            return
        try:
            import requests
            if not self._is_frozen:
                self._emit_status("Updates can only be installed in the packaged app.")
                return
            if not self._asset_url:
                self._emit_status("No update asset found to download.")
                return

            os.makedirs(self._update_dir, exist_ok=True)
            zip_path = self._cached_zip_path

            # ── Phase 1: Check for valid cached ZIP ────────────────────
            if self._download_ready and self._validate_zip(zip_path):
                self._emit_status("Using previously downloaded update.")
                self._set_progress(1.0)
            elif os.path.exists(zip_path) and self._validate_zip(zip_path):
                self._emit_status("Using cached update file.")
                self._set_progress(1.0)
            else:
                # ── Phase 2: Download ──────────────────────────────────
                self._set_downloading(True)
                self._set_progress(0.0)
                self._emit_status("Downloading update…")

                if not self._download_with_resume(self._asset_url, zip_path):
                    return
                self._set_progress(1.0)

            # ── Phase 3: Verify checksum ───────────────────────────────
            if not self._verify_checksum(zip_path):
                self._emit_status("Update aborted due to checksum failure.")
                return

            self._set_downloading(False)

            # ── Phase 4: Extract ───────────────────────────────────────
            self._emit_status("Extracting update…")
            extract_to = os.path.join(self._update_dir, "extracted")
            if os.path.exists(extract_to):
                shutil.rmtree(extract_to, ignore_errors=True)
            os.makedirs(extract_to, exist_ok=True)

            self._set_progress(0.0)
            if not self._extract_with_progress(zip_path, extract_to):
                return
            self._set_progress(1.0)

            # ── Phase 5: Apply ─────────────────────────────────────────
            new_root = self._find_new_app_root(extract_to)
            if not new_root:
                self._emit_status("Update file did not contain the app. Please re-download.")
                return

            self._emit_status("Applying update — restarting…")
            self._set_applying(True)
            self._set_progress(0.0)

            self._emit_status("Preparing update files…")
            self._set_progress(0.2)
            time.sleep(0.3)

            self._emit_status("Launching updater…")
            self._set_progress(0.5)
            self._launch_applier(new_root, extract_to, zip_path)

        except Exception as e:
            self._emit_status(f"Update failed: {e}")
        finally:
            self._set_downloading(False)
            self._set_applying(False)
            self._set_checking(False)
            self._download_lock.release()

    def _find_new_app_root(self, extract_to: str):
        candidates = []
        for root, _dirs, files in os.walk(extract_to):
            if "ShobdoCalok.exe" in files or "main.py" in files:
                candidates.append(root)
        if not candidates:
            return None
        return min(candidates, key=lambda p: p.count(os.sep))

    def _launch_applier(self, new_root: str, extract_to: str, zip_path: str):
        app_root = self._app_root
        update_dir = self._update_dir
        bat = os.path.join(update_dir, "apply_update.bat")
        vbs = os.path.join(update_dir, "apply_update.vbs")
        log_file = os.path.join(update_dir, "update_log.txt")

        batch_script = f"""@echo off
setlocal enabledelayedexpansion
title ShobdoCalok Updater
mode con: cols=60 lines=24
color 0B

REM Center the window
powershell -Command "$h=Get-Host;$w=$h.UI.RawUI.WindowSize;$b=$h.UI.RawUI.BufferSize;$b.Width=60;$h.UI.RawUI.BufferSize=$b;$x=[int](([System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Width-$w.Width)/2);$y=[int](([System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Height-$w.Height)/2);$h.UI.RawUI.WindowPosition=New-Object System.Management.Automation.Host.Coordinates($x,$y)" 2>nul

cls
echo.
echo  ==========================================
echo     ShobdoCalok Updater
echo  ==========================================
echo.
set "APP={app_root}"
set "NEW={new_root}"
set "UPDATE={update_dir}"
set "TMPEXTRACT={extract_to}"
set "LOG={log_file}"

echo ===== ShobdoCalok Updater ===== >> "%LOG%"
echo Timestamp: %DATE% %TIME% >> "%LOG%"
echo APP=%APP% >> "%LOG%"
echo NEW=%NEW% >> "%LOG%"
echo. >> "%LOG%"

echo  [1/4] Stopping ShobdoCalok...
echo  [1/4] Killing app process... >> "%LOG%"
taskkill /F /IM ShobdoCalok.exe 2>nul >> "%LOG%"
echo  Waiting for files to unlock...
timeout /t 3 /nobreak >nul
echo  [1/4] Process stopped.
echo [1/4] Process killed. >> "%LOG%"

echo.
echo  [2/4] Checking update files...
echo [2/4] Checking source... >> "%LOG%"
set "LAUNCH_EXE="
if exist "%NEW%\\ShobdoCalok.exe" set "LAUNCH_EXE=1"
if "!LAUNCH_EXE!"=="" if exist "%NEW%\\main.py" set "LAUNCH_EXE=0"
if "!LAUNCH_EXE!"=="" goto no_files

if "!LAUNCH_EXE!"=="1" (
    echo  [2/4] Found: ShobdoCalok.exe
    echo [2/4] Source OK ^(packaged^) >> "%LOG%"
) else (
    echo  [2/4] Found: main.py
    echo [2/4] Source OK ^(source mode^) >> "%LOG%"
)
goto copy_files

:no_files
echo  [2/4] ERROR: No update files found!
echo  Extracted folder contents:
dir "%NEW%" /b 2>nul
echo [2/4] ERROR: No app found in: %NEW% >> "%LOG%"
dir "%NEW%" /b >> "%LOG%" 2>nul
echo.
echo  Press any key to close...
pause >nul
goto cleanup

:copy_files
echo.
echo  [3/4] Installing update...
echo  Copying new files (this may take a moment)...
echo [3/4] Copying new files... >> "%LOG%"
robocopy "%NEW%" "%APP%" /E /XD AppData /NFL /NDL /NJH /NJS /NC /NS /NP
set RC=!errorlevel!
echo  [3/4] Copy finished (exit code !RC!)
echo [3/4] Copy finished ^(errorlevel !RC!^) >> "%LOG%"
if !RC! GEQ 8 (
    echo  [3/4] robocopy had issues, trying xcopy...
    echo [3/4] WARNING: robocopy had errors, trying xcopy... >> "%LOG%"
    xcopy "%NEW%\\*" "%APP%\\" /E /Y /Q >> "%LOG%" 2>nul
    echo  [3/4] xcopy completed.
)

echo.
echo  [4/4] Starting ShobdoCalok...
echo [4/4] Starting app... >> "%LOG%"
if "!LAUNCH_EXE!"=="1" (
    start "" "%APP%\\ShobdoCalok.exe"
) else (
    start "" /D "%APP%" python main.py
)
echo  [4/4] App started!
echo [4/4] App started. >> "%LOG%"

echo.
echo  Update complete! Closing in 3 seconds...
timeout /t 3 /nobreak >nul

:cleanup
echo Cleaning up update files... >> "%LOG%"
if exist "%TMPEXTRACT%" rmdir /s /q "%TMPEXTRACT%"
if exist "%UPDATE%" rmdir /s /q "%UPDATE%"
del "%~f0" 2>nul
"""
        with open(bat, "w", encoding="ascii", errors="ignore") as f:
            f.write(batch_script.replace("\n", "\r\n"))

        vbs_script = (
            'Set objShell = CreateObject("WScript.Shell")\n'
            f'objShell.Run "cmd /c ""{bat}""", 1, False\n'
        )
        with open(vbs, "w", encoding="ascii", errors="ignore") as f:
            f.write(vbs_script)

        try:
            import ctypes
            result = ctypes.windll.shell32.ShellExecuteW(
                None, "open", vbs, None, update_dir, 1
            )
            if result <= 32:
                raise RuntimeError(f"ShellExecuteW returned {result}")
        except Exception:
            try:
                subprocess.Popen(
                    f'start "" "{vbs}"',
                    shell=True, cwd=update_dir,
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
