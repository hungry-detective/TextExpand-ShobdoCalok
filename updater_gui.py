"""
ShobdoCalok GUI Updater — replaces the CMD batch script.
Launched by the main app before it exits. Copies files and restarts.
"""
import sys
import os
import time
import shutil
import subprocess

# Find PySide6 from the parent app's _internal directory
_app_dir = os.path.dirname(os.path.abspath(__file__)).rstrip(os.sep)
# When running as frozen app, _internal is next to this script
_internal = os.path.join(_app_dir, "_internal")
if os.path.isdir(_internal):
    sys.path.insert(0, _internal)

from PySide6.QtWidgets import QApplication, QWidget, QVBoxLayout, QHBoxLayout, QLabel, QProgressBar
from PySide6.QtCore import Qt, QTimer, QThread, Signal
from PySide6.QtGui import QFont, QIcon


class CopyWorker(QThread):
    progress = Signal(int, str)  # percent, message
    finished = Signal(bool, str)  # success, message

    def __init__(self, src_dir, dst_dir):
        super().__init__()
        self.src_dir = src_dir
        self.dst_dir = dst_dir

    def run(self):
        try:
            self.progress.emit(5, "Preparing to copy files...")

            # Count total files
            total_files = 0
            for root, dirs, files in os.walk(self.src_dir):
                dirs[:] = [d for d in dirs if d != "AppData"]
                total_files += len(files)

            if total_files == 0:
                self.finished.emit(False, "No files found in update package!")
                return

            self.progress.emit(10, f"Found {total_files} files to copy...")
            time.sleep(0.5)

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

                    # Skip locked files by retrying
                    for attempt in range(3):
                        try:
                            shutil.copy2(src_file, dst_file)
                            break
                        except PermissionError:
                            if attempt < 2:
                                time.sleep(1)
                            else:
                                errors.append(fname)

                    copied += 1
                    pct = int(10 + (copied / total_files) * 80)
                    self.progress.emit(pct, f"Copying: {fname} ({copied}/{total_files})")

            if errors:
                self.progress.emit(95, f"Copied {copied} files ({len(errors)} skipped)")
            else:
                self.progress.emit(95, f"All {copied} files copied successfully!")

            time.sleep(0.3)
            self.finished.emit(True, f"Update installed! {copied} files copied.")

        except Exception as e:
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
        self.setFixedSize(420, 200)
        self.setWindowFlags(Qt.WindowStaysOnTopHint | Qt.Dialog)
        self.setStyleSheet("""
            QWidget { background-color: #1a1b2e; color: white; }
            QLabel { color: #e2e8f0; }
        """)

        # Center on screen
        screen = QApplication.primaryScreen().availableGeometry()
        self.move(
            screen.x() + (screen.width() - 420) // 2,
            screen.y() + (screen.height() - 200) // 2
        )

        layout = QVBoxLayout(self)
        layout.setSpacing(12)
        layout.setContentsMargins(24, 20, 24, 20)

        # Title
        title = QLabel("ShobdoCalok Updater")
        title.setFont(QFont("Inter", 14, QFont.Weight.Bold))
        title.setStyleSheet("color: #a78bfa;")
        title.setAlignment(Qt.AlignCenter)
        layout.addWidget(title)

        # Status
        self.status_label = QLabel("Preparing update...")
        self.status_label.setFont(QFont("Inter", 10))
        self.status_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.status_label)

        # Progress bar
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        self.progress_bar.setFixedHeight(8)
        self.progress_bar.setStyleSheet("""
            QProgressBar { border: none; border-radius: 4px; background-color: #2a2b3e; }
            QProgressBar::chunk { border-radius: 4px; background-color: #a78bfa; }
        """)
        layout.addWidget(self.progress_bar)

        # Detail
        self.detail_label = QLabel("")
        self.detail_label.setFont(QFont("Inter", 8))
        self.detail_label.setStyleSheet("color: #64748b;")
        self.detail_label.setAlignment(Qt.AlignCenter)
        layout.addWidget(self.detail_label)

        # Start copying after a brief delay
        QTimer.singleShot(800, self.start_copy)

    def start_copy(self):
        self.worker = CopyWorker(self.src_dir, self.dst_dir)
        self.worker.progress.connect(self.on_progress)
        self.worker.finished.connect(self.on_finished)
        self.worker.start()

    def on_progress(self, pct, msg):
        self.progress_bar.setValue(pct)
        self.status_label.setText(msg)

    def on_finished(self, success, msg):
        self.status_label.setText(msg)
        self.detail_label.setText("Starting ShobdoCalok...")
        self.progress_bar.setValue(100)

        # Cleanup
        try:
            if os.path.isdir(self.extract_dir):
                shutil.rmtree(self.extract_dir, ignore_errors=True)
        except Exception:
            pass

        # Launch app
        time.sleep(0.5)
        try:
            subprocess.Popen([self.app_exe], cwd=os.path.dirname(self.app_exe))
        except Exception:
            pass

        # Cleanup update dir after delay
        QTimer.singleShot(2000, self.cleanup_and_close)

    def cleanup_and_close(self):
        try:
            if os.path.isdir(self.update_dir):
                shutil.rmtree(self.update_dir, ignore_errors=True)
        except Exception:
            pass
        QApplication.quit()


def main():
    if len(sys.argv) < 6:
        print("Usage: updater_gui.py <src_dir> <dst_dir> <app_exe> <extract_dir> <update_dir>")
        sys.exit(1)

    src_dir = sys.argv[1]
    dst_dir = sys.argv[2]
    app_exe = sys.argv[3]
    extract_dir = sys.argv[4]
    update_dir = sys.argv[5]

    app = QApplication(sys.argv)

    window = UpdaterWindow(src_dir, dst_dir, app_exe, extract_dir, update_dir)
    window.show()

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
