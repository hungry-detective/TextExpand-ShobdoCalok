# -*- mode: python ; coding: utf-8 -*-
"""
ShobdoCalok Updater — standalone GUI updater.
Built as a separate executable so it has its own PySide6 runtime.
"""

import os
import shutil

# Use separate workpath to avoid conflicts with main app build
WORKPATH = 'build_updater'
DISTPATH = 'dist'

# Clean previous build
if os.path.exists(WORKPATH):
    shutil.rmtree(WORKPATH, ignore_errors=True)

# Ensure dist/ShobdoCalok/ exists so the updater is in the right place
os.makedirs(os.path.join(DISTPATH, 'ShobdoCalok'), exist_ok=True)

a = Analysis(
    ['updater_gui.py'],
    pathex=[WORKPATH],
    binaries=[],
    datas=[],
    hiddenimports=[
        'PySide6.QtWidgets',
        'PySide6.QtCore',
        'PySide6.QtGui',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=['PyQt5', 'PyQt6', 'PyQt5-sip', 'PyQt6-sip', 'brotli', 'brotlicffi', 'zstandard', 'backports.zstd', 'pynput'],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='Updater',
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

# Move the single-file exe into dist/ShobdoCalok/ so the main build can find it
_src = os.path.join(DISTPATH, 'Updater.exe')
_dst = os.path.join(DISTPATH, 'ShobdoCalok', 'Updater.exe')
if os.path.exists(_src) and not os.path.exists(_dst):
    shutil.copy2(_src, _dst)
