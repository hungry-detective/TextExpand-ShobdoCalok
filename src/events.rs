//! Background → UI notifications (Qt signals in the original).

use parking_lot::Mutex;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct BackupItem {
    pub id: String,
    pub name: String,
    pub backup_name: String,
    pub time_label: String,
    pub label: String,
}

#[derive(Clone, Debug)]
pub enum Event {
    /// updaterViewModel.statusMessage
    UpdaterStatus(String),
    /// updaterViewModel.updateAvailableChanged
    UpdateAvailableChanged,
    /// driveViewModel.statusMessage
    DriveStatus(String),
    /// driveViewModel.loginStateChanged
    DriveLoginChanged(bool),
    /// driveViewModel.backupListReady
    DriveBackupList(Vec<BackupItem>),
    /// driveViewModel.restoreComplete
    DriveRestoreComplete,
    /// Tray → "Check for Updates" (navigate to About)
    ShowAboutPage,
}

static TX: OnceLock<Mutex<Sender<Event>>> = OnceLock::new();

pub fn channel() -> Receiver<Event> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = TX.set(Mutex::new(tx));
    rx
}

pub fn emit(e: Event) {
    if let Some(tx) = TX.get() {
        let _ = tx.lock().send(e);
    }
    crate::request_repaint();
}
