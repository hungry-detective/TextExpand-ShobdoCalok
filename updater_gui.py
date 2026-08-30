"""
ShobdoCalok GUI Updater — standalone process that runs after the main app exits.
Shows a progress window while replacing files, then relaunches the app.
"""
import os
import sys
import time
import shutil
import subprocess
from datetime import datetime

DEBUG_LOG_NAME = "update_debug.log"


def _log(message: str, **kwargs):
    """Write to update_debug.log in the AppData folder next to the main app exe."""
    try:
        # Log goes to AppData/update_debug.log next to ShobdoCalok.exe
        # app_root is sys.argv[2], AppData folder is app_root/AppData
        if len(sys.argv) >= 3:
            app_root = sys.argv[2]
            log_path = os.path.join(app_root, "AppData", DEBUG_LOG_NAME)
        else:
            log_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), DEBUG_LOG_NAME)
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        lines = [
            "",
            "=" * 70,
            f"[{timestamp}] GUI UPDATER",
            f"Message: {message}",
        ]
        for k, v in kwargs.items():
            sv = str(v)
            if len(sv) > 500:
                sv = sv[:500] + "..."
            lines.append(f"{k}: {sv}")
        lines.append(f"Python: {sys.version}")
        lines.append(f"Frozen: {getattr(sys, 'frozen', False)}")
        lines.append("=" * 70)
        lines.append("")
        with open(log_path, "a", encoding="utf-8") as f:
            f.write("\n".join(lines))
    except Exception as e:
        print(f"Failed to write updater log: {e}")


from PySide6.QtWidgets import QApplication, QWidget, QVBoxLayout, QLabel, QProgressBar
from PySide6.QtCore import Qt, QTimer, QThread, Signal
from PySide6.QtGui import QFont


class CopyWorker(QThread):
    progress = Signal(int, str)
    finished = Signal(bool, str)

    def __init__(self, src_dir, dst_dir, exe_dir):
        super().__init__()
        self.src_dir = src_dir
        self.dst_dir = dst_dir
        self.exe_dir = exe_dir

    def run(self):
        try:
            self.progress.emit(5, "Preparing to copy files...")
            _log("CopyWorker started", src_dir=self.src_dir, dst_dir=self.dst_dir)
            time.sleep(0.3)

            if not os.path.isdir(self.src_dir):
                _log("ERROR: source not found", src_dir=self.src_dir)
                self.finished.emit(False, f"Source not found: {self.src_dir}")
                return

            # Count total files
            total_files = 0
            for root, dirs, files in os.walk(self.src_dir):
                dirs[:] = [d for d in dirs if d != "AppData"]
                total_files += len(files)

            if total_files == 0:
                _log("ERROR: no files in update package")
                self.finished.emit(False, "No files found in update package!")
                return

            self.progress.emit(10, f"Found {total_files} files to copy...")
            _log(f"Found {total_files} files to copy")
            time.sleep(0.3)

            copied = 0
            errors = []
            for root, dirs, files in os.walk(self.src_dir):
                dirs[:] = [d for d in dirs if d != "AppData"]

                rel = os.path.relpath(root, self.src_dir)
                dst_sub = os.path.join(self.dst_dir, rel)

                os.makedirs(dst_sub, exist_ok=True)

                for fname in files:
                    src_file = os.path.join(root, fname)
                    dst_file = os.path.join(dst_sub, fname)

                    # Try copying with retries for locked files
                    success = False
                    last_error = ""
                    for attempt in range(5):
                        try:
                            shutil.copy2(src_file, dst_file)
                            success = True
                            break
                        except PermissionError as e:
                            last_error = str(e)
                            time.sleep(1)
                        except Exception as e:
                            last_error = str(e)
                            errors.append(f"{fname}: {e}")
                            break

                    if not success:
                        errors.append(f"{fname}: locked after 5 retries - {last_error}")

                    copied += 1
                    pct = 10 + int((copied / total_files) * 80)
                    if copied % 5 == 0 or copied == total_files:
                        self.progress.emit(pct, f"Copying: {fname} ({copied}/{total_files})")

            if errors:
                self.progress.emit(95, f"Copied {copied}/{total_files} files ({len(errors)} issues)")
                _log(f"Copy completed with errors: {copied}/{total_files} copied, {len(errors)} failed",
                     errors=errors[:10])
            else:
                self.progress.emit(95, f"All {copied} files copied!")
                _log(f"Copy completed successfully: {copied} files")

            time.sleep(0.5)
            self.finished.emit(True, f"Update complete! {copied} files installed.")
        except Exception as e:
            _log(f"CopyWorker exception: {e}")
            self.finished.emit(False, f"Error: {e}")


