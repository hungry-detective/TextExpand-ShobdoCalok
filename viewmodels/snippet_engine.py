"""
snippet_engine.py — persistent snippet storage backed by a JSON file.
"""

import json
import os
from typing import List, Dict, Optional

DEFAULT_DATA = {
    "folders": [
        {
            "name": "General",
            "snippets": [
                {"abbreviation": ".hello", "content": "Hello! How are you today?"},
                {"abbreviation": ".bye",   "content": "Goodbye! Take care."}
            ]
        },
        {
            "name": "Examples",
            "snippets": [
                {"abbreviation": ".date",  "content": "Today is {date}."},
                {"abbreviation": ".clip",  "content": "You copied: {clipboard}"}
            ]
        },
        {
            "name": "Code Snippets",
            "snippets": [
                {"abbreviation": ".py",    "content": "if __name__ == '__main__':\n    main()"}
            ]
        }
    ]
}


class SnippetStore:
    """Load, save and mutate snippet data from a JSON file."""

    def __init__(self, filepath: str):
        self.filepath = filepath
        self._data: Dict = {}
        self._abbrev_cache: Optional[Dict[str, str]] = None
        self.load()

    # ── Persistence ────────────────────────────────────────────────────────────

    def load(self):
        if os.path.exists(self.filepath):
            try:
                with open(self.filepath, "r", encoding="utf-8") as f:
                    self._data = json.load(f)
                return
            except (json.JSONDecodeError, IOError):
                pass
        # First run or corrupted file — use defaults
        self._data = json.loads(json.dumps(DEFAULT_DATA))  # deep copy
        self.save()

    def save(self):
        try:
            with open(self.filepath, "w", encoding="utf-8") as f:
                json.dump(self._data, f, indent=2, ensure_ascii=False)
        except IOError as e:
            print(f"[SnippetStore] Failed to save: {e}")

    # ── Import / Export ────────────────────────────────────────────────────────

    def export_data(self) -> Dict:
        """Return a deep copy of the raw data (safe to serialise)."""
        return json.loads(json.dumps(self._data))

    def import_data(self, data: Dict) -> bool:
        """Replace the whole library with *data*. Returns False if invalid."""
        if not isinstance(data, dict) or not isinstance(data.get("folders"), list):
            return False
        for folder in data["folders"]:
            if not isinstance(folder, dict) or not isinstance(folder.get("name"), str):
                return False
            if not isinstance(folder.get("snippets"), list):
                return False
            for sn in folder["snippets"]:
                if not isinstance(sn, dict):
                    return False
                if not isinstance(sn.get("abbreviation"), str) or not isinstance(sn.get("content"), str):
                    return False
        self._data = data
        self._abbrev_cache = None
        self.save()
        return True

    # ── Queries ────────────────────────────────────────────────────────────────

    def folder_names(self) -> List[str]:
        return [f["name"] for f in self._data["folders"]]

    def snippets_for_folder(self, folder_name: str) -> List[Dict]:
        for f in self._data["folders"]:
            if f["name"] == folder_name:
                return list(f["snippets"])
        return []

    def abbreviations_map(self) -> Dict[str, str]:
        """Return {abbreviation: content} for ALL snippets (cached)."""
        if self._abbrev_cache is not None:
            return self._abbrev_cache
            
        result = {}
        for folder in self._data["folders"]:
            for sn in folder["snippets"]:
                result[sn["abbreviation"]] = sn["content"]
        
        self._abbrev_cache = result
        return result

    def get_snippet(self, folder_name: str, abbreviation: str) -> Optional[Dict]:
        for f in self._data["folders"]:
            if f["name"] == folder_name:
                for sn in f["snippets"]:
                    if sn["abbreviation"] == abbreviation:
                        return dict(sn)
        return None

    # ── Folder mutations ───────────────────────────────────────────────────────

    def add_folder(self, name: str) -> bool:
        if any(f["name"] == name for f in self._data["folders"]):
            return False
        self._data["folders"].append({"name": name, "snippets": []})
        self._abbrev_cache = None
        self.save()
        return True

    def rename_folder(self, old_name: str, new_name: str) -> bool:
        for f in self._data["folders"]:
            if f["name"] == old_name:
                f["name"] = new_name
                self.save()
                return True
        return False

    def delete_folder(self, name: str) -> bool:
        before = len(self._data["folders"])
        self._data["folders"] = [f for f in self._data["folders"] if f["name"] != name]
        if len(self._data["folders"]) < before:
            self.save()
            return True
        return False

    # ── Snippet mutations ──────────────────────────────────────────────────────

    def add_snippet(self, folder_name: str, abbreviation: str, content: str = "") -> bool:
        for f in self._data["folders"]:
            if f["name"] == folder_name:
                if any(s["abbreviation"] == abbreviation for s in f["snippets"]):
                    return False
                f["snippets"].append({"abbreviation": abbreviation, "content": content})
                self._abbrev_cache = None
                self.save()
                return True
        return False

    def update_snippet(self, folder_name: str, abbreviation: str,
                       new_abbreviation: str, new_content: str) -> bool:
        for f in self._data["folders"]:
            if f["name"] == folder_name:
                # Check for duplicate if abbreviation is changing
                if new_abbreviation != abbreviation:
                    if any(s["abbreviation"] == new_abbreviation for s in f["snippets"]):
                        return False
                for sn in f["snippets"]:
                    if sn["abbreviation"] == abbreviation:
                        sn["abbreviation"] = new_abbreviation
                        sn["content"]      = new_content
                        self._abbrev_cache = None
                        self.save()
                        return True
        return False

    def delete_snippet(self, folder_name: str, abbreviation: str) -> bool:
        for f in self._data["folders"]:
            if f["name"] == folder_name:
                before = len(f["snippets"])
                f["snippets"] = [s for s in f["snippets"] if s["abbreviation"] != abbreviation]
                if len(f["snippets"]) < before:
                    self._abbrev_cache = None
                    self.save()
                    return True
        return False

    def move_snippet(self, abbreviation: str, from_folder: str, to_folder: str) -> bool:
        snippet = None
        for f in self._data["folders"]:
            if f["name"] == from_folder:
                for i, s in enumerate(f["snippets"]):
                    if s["abbreviation"] == abbreviation:
                        snippet = f["snippets"].pop(i)
                        break
                break
        if snippet is None:
            return False
        for f in self._data["folders"]:
            if f["name"] == to_folder:
                f["snippets"].append(snippet)
                self._abbrev_cache = None
                self.save()
                return True
        return False

    # ── Order persistence ──────────────────────────────────────────────────────

    def reorder_folders(self, names: list) -> bool:
        """Reorder folders to match *names* list, then persist."""
        lookup = {f["name"]: f for f in self._data["folders"]}
        ordered = [lookup[n] for n in names if n in lookup]
        # keep any folders not in the list at the end (safety)
        known = set(names)
        extras = [f for f in self._data["folders"] if f["name"] not in known]
        self._data["folders"] = ordered + extras
        self.save()
        return True

    def reorder_snippets(self, folder_name: str, names: list) -> bool:
        """Reorder snippets inside *folder_name* to match *names*, then persist."""
        for f in self._data["folders"]:
            if f["name"] == folder_name:
                lookup = {s["abbreviation"]: s for s in f["snippets"]}
                ordered = [lookup[n] for n in names if n in lookup]
                known = set(names)
                extras = [s for s in f["snippets"] if s["abbreviation"] not in known]
                f["snippets"] = ordered + extras
                self._abbrev_cache = None
                self.save()
                return True
        return False
