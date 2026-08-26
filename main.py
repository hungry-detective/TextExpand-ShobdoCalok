import sys
import os
import platform
from PySide6.QtWidgets import QApplication, QSystemTrayIcon, QMenu
from PySide6.QtGui import QFontDatabase, QIcon, QAction
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtCore import Qt, QUrl, QObject, Slot, QTimer

# Register QmlElements
import viewmodels.header
import viewmodels.main as vm_main

if platform.system() == "Windows":
    import ctypes
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32

    def _capture_foreground():
        try:
            return user32.GetForegroundWindow()
        except Exception:
            return None

    def _restore_foreground(hwnd):
        if not hwnd:
            return
        try:
            # Windows only lets SetForegroundWindow succeed if the calling
            # thread "owns" the input. Attach our thread to the target window's
            # thread to reliably restore keyboard focus to that app.
            cur_tid = kernel32.GetCurrentThreadId()
            try:
                tgt_tid = user32.GetWindowThreadProcessId(hwnd, None)
            except Exception:
                tgt_tid = 0
            attached = False
            if tgt_tid and tgt_tid != cur_tid:
                try:
                    attached = bool(user32.AttachThreadInput(cur_tid, tgt_tid, True))
                except Exception:
                    attached = False
            # Only un-minimize; never resize. SW_RESTORE on a maximized window
            # would shrink it, so skip it unless the window is minimized.
            try:
                if user32.IsIconic(hwnd):
                    user32.ShowWindow(hwnd, 9)  # SW_RESTORE (un-minimize only)
            except Exception:
                pass
            user32.SetForegroundWindow(hwnd)
            user32.BringWindowToTop(hwnd)
            if attached:
                try:
                    user32.AttachThreadInput(cur_tid, tgt_tid, False)
                except Exception:
                    pass
        except Exception:
            pass
else:
    def _capture_foreground():
        return None

    def _restore_foreground(hwnd):
        pass

