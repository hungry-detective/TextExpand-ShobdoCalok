"""
viewmodels/main.py — SnippetViewModel exposed to QML.
"""
import json
import os
import sys
from typing import List

from PySide6.QtCore import QObject, Signal, Slot, Property, Qt, QSettings
from PySide6.QtQml import QmlElement

from .snippet_engine import SnippetStore

QML_IMPORT_NAME = "pyobjects"
QML_IMPORT_MAJOR_VERSION = 1


@QmlElement
class SnippetViewModel(QObject):
    """Bridge between the Python SnippetStore and the QML UI."""

    foldersChanged         = Signal()
    snippetsChanged        = Signal(str)            # emits folder name
    snippetSelected        = Signal(str, str, str)  # folder, abbrev, content
    expansionToggled       = Signal(bool)           # is_enabled
    startupEnabledChanged  = Signal(bool)
    themeChanged           = Signal(bool)
    themeModeChanged       = Signal(str)

    def __init__(self, data_dir: str = None, parent=None):
        super().__init__(parent)

        # ── Determine snippets.json path ───────────────────────────────────────
        if data_dir is None:
            data_dir = os.path.dirname(os.path.dirname(__file__))
        os.makedirs(data_dir, exist_ok=True)

        # Migrate old snippets.json from the project root if needed
        new_path = os.path.join(data_dir, "snippets.json")
        old_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), "snippets.json")
        if not os.path.exists(new_path) and os.path.exists(old_path):
            import shutil
            try:
                shutil.copy2(old_path, new_path)
                print(f"[SnippetViewModel] Migrated snippets.json -> {new_path}")
            except Exception as e:
                print(f"[SnippetViewModel] Migration failed: {e}")

        self._store  = SnippetStore(new_path)

        # ── QSettings for UI preferences — stored in data/settings.ini ────────
        settings_path = os.path.join(data_dir, "settings.ini")
        self._settings = QSettings(settings_path, QSettings.Format.IniFormat)

        self._engine = None          # set later by main.py
        self._current_folder   = ""
        self._current_abbrev   = ""
        self._current_content  = ""
        self._is_enabled       = True

        # Load persisted theme preference: "system", "dark", or "light"
        self._theme_mode = self._settings.value("appearance/themeMode", "system", type=str)
        if self._theme_mode not in ("system", "dark", "light"):
            self._theme_mode = "system"
        self._is_dark = self._compute_dark()

        self._startup_enabled  = self._check_startup()

    # ── Theme helpers ───────────────────────────────────────────────────────────

    def _system_is_dark(self) -> bool:
        """Ask Windows whether the system is in dark mode (registry)."""
        if sys.platform != "win32":
            return False
        try:
            import winreg
            key = winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            try:
                val, _ = winreg.QueryValueEx(key, "AppsUseLightTheme")
                # 0 => dark, 1 => light
                return int(val) == 0
            finally:
                winreg.CloseKey(key)
        except Exception:
            return False

    def _compute_dark(self) -> bool:
        if self._theme_mode == "dark":
            return True
        if self._theme_mode == "light":
            return False
        return self._system_is_dark()

    def set_engine(self, engine):
        self._engine = engine

    # ── Startup helpers ────────────────────────────────────────────────────────

    _APP_NAME = "ShobdoCalok"

    def _check_startup(self) -> bool:
        if sys.platform != "win32":
            return False
        try:
            import winreg
            key = winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Run", 0, winreg.KEY_READ)
            try:
                winreg.QueryValueEx(key, self._APP_NAME)
                return True
            except FileNotFoundError:
                return False
            finally:
                winreg.CloseKey(key)
        except Exception:
            return False

    # ── Properties ─────────────────────────────────────────────────────────────

    @Property(list, notify=foldersChanged)
    def folders(self) -> List[str]:
        return self._store.folder_names()

    @Property(bool, notify=expansionToggled)
    def isEnabled(self) -> bool:
        return self._is_enabled

    @Property(bool, notify=startupEnabledChanged)
    def startupEnabled(self) -> bool:
        return self._startup_enabled

    @Property(bool, notify=themeChanged)
    def isDark(self) -> bool:
        return self._is_dark

    @Property(str, notify=themeModeChanged)
    def themeMode(self) -> str:
        return self._theme_mode

    @Slot(str)
    def setThemeMode(self, mode: str):
        if mode not in ("system", "dark", "light"):
            return
        if mode != self._theme_mode:
            self._theme_mode = mode
            self._settings.setValue("appearance/themeMode", mode)
            self._settings.sync()
            self.themeModeChanged.emit(mode)
        new_dark = self._compute_dark()
        if new_dark != self._is_dark:
            self._is_dark = new_dark
            self.themeChanged.emit(new_dark)

    @Property(str)
    def currentFolder(self) -> str:
        return self._current_folder

    @Property(str)
    def currentAbbrev(self) -> str:
        return self._current_abbrev

    @Property(str)
    def currentContent(self) -> str:
        return self._current_content

    # ── Folder slots ───────────────────────────────────────────────────────────

    @Slot(str, result=bool)
    def addFolder(self, name: str) -> bool:
        ok = self._store.add_folder(name.strip())
        if ok:
            self.foldersChanged.emit()
        return ok

    @Slot(str, str, result=bool)
    def renameFolder(self, old_name: str, new_name: str) -> bool:
        new_name = new_name.strip()
        if not new_name or new_name == old_name:
            return False
        ok = self._store.rename_folder(old_name, new_name)
        if ok:
            self.foldersChanged.emit()
            if self._current_folder == old_name:
                self._current_folder = new_name
        return ok

    @Slot(str, result=bool)
    def deleteFolder(self, name: str) -> bool:
        ok = self._store.delete_folder(name)
        if ok:
            self.foldersChanged.emit()
        return ok

    # ── Reorder slots (persist drag-and-drop order) ────────────────────────────

    @Slot(list)
    def reorderFolders(self, names: list):
        """Called from QML after a folder drag-and-drop completes."""
        self._store.reorder_folders(names)
        # No foldersChanged emit — QML model is already visually correct

    @Slot(str, list)
    def reorderSnippets(self, folder_name: str, names: list):
        """Called from QML after a snippet drag-and-drop completes."""
        self._store.reorder_snippets(folder_name, names)
        # No snippetsChanged emit — QML model is already visually correct

    # ── Snippet slots ──────────────────────────────────────────────────────────

    @Slot(str, result=list)
    def snippetsForFolder(self, folder_name: str) -> List[dict]:
        """Return list of {abbreviation, content} dicts for a given folder."""
        return self._store.snippets_for_folder(folder_name)

    @Slot(str, str)
    def selectSnippet(self, folder_name: str, abbreviation: str):
        sn = self._store.get_snippet(folder_name, abbreviation)
        if sn:
            self._current_folder  = folder_name
            self._current_abbrev  = sn["abbreviation"]
            self._current_content = sn["content"]
            self.snippetSelected.emit(folder_name, sn["abbreviation"], sn["content"])

    @Slot(str, str, result=bool)
    def addSnippet(self, folder_name: str, abbreviation: str) -> bool:
        ok = self._store.add_snippet(folder_name, abbreviation.strip())
        if ok:
            self.snippetsChanged.emit(folder_name)
            if self._engine:
                self._engine.refresh_store()
        return ok

    @Slot(str, str, str, str, result=bool)
    def updateSnippet(self, folder_name: str, old_abbrev: str,
                      new_abbrev: str, new_content: str) -> bool:
        ok = self._store.update_snippet(folder_name, old_abbrev,
                                        new_abbrev.strip(), new_content)
        if ok:
            self._current_abbrev  = new_abbrev.strip()
            self._current_content = new_content
            self.snippetsChanged.emit(folder_name)
            if self._engine:
                self._engine.refresh_store()
        return ok

    @Slot(str, str, result=bool)
    def deleteSnippet(self, folder_name: str, abbreviation: str) -> bool:
        ok = self._store.delete_snippet(folder_name, abbreviation)
        if ok:
            self.snippetsChanged.emit(folder_name)
            if self._engine:
                self._engine.refresh_store()
        return ok

    @Slot(str, str, result=bool)
    def duplicateSnippet(self, folder_name: str, abbreviation: str) -> bool:
        sn = self._store.get_snippet(folder_name, abbreviation)
        if not sn:
            return False
        existing = self._store.abbreviations_map()
        new_abbrev = abbreviation + "_copy"
        counter = 1
        while new_abbrev in existing:
            counter += 1
            new_abbrev = f"{abbreviation}_copy{counter}"
        ok = self._store.add_snippet(folder_name, new_abbrev, sn["content"])
        if ok:
            self.snippetsChanged.emit(folder_name)
            if self._engine:
                self._engine.refresh_store()
        return ok

    @Slot(str, result=str)
    def previewExpansion(self, content: str) -> str:
        """Resolve tokens for a live editor preview."""
        if self._engine is None:
            return content
        return self._engine.preview(content)

    @Slot(result=str)
    def exportSnippets(self) -> str:
        from PySide6.QtWidgets import QFileDialog
        path, _ = QFileDialog.getSaveFileName(
            None, "Export Snippets",
            os.path.join(os.path.expanduser("~"), "shobdocalok_backup.json"),
            "JSON (*.json)")
        if not path:
            return ""
        try:
            with open(path, "w", encoding="utf-8") as f:
                json.dump(self._store.export_data(), f, indent=2, ensure_ascii=False)
            return path
        except Exception as e:
            return f"Error: {e}"

    @Slot(result=str)
    def importSnippets(self) -> str:
        from PySide6.QtWidgets import QFileDialog
        path, _ = QFileDialog.getOpenFileName(None, "Import Snippets", "", "JSON (*.json)")
        if not path:
            return ""
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
            if not self._store.import_data(data):
                return "Invalid file: missing 'folders'."
            self.foldersChanged.emit()
            if self._engine:
                self._engine.refresh_store()
            return path
        except Exception as e:
            return f"Error: {e}"

    @Slot(str, str, str, result=bool)
    def moveSnippet(self, abbreviation: str, from_folder: str, to_folder: str) -> bool:
        ok = self._store.move_snippet(abbreviation, from_folder, to_folder)
        if ok:
            self.snippetsChanged.emit(from_folder)
            self.snippetsChanged.emit(to_folder)
            if self._engine:
                self._engine.refresh_store()
        return ok

    # ── Enable/Disable ─────────────────────────────────────────────────────────

    @Slot(bool)
    def setEnabled(self, enabled: bool):
        self._is_enabled = enabled
        if self._engine:
            self._engine.set_enabled(enabled)
        self.expansionToggled.emit(enabled)

    @Slot(result=bool)
    def toggleEnabled(self) -> bool:
        self.setEnabled(not self._is_enabled)
        return self._is_enabled

    # ── Startup toggle ─────────────────────────────────────────────────────────

    @Slot(bool)
    def setStartup(self, enabled: bool):
        if sys.platform != "win32":
            return
        try:
            import winreg
            key = winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                r"Software\Microsoft\Windows\CurrentVersion\Run", 0,
                winreg.KEY_SET_VALUE)
            if enabled:
                exe_path = sys.executable
                script_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), "main.py")
                winreg.SetValueEx(key, self._APP_NAME, 0, winreg.REG_SZ,
                    f'"{exe_path}" "{script_path}"')
            else:
                try:
                    winreg.DeleteValue(key, self._APP_NAME)
                except FileNotFoundError:
                    pass
            winreg.CloseKey(key)
            self._startup_enabled = enabled
            self.startupEnabledChanged.emit(enabled)
        except Exception as e:
            print(f"Startup toggle error: {e}")

    # ── Store accessor (for engine) ────────────────────────────────────────────

    @property
    def store(self) -> SnippetStore:
        return self._store
