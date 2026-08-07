# Shobdo Calok

**A fast, keyboard-friendly text expander for Windows.** Define short abbreviations that automatically expand into full phrases, code blocks, or dynamic templates.

Shobdo Calok runs silently in your system tray and supports date/time tokens, clipboard insertion, fill-in fields, folder-based snippet organisation, Google Drive backup, and automatic updates from GitHub Releases.

![Platform](https://img.shields.io/badge/platform-Windows-blue)
![Python](https://img.shields.io/badge/python-3.12+-informational)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

---

## Features

- ⚡ Real-time abbreviation expansion while you type
- 📁 Folder-based snippet organisation
- 🧩 Dynamic templates: `{date}`, `{time}`, `{field:Label}`, `{cursor}`
- 📋 Clipboard + direct-insert modes
- 🌗 Light / dark / system theme
- ☁️ Google Drive backup & restore
- 🚀 Automatic "check for updates" via GitHub Releases
- 🔄 Fully portable (no installer — your data lives in `AppData\`)

## How updates work

The app checks the [Releases](https://github.com/hungry-detective/TextExpand-ShobdoCalok/releases) page for the newest version. When a newer release is found you can download and install it from **Settings → Check for Updates**. The updater:

1. Downloads the portable build (ZIP) for the latest release
2. Replaces the program files (`ShobdoCalok.exe`, `_internal\`)
3. **Never touches your data** in `AppData\` (snippets, settings, tokens)

## Running from source

```bash
# 1. Install Python 3.12+ and create a virtual environment
python -m venv .venv
.venv\Scripts\activate

# 2. Install dependencies
pip install -r requirements.txt

# 3. Launch
python main.py
```

Or simply double-click `Run_ShobdoCalok.bat`.

### Google Drive backup

Google Drive backup needs your own OAuth credentials. Copy `client_secret.example.json` to `client_secret.json` and fill in your **client_id** and **client_secret** from the [Google Cloud Console](https://console.cloud.google.com/apis/credentials). This file is git-ignored and never published.

## Building a portable EXE

```bash
pip install pyinstaller
pyinstaller --noconfirm --clean ShobdoCalok.spec
# Output: dist\ShobdoCalok\
```

## Releasing a new version

1. Bump `APP_VERSION` in `version.py`
2. Tag & push:

```bash
git tag v1.1.0
git push origin v1.1.0
```

The GitHub Actions workflow automatically builds the portable ZIP and publishes it as a release. Users then get the update through **Settings → Check for Updates**.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) first.

## License

This project is licensed under the [GNU General Public License v3](LICENSE).