if __name__ == "__main__":
    # ── Windows Taskbar Icon Fix ─────────────────────────────────────────────
    if platform.system() == "Windows":
        import ctypes
        myappid = 'com.shobdocalok.v1' # arbitrary string
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(myappid)

    # ── PyInstaller fixes ──────────────────────────────────────────────────────
    from multiprocessing import freeze_support
    freeze_support() # Essential to prevent double GUI on Windows with pynput/PySide6
    
    os.environ["QT_QUICK_CONTROLS_STYLE"] = "Fusion"

    QApplication.setHighDpiScaleFactorRoundingPolicy(Qt.HighDpiScaleFactorRoundingPolicy.PassThrough)

    app = QApplication(sys.argv)
    app.setOrganizationName("ShobdoCalok")
    app.setApplicationName("Shobdo Calok")
    app.setQuitOnLastWindowClosed(False)

    # ── Path resolution for PyInstaller ────────────────────────────────────────
    if getattr(sys, 'frozen', False):
        # Bundled as a single executable or folder
        _bundle_dir = sys._MEIPASS
        # For portable data, use the directory where the EXE is actually located
        _app_root = os.path.dirname(sys.executable)
    else:
        # Running as a script
        _bundle_dir = os.path.dirname(os.path.abspath(__file__))
        _app_root = _bundle_dir

    # Load fonts
    font_path = os.path.join(_bundle_dir, "MaterialSymbolsOutlined.ttf")
    if os.path.exists(font_path):
        font_id = QFontDatabase.addApplicationFont(font_path)
        if font_id != -1:
            families = QFontDatabase.applicationFontFamilies(font_id)
            print(f"Loaded font families: {families}")

    # ── Data folder ─────────────────────────────────────────────────────────────
    # Use an 'AppData/' folder next to the EXE (if frozen) or script — fully portable
    data_dir = os.path.join(_app_root, "AppData")
    os.makedirs(data_dir, exist_ok=True)

    # ── Backend: Snippet store + expansion engine ──────────────────────────────
    snippet_vm = vm_main.SnippetViewModel(data_dir=data_dir)

    from viewmodels.google_drive import GoogleDriveViewModel
    drive_vm = GoogleDriveViewModel(data_dir=data_dir)
    drive_vm.set_snippet_store(snippet_vm.store)

    from viewmodels.expansion_engine import ExpansionEngine
    from viewmodels.field_dialog import FieldFillDialog
    from viewmodels.updater import UpdaterViewModel

    expansion_engine = ExpansionEngine(snippet_vm.store)
    snippet_vm.set_engine(expansion_engine)

    def _on_field_fill_requested(labels: list, key: str, content: str):
        """Called on main thread when {field:Label} tokens are encountered."""
        fg = _capture_foreground()
        dlg = FieldFillDialog(labels, content, is_dark=snippet_vm.isDark)
        accepted = dlg.exec()
        # Restore focus to the app the user was typing in BEFORE the paste
        # runs, so the inserted text keeps keyboard focus in that input field.
        _restore_foreground(fg)
        result = dlg.results if accepted else {}
        QTimer.singleShot(120, lambda: expansion_engine.resolve_fields(key, result))

    expansion_engine.fieldFillRequested.connect(_on_field_fill_requested, Qt.ConnectionType.QueuedConnection)

    # ── Startup with retry ──────────────────────────────────────────────────
    # The keyboard hook (WH_KEYBOARD_LL) can fail to install on Windows startup
    # when the input subsystem isn't fully ready. Retry with backoff.
    import time as _time
    _time.sleep(0.5)  # Brief initial delay for Windows to finish booting
    _max_retries = 5
    _started = False
    for _attempt in range(_max_retries):
        print(f"[Main] Starting expansion engine (attempt {_attempt + 1}/{_max_retries})")
        if expansion_engine.start():
            print("[Main] Expansion engine started successfully")
            _started = True
            break
        print(f"[Main] Attempt {_attempt + 1} failed, retrying...")
        _time.sleep(0.5 + _attempt * 0.5)  # 0.5, 1.0, 1.5, 2.0, 2.5 second delays
    if not _started:
        print("[Main] WARNING: Expansion engine failed to start after all retries")

    # ── QML Engine ─────────────────────────────────────────────────────────────
    engine = QQmlApplicationEngine()
    engine.addImportPath(_bundle_dir)

    updater_vm = UpdaterViewModel(data_dir=data_dir)

    # Expose snippetViewModel to all QML files BEFORE loading
    engine.rootContext().setContextProperty("snippetViewModel", snippet_vm)
    engine.rootContext().setContextProperty("driveViewModel", drive_vm)
    engine.rootContext().setContextProperty("updaterViewModel", updater_vm)

    main_qml = os.path.join(_bundle_dir, "main.qml")
    engine.load(QUrl.fromLocalFile(main_qml))

    if not engine.rootObjects():
        expansion_engine.stop()
        sys.exit(-1)

    root_window = engine.rootObjects()[0]

    # Push persisted isDark preference into AppTheme via the root object
    try:
        root_window.setProperty("isDarkFromSettings", snippet_vm.isDark)
    except Exception:
        pass

    # Centre window
    screen = QApplication.primaryScreen().geometry()
    root_window.setX((screen.width()  - root_window.width())  / 2)
    root_window.setY((screen.height() - root_window.height()) / 2)

    # Native frameless window effects (Windows only)
    if platform.system() == "Windows":
        from framelesswindow.win import WindowsEventFilter, WindowsWindowEffect
        event_filter = WindowsEventFilter(border_width=5)
        app.installNativeEventFilter(event_filter)
        try:
            hwnd = root_window.winId()
            effects = WindowsWindowEffect()
            effects.addShadowEffect(hwnd)
            effects.addWindowAnimation(hwnd)
        except Exception as e:
            print(f"Failed to apply window effects: {e}")

        # Restart keyboard hook after Windows sleep/resume
        def _on_system_resume():
            print("[Main] System resumed — restarting expansion engine")
            expansion_engine.restart()

        event_filter.on_resume_callback = _on_system_resume

    # ── System Tray ────────────────────────────────────────────────────────────
    # Resolve tray icon path correctly for bundle. Prefer the crisp multi-size
    # ICO (renders best in the Windows tray), then PNG, then SVG.
    _tray_icon_path = os.path.join(_bundle_dir, "app-icon.ico")
    if not os.path.exists(_tray_icon_path):
        _tray_icon_path = os.path.join(_bundle_dir, "app-icon.png")
    if not os.path.exists(_tray_icon_path):
        _tray_icon_path = os.path.join(_bundle_dir, "app-icon.svg")
    
    _icon = QIcon(_tray_icon_path)
    app.setWindowIcon(_icon) # Sets the default window icon for the entire app
    if root_window:
        root_window.setIcon(_icon)

    tray_icon = QSystemTrayIcon(_icon, app)
    tray_icon.setToolTip("Shobdo Calok")

    tray_menu = QMenu()

    if snippet_vm.isDark:
        _menu_qss = """
QMenu {
    background-color: #24282e;
    color: #f1f5f9;
    border: 1px solid #3f3f46;
    border-radius: 10px;
    padding: 6px;
    font-family: "Segoe UI";
    font-size: 12px;
    icon-size: 20px;
}
QMenu::item {
    background-color: transparent;
    padding: 7px 28px 7px 12px;
    border-radius: 6px;
}
QMenu::item:selected { background-color: #7c3aed; color: #ffffff; }
QMenu::item:disabled { color: #94a3b8; }
QMenu::separator {
    height: 1px;
    background: #3f3f46;
    margin: 5px 10px;
}
"""
    else:
        _menu_qss = """
QMenu {
    background-color: #f8f9fa;
    color: #0f172a;
    border: 1px solid #e2e8f0;
    border-radius: 10px;
    padding: 6px;
    font-family: "Segoe UI";
    font-size: 12px;
    icon-size: 20px;
}
QMenu::item {
    background-color: transparent;
    padding: 7px 28px 7px 12px;
    border-radius: 6px;
}
QMenu::item:selected { background-color: #7c3aed; color: #ffffff; }
QMenu::item:disabled { color: #94a3b8; }
QMenu::separator {
    height: 1px;
    background: #e2e8f0;
    margin: 5px 10px;
}
"""
    tray_menu.setStyleSheet(_menu_qss)

    _show_icon   = QIcon(os.path.join(_bundle_dir, "icon-tray-show.svg"))
    _pause_icon  = QIcon(os.path.join(_bundle_dir, "icon-tray-pause.svg"))
    _resume_icon = QIcon(os.path.join(_bundle_dir, "icon-tray-resume.svg"))
    _quit_icon   = QIcon(os.path.join(_bundle_dir, "icon-tray-quit.svg"))
    _update_icon = QIcon(os.path.join(_bundle_dir, "icon-tray-update.svg"))

    show_action    = QAction(_show_icon,  "Show Application")
    toggle_action  = QAction(_pause_icon, "Pause Expansion")
    update_action  = QAction(_update_icon, "Check for Updates")
    quit_action    = QAction(_quit_icon,  "Quit")

    show_action.triggered.connect(root_window.showNormal)
    show_action.triggered.connect(root_window.raise_)
    quit_action.triggered.connect(app.quit)

    def _check_for_updates():
        updater_vm.checkForUpdate()
        tray_icon.showMessage(
            "Shobdo Calok",
            "Checking for updates...",
            QSystemTrayIcon.MessageIcon.Information,
            2000
        )

    update_action.triggered.connect(_check_for_updates)

    def _toggle_expansion():
        enabled = snippet_vm.toggleEnabled()
        toggle_action.setIcon(_resume_icon if not enabled else _pause_icon)
        toggle_action.setText("Resume Expansion" if not enabled else "Pause Expansion")
        tray_icon.showMessage(
            "Shobdo Calok",
            "Text expansion enabled" if enabled else "Text expansion paused",
            QSystemTrayIcon.MessageIcon.Information,
            2000
        )

    toggle_action.triggered.connect(_toggle_expansion)

    def _on_updater_status(msg):
        if msg and "available" in msg.lower():
            tray_icon.showMessage("Shobdo Calok — Update Available", msg,
                                 QSystemTrayIcon.MessageIcon.Information, 5000)

    updater_vm.statusMessage.connect(_on_updater_status)

    tray_menu.addAction(show_action)
    tray_menu.addSeparator()
    tray_menu.addAction(toggle_action)
    tray_menu.addAction(update_action)
    tray_menu.addSeparator()
    tray_menu.addAction(quit_action)

    tray_icon.setContextMenu(tray_menu)
    tray_icon.show()
    
    # Show on double click
    tray_icon.activated.connect(
        lambda reason: (root_window.showNormal(), root_window.raise_()) \
        if reason == QSystemTrayIcon.ActivationReason.DoubleClick else None
    )

    # Periodic health check: restart the keyboard hook if it died unexpectedly
    # Uses two phases: fast checks at startup (every 500ms for 30s), then slower (5s)
    _health_phase = [0]  # Use list for closure mutability
    _health_start_time = _time.time()

    def _health_check():
        if expansion_engine.is_enabled and not expansion_engine.is_alive():
            print("[Main] Keyboard hook is dead — auto-restarting")
            expansion_engine.restart()

        # Switch from fast to slow phase after 30 seconds
        elapsed = _time.time() - _health_start_time
        if _health_phase[0] == 0 and elapsed > 30:
            _health_phase[0] = 1
            _health_timer.setInterval(5000)
            print("[Main] Health check: switching to normal 5s interval")

    _health_timer = QTimer()
    _health_timer.timeout.connect(_health_check)
    _health_timer.start(500)  # Start with fast 500ms checks for first 30 seconds

    def _cleanup():
        expansion_engine.stop()

    app.aboutToQuit.connect(_cleanup)

    sys.exit(app.exec())
