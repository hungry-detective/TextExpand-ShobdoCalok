# -*- mode: python ; coding: utf-8 -*-
"""
ShobdoCalok Updater — standalone GUI updater.
Built as a separate executable so it has its own PySide6 runtime.
"""

import os
import shutil

# Clean previous build
if os.path.exists('build_updater'):
    shutil.rmtree('build_updater', ignore_errors=True)
if os.path.exists('dist_updater'):
    shutil.rmtree('dist_updater', ignore_errors=True)

a = Analysis(
    ['updater_gui.py'],
    pathex=[],
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
    name='ShobdoCalok_updater',
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
