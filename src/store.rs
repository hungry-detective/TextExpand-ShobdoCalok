//! Port of viewmodels/snippet_engine.py — persistent snippet storage backed by
//! a JSON file (same format, so existing `AppData/snippets.json` files load).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Snippet {
    pub abbreviation: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Folder {
    pub name: String,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Library {
    pub folders: Vec<Folder>,
}

fn default_data() -> Library {
    let sn = |a: &str, c: &str| Snippet { abbreviation: a.into(), content: c.into() };
    Library {
        folders: vec![
            Folder {
                name: "General".into(),
                snippets: vec![sn(".hello", "Hello! How are you today?"), sn(".bye", "Goodbye! Take care.")],
            },
            Folder {
                name: "Examples".into(),
                snippets: vec![sn(".date", "Today is {date}."), sn(".clip", "You copied: {clipboard}")],
            },
            Folder {
                name: "Code Snippets".into(),
                snippets: vec![sn(".py", "if __name__ == '__main__':\n    main()")],
            },
        ],
    }
}

pub struct SnippetStore {
    filepath: PathBuf,
    data: Library,
    abbrev_cache: Option<HashMap<String, String>>,
}

impl SnippetStore {
    pub fn new(filepath: &Path) -> Self {
        let mut s = Self { filepath: filepath.to_path_buf(), data: Library::default(), abbrev_cache: None };
        s.load();
        s
    }

    // ── Persistence ──────────────────────────────────────────────────────────

    pub fn load(&mut self) {
        if let Ok(text) = std::fs::read_to_string(&self.filepath) {
            if let Ok(lib) = serde_json::from_str::<Library>(text.trim_start_matches('\u{feff}')) {
                self.data = lib;
                self.abbrev_cache = None;
                return;
            }
        }
        // First run or corrupted file — use defaults
        self.data = default_data();
        self.abbrev_cache = None;
        self.save();
    }

    pub fn save(&self) {
        match serde_json::to_string_pretty(&self.data) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.filepath, json) {
                    eprintln!("[SnippetStore] Failed to save: {e}");
                }
            }
            Err(e) => eprintln!("[SnippetStore] Failed to save: {e}"),
        }
    }

    // ── Import / Export ──────────────────────────────────────────────────────

    pub fn export_data(&self) -> Library {
        self.data.clone()
    }

    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.data).unwrap_or_else(|_| "{}".into())
    }

    /// Replace the whole library with `data`. Returns false if invalid.
    pub fn import_value(&mut self, v: &serde_json::Value) -> bool {
        let Some(folders) = v.get("folders").and_then(|f| f.as_array()) else { return false };
        for folder in folders {
            if !folder.is_object() || !folder.get("name").map(|n| n.is_string()).unwrap_or(false) {
                return false;
            }
            let Some(sns) = folder.get("snippets").and_then(|s| s.as_array()) else { return false };
            for sn in sns {
                if !sn.is_object() {
                    return false;
                }
                if !sn.get("abbreviation").map(|a| a.is_string()).unwrap_or(false)
                    || !sn.get("content").map(|a| a.is_string()).unwrap_or(false)
                {
                    return false;
                }
            }
        }
        match serde_json::from_value::<Library>(v.clone()) {
            Ok(lib) => {
                self.data = lib;
                self.abbrev_cache = None;
                self.save();
                true
            }
            Err(_) => false,
        }
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    pub fn folder_names(&self) -> Vec<String> {
        self.data.folders.iter().map(|f| f.name.clone()).collect()
    }

    pub fn snippets_for_folder(&self, folder: &str) -> Vec<Snippet> {
        self.data.folders.iter().find(|f| f.name == folder).map(|f| f.snippets.clone()).unwrap_or_default()
    }

    pub fn folders(&self) -> &[Folder] {
        &self.data.folders
    }

    /// {abbreviation: content} for ALL snippets (cached).
    pub fn abbreviations_map(&mut self) -> &HashMap<String, String> {
        if self.abbrev_cache.is_none() {
            let mut m = HashMap::new();
            for f in &self.data.folders {
                for s in &f.snippets {
                    m.insert(s.abbreviation.clone(), s.content.clone());
                }
            }
            self.abbrev_cache = Some(m);
        }
        self.abbrev_cache.as_ref().unwrap()
    }

    pub fn get_snippet(&self, folder: &str, abbrev: &str) -> Option<Snippet> {
        self.data
            .folders
            .iter()
            .find(|f| f.name == folder)
            .and_then(|f| f.snippets.iter().find(|s| s.abbreviation == abbrev).cloned())
    }

    // ── Folder mutations ─────────────────────────────────────────────────────

    pub fn add_folder(&mut self, name: &str) -> bool {
        if self.data.folders.iter().any(|f| f.name == name) {
            return false;
        }
        self.data.folders.push(Folder { name: name.into(), snippets: vec![] });
        self.abbrev_cache = None;
        self.save();
        true
    }

    pub fn rename_folder(&mut self, old: &str, new: &str) -> bool {
        if let Some(f) = self.data.folders.iter_mut().find(|f| f.name == old) {
            f.name = new.into();
            self.save();
            return true;
        }
        false
    }

    pub fn delete_folder(&mut self, name: &str) -> bool {
        let before = self.data.folders.len();
        self.data.folders.retain(|f| f.name != name);
        if self.data.folders.len() < before {
            self.abbrev_cache = None;
            self.save();
            return true;
        }
        false
    }

    // ── Snippet mutations ────────────────────────────────────────────────────

    pub fn add_snippet(&mut self, folder: &str, abbrev: &str, content: &str) -> bool {
        if let Some(f) = self.data.folders.iter_mut().find(|f| f.name == folder) {
            if f.snippets.iter().any(|s| s.abbreviation == abbrev) {
                return false;
            }
            f.snippets.push(Snippet { abbreviation: abbrev.into(), content: content.into() });
            self.abbrev_cache = None;
            self.save();
            return true;
        }
        false
    }

    pub fn update_snippet(&mut self, folder: &str, abbrev: &str, new_abbrev: &str, new_content: &str) -> bool {
        for f in self.data.folders.iter_mut() {
            if f.name == folder {
                if new_abbrev != abbrev && f.snippets.iter().any(|s| s.abbreviation == new_abbrev) {
                    return false;
                }
                if let Some(sn) = f.snippets.iter_mut().find(|s| s.abbreviation == abbrev) {
                    sn.abbreviation = new_abbrev.into();
                    sn.content = new_content.into();
                    self.abbrev_cache = None;
                    self.save();
                    return true;
                }
            }
        }
        false
    }

    pub fn delete_snippet(&mut self, folder: &str, abbrev: &str) -> bool {
        for f in self.data.folders.iter_mut() {
            if f.name == folder {
                let before = f.snippets.len();
                f.snippets.retain(|s| s.abbreviation != abbrev);
                if f.snippets.len() < before {
                    self.abbrev_cache = None;
                    self.save();
                    return true;
                }
            }
        }
        false
    }

    pub fn move_snippet(&mut self, abbrev: &str, from: &str, to: &str) -> bool {
        let mut snippet = None;
        if let Some(f) = self.data.folders.iter_mut().find(|f| f.name == from) {
            if let Some(i) = f.snippets.iter().position(|s| s.abbreviation == abbrev) {
                snippet = Some(f.snippets.remove(i));
            }
        }
        let Some(sn) = snippet else { return false };
        if let Some(f) = self.data.folders.iter_mut().find(|f| f.name == to) {
            f.snippets.push(sn);
            self.abbrev_cache = None;
            self.save();
            return true;
        }
        // Destination missing — the snippet is dropped, exactly like the original.
        false
    }

    // ── Order persistence ────────────────────────────────────────────────────

    pub fn reorder_folders(&mut self, names: &[String]) {
        let mut ordered = vec![];
        let mut rest = std::mem::take(&mut self.data.folders);
        for n in names {
            if let Some(i) = rest.iter().position(|f| &f.name == n) {
                ordered.push(rest.remove(i));
            }
        }
        ordered.extend(rest);
        self.data.folders = ordered;
        self.save();
    }

    pub fn reorder_snippets(&mut self, folder: &str, names: &[String]) {
        if let Some(f) = self.data.folders.iter_mut().find(|f| f.name == folder) {
            let mut ordered = vec![];
            let mut rest = std::mem::take(&mut f.snippets);
            for n in names {
                if let Some(i) = rest.iter().position(|s| &s.abbreviation == n) {
                    ordered.push(rest.remove(i));
                }
            }
            ordered.extend(rest);
            f.snippets = ordered;
            self.abbrev_cache = None;
            self.save();
        }
    }
}
