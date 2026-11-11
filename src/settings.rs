//! Minimal INI settings compatible with the QSettings file the Python app
//! wrote (`AppData/settings.ini`, key `appearance/themeMode`).

use std::path::{Path, PathBuf};

pub struct Settings {
    path: PathBuf,
    sections: Vec<(String, Vec<(String, String)>)>,
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        let mut sections: Vec<(String, Vec<(String, String)>)> = vec![("General".into(), vec![])];
        if let Ok(text) = std::fs::read_to_string(path) {
            let mut cur = 0usize;
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                    continue;
                }
                if line.starts_with('[') && line.ends_with(']') {
                    let name = line[1..line.len() - 1].to_string();
                    cur = match sections.iter().position(|(n, _)| *n == name) {
                        Some(i) => i,
                        None => {
                            sections.push((name, vec![]));
                            sections.len() - 1
                        }
                    };
                } else if let Some((k, v)) = line.split_once('=') {
                    sections[cur].1.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
        }
        Self { path: path.to_path_buf(), sections }
    }

    /// `key` is "section/name" like QSettings.
    pub fn value(&self, key: &str, default: &str) -> String {
        let (sec, name) = key.split_once('/').unwrap_or(("General", key));
        self.sections
            .iter()
            .find(|(n, _)| n == sec)
            .and_then(|(_, kv)| kv.iter().find(|(k, _)| k == name))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| default.to_string())
    }

    pub fn set_value(&mut self, key: &str, value: &str) {
        let (sec, name) = key.split_once('/').unwrap_or(("General", key));
        let idx = match self.sections.iter().position(|(n, _)| n == sec) {
            Some(i) => i,
            None => {
                self.sections.push((sec.to_string(), vec![]));
                self.sections.len() - 1
            }
        };
        let kv = &mut self.sections[idx].1;
        match kv.iter_mut().find(|(k, _)| k == name) {
            Some(e) => e.1 = value.to_string(),
            None => kv.push((name.to_string(), value.to_string())),
        }
        self.sync();
    }

    pub fn sync(&self) {
        let mut out = String::new();
        for (name, kv) in &self.sections {
            if kv.is_empty() {
                continue;
            }
            out.push_str(&format!("[{name}]\r\n"));
            for (k, v) in kv {
                out.push_str(&format!("{k}={v}\r\n"));
            }
            out.push_str("\r\n");
        }
        let _ = std::fs::write(&self.path, out);
    }
}