class UpdaterWindow(QWidget):
    def __init__(self, src_dir, dst_dir, app_exe, extract_dir, update_dir):
        super().__init__()
        self.src_dir = src_dir
        self.dst_dir = dst_dir
        self.app_exe = app_exe
        self.extract_dir = extract_dir
        self.update_dir = update_dir
        self.worker = None

        self.setWindowTitle("ShobdoCalok Updater")
        self.setFixedSize(460, 220)
        self.setWindowFlags(Qt.WindowStaysOnTopHint | Qt.Dialog | Qt.CustomizeWindowHint)
        self.setStyleSheet("background-color: #1a1b2e; color: white;")

        # Center on screen
        screen = QApplication.primaryScreen().availableGeometry()
        self.move(
            screen.x() + (screen.width() - 460) // 2,
            screen.y() + (screen.height() - 220) // 2
        )

        layout = QVBoxLayout(self)
        layout.setSpacing(14)
        layout.setContentsMargins(28, 22, 28, 22)

        title = QLabel("ShobdoCalok Updater")
        title.setFont(QFont("Inter", 15, QFont.Weight.Bold))
        title.setStyleSheet("color: #a78bfa;")
        title.setAlignment(Qt.AlignCenter)
        layout.addWidget(title)

        self.status_label = QLabel("Preparing update...")
        self.status_label.setFont(QFont("Inter", 10))
        self.status_label.setStyleSheet("color: #e2e8f0;")
        self.status_label.setAlignment(Qt.AlignCenter)
        self.status_label.setWordWrap(True)
        layout.addWidget(self.status_label)

        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        self.progress_bar.setFixedHeight(10)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.setStyleSheet("""
            QProgressBar { border: none; border-radius: 5px; background-color: #2a2b3e; }
            QProgressBar::chunk { border-radius: 5px; background-color: #a78bfa; }
        """)
        layout.addWidget(self.progress_bar)

        self.detail_label = QLabel("")
        self.detail_label.setFont(QFont("Inter", 8))
        self.detail_label.setStyleSheet("color: #64748b;")
        self.detail_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.detail_label)

        # Start copying after 500ms
        QTimer.singleShot(500, self.start_copy)

    def start_copy(self):
        self.worker = CopyWorker(self.src_dir, self.dst_dir, self.dst_dir)
        self.worker.progress.connect(self.on_progress)
        self.worker.finished.connect(self.on_finished)
        self.worker.start()

    def on_progress(self, pct, msg):
        self.progress_bar.setValue(pct)
        self.status_label.setText(msg)

    def on_finished(self, success, msg):
        self.progress_bar.setValue(100)
        self.status_label.setText(msg)
        self.detail_label.setText("Starting ShobdoCalok...")

        _log(f"Copy finished: success={success}, msg={msg}")

        # Cleanup extract and update dirs
        try:
            if os.path.isdir(self.extract_dir):
                shutil.rmtree(self.extract_dir, ignore_errors=True)
                _log(f"Removed extract dir: {self.extract_dir}")
        except Exception as e:
            _log(f"Failed to remove extract dir: {e}")

        # Launch app after brief delay
        def launch_and_close():
            _log(f"Launching app: {self.app_exe}")
            try:
                subprocess.Popen(
                    [self.app_exe],
                    cwd=os.path.dirname(self.app_exe),
                    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0)
                )
            except Exception as e:
                _log(f"Failed to launch app: {e}")
            # Cleanup update dir
            try:
                if os.path.isdir(self.update_dir):
                    shutil.rmtree(self.update_dir, ignore_errors=True)
                    _log(f"Removed update dir: {self.update_dir}")
            except Exception as e:
                _log(f"Failed to remove update dir: {e}")
            QApplication.quit()

        QTimer.singleShot(1000, launch_and_close)


def main():
    if len(sys.argv) < 6:
        _log("ERROR: not enough arguments", argv=sys.argv)
        QMessageBox = None
        try:
            from PySide6.QtWidgets import QMessageBox
            app = QApplication(sys.argv)
            QMessageBox.critical(None, "Updater Error",
                "Usage: updater_gui.py <src_dir> <dst_dir> <app_exe> <extract_dir> <update_dir>")
        except Exception:
            print("Usage: updater_gui.py <src_dir> <dst_dir> <app_exe> <extract_dir> <update_dir>")
        sys.exit(1)

    src_dir = sys.argv[1]
    dst_dir = sys.argv[2]
    app_exe = sys.argv[3]
    extract_dir = sys.argv[4]
    update_dir = sys.argv[5]

    _log("GUI updater started",
         src_dir=src_dir, dst_dir=dst_dir, app_exe=app_exe,
         extract_dir=extract_dir, update_dir=update_dir)

    app = QApplication(sys.argv)
    app.setQuitOnLastWindowClosed(True)

    window = UpdaterWindow(src_dir, dst_dir, app_exe, extract_dir, update_dir)
    window.show()
    window.raise_()
    window.activateWindow()

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
