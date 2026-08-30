# -*- mode: python ; coding: utf-8 -*-

import os

_datas = [
    ('Components', 'Components'),
    ('viewmodels', 'viewmodels'),
    ('framelesswindow', 'framelesswindow'),
    ('icons', 'icons'),
    ('main.qml', '.'),
    ('version.py', '.'),
    ('MaterialSymbolsOutlined.ttf', '.'),
    ('app-icon.svg', '.'),
    ('app-icon.ico', '.'),
]

# Bundle Google OAuth credentials ONLY if present locally (never committed).
# Skip silently otherwise so CI/public builds still succeed.
if os.path.exists('client_secret.json'):
    _datas.append(('client_secret.json', '.'))

# Bundle the standalone updater exe (must be built first via updater.spec)
updater_paths = [
    os.path.join('dist', 'ShobdoCalok', 'ShobdoCalok_updater.exe'),
    os.path.join('dist', 'ShobdoCalok_updater.exe'),
]
for _p in updater_paths:
    if os.path.exists(_p):
        _datas.append((_p, '.'))
        break

a = Analysis(
    ['main.py'],
    pathex=[],
    binaries=[],
    datas=_datas,
    hiddenimports=[
        'pynput.keyboard._win32', 
        'pynput.mouse._win32',
        'pyperclip',
        'requests'
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=['PyQt5', 'PyQt6', 'PyQt5-sip', 'PyQt6-sip', 'brotli', 'brotlicffi', 'zstandard', 'backports.zstd'],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='ShobdoCalok',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=['app-icon.ico'],
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name='ShobdoCalok',
)

# Post-build: ensure the standalone updater is bundled next to the main exe
import shutil as _shutil
_updater_src = os.path.join('dist', 'ShobdoCalok_updater.exe')
_updater_dst = os.path.join('dist', 'ShobdoCalok', 'ShobdoCalok_updater.exe')
if os.path.exists(_updater_src) and not os.path.exists(_updater_dst):
    _shutil.copy2(_updater_src, _updater_dst)
