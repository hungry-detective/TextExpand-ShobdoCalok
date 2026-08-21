"""
expansion_engine.py — system-wide keyboard hook for text expansion.

Uses pynput to monitor all keypresses. When the user types a registered
abbreviation (bounded by word boundaries), the abbreviation is deleted and
the expanded content is inserted.

Token resolution:
  {date}            → current date    (e.g. 1 August 2026)
  {time}            → current time    (e.g. 14:32)
  {datetime}        → date + time
  {clipboard}       → current clipboard contents
  {cursor}          → place the caret here after insertion
  {field:Label}     → prompts the user via a Qt dialog
  {date:fmt}        → date formatted with a strftime pattern (e.g. {date:%d/%m/%Y})
  {time:fmt}        → time formatted with a strftime pattern
  {datetime:fmt}    → date + time formatted with a strftime pattern
"""

import re
import time
import threading
import datetime
import pyperclip

from pynput import keyboard
from pynput.keyboard import Key, Controller

from PySide6.QtCore import QObject, Signal


_TRIGGER_KEYS = {Key.space, Key.enter, Key.tab}
_kb = Controller()

# Text shorter than this is typed directly (non-destructive); longer text is
# pasted via the clipboard (with the previous clipboard restored afterwards).
_TYPE_THRESHOLD = 120


def _is_word_char(ch) -> bool:
    """True when *ch* could be part of a word (letters, digits, underscore)."""
    return ch is not None and (ch.isalnum() or ch == "_")


def _press_backspace(n: int):
    for _ in range(n):
        _kb.press(Key.backspace)
        _kb.release(Key.backspace)
        time.sleep(0.004)


class ExpansionEngine(QObject):
    """
    Background thread that monitors keyboard input globally and expands
    abbreviations when typed into *any* application.
    """

    # Emitted when {field:Label} tokens are found; main thread shows one dialog for all
    fieldFillRequested = Signal(list, str, str)    # labels, unique_key, content_preview

    def __init__(self, store, parent=None):
        super().__init__(parent)
        self._store = store
        self._enabled = True
        self._buffer = ""
        self._lock = threading.Lock()
        self._listener: keyboard.Listener | None = None
        self._mods: set = set()
        # True while we are programmatically injecting keystrokes (backspace,
        # typing, paste). The global hook must ignore these to avoid feedback.
        self._injecting = False

        # For batch field-fill synchronisation: {key: {label: value}}
        self._pending_results: dict[str, dict[str, str] | None] = {}
        self._field_events: dict[str, threading.Event] = {}

    # ── Public API ─────────────────────────────────────────────────────────────

    def start(self):
        self._listener = keyboard.Listener(
            on_press=self._on_press,
            on_release=self._on_release
        )
        self._listener.daemon = True
        self._listener.start()

    def stop(self):
        if self._listener:
            self._listener.stop()
            self._listener = None

    def restart(self):
        """Stop the current listener and start a new one.

        Called after Windows sleep/resume because the OS removes the
        low-level keyboard hook (WH_KEYBOARD_LL) during the transition.
        """
        print("[ExpansionEngine] Restarting keyboard listener")
        self.stop()
        with self._lock:
            self._buffer = ""
        self.start()

    def is_alive(self) -> bool:
        """Return True if the keyboard listener thread is still running."""
        return self._listener is not None and self._listener.is_alive()

    def set_enabled(self, enabled: bool):
        self._enabled = enabled
        if enabled:
            with self._lock:
                self._buffer = ""

    @property
    def is_enabled(self) -> bool:
        return self._enabled

    def refresh_store(self):
        """Call after snippets are changed so the engine picks up new data."""
        with self._lock:
            self._buffer = ""

    def resolve_fields(self, key: str, results: dict[str, str]):
        """Called from the main thread when the user fills in the multi-field dialog."""
        self._pending_results[key] = results
        if key in self._field_events:
            self._field_events[key].set()

    def preview(self, content: str) -> str:
        """Resolve tokens for a live preview; {field:Label} shows as [Label]."""
        try:
            return self._resolve_tokens(content, preview=True)
        except Exception as e:
            print(f"[ExpansionEngine] Preview error: {e}")
            return content

    # ── Keyboard listener ──────────────────────────────────────────────────────

    def _on_press(self, key):
        self._handle_modifier(key, pressed=True)

        # Ignore keystrokes we inject ourselves (backspace / typing / paste)
        if self._injecting:
            return

        if not self._enabled:
            return

        with self._lock:
            try:
                ch = key.char
            except AttributeError:
                ch = None

            if ch is not None:
                self._buffer += ch
                # Trim buffer — no abbreviation should be longer than 40 chars
                self._buffer = self._buffer[-40:]

                # Try expand immediately if buffer might contain an abbreviation
                if len(self._buffer) >= 2:
                    self._try_expand_immediate(self._buffer)
                return

            # Special key pressed
            if key == Key.backspace:
                self._buffer = self._buffer[:-1]
            elif key in _TRIGGER_KEYS or key == Key.esc:
                self._buffer = ""
            else:
                self._buffer = ""

    def _on_release(self, key):
        self._handle_modifier(key, pressed=False)

    def _handle_modifier(self, key, pressed: bool):
        if key in (Key.ctrl, Key.ctrl_l, Key.ctrl_r,
                   Key.shift, Key.shift_l, Key.shift_r,
                   Key.alt, Key.alt_l, Key.alt_r,
                   Key.cmd, Key.cmd_l, Key.cmd_r):
            if pressed:
                self._mods.add(key)
            else:
                self._mods.discard(key)

    def _try_expand_immediate(self, buf: str):
        """Finds a boundary-bounded abbreviation match and expands it."""
        abbrevs = self._store.abbreviations_map()

        found_abbrev = None
        for length in range(min(len(buf), 30), 1, -1):
            target = buf[-length:]
            if target in abbrevs:
                # Only expand when the character before the abbreviation is a
                # word boundary — prevents false triggers inside regular words.
                prev = buf[-length - 1] if length < len(buf) else None
                if prev is not None and _is_word_char(prev):
                    continue
                found_abbrev = target
                break

        if found_abbrev is None:
            return

        # Found a match! Clear buffer immediately
        self._buffer = ""
        matched_content = abbrevs[found_abbrev]

        # Spawn expansion thread
        threading.Thread(
            target=self._perform_expansion,
            args=(found_abbrev, matched_content),
            daemon=True
        ).start()

    def _perform_expansion(self, abbrev: str, content: str):
        """The actual work of deleting and inserting. Runs in its own thread."""
        # Let the last typed character be fully registered before deleting it
        time.sleep(0.02)

        self._injecting = True
        try:
            # Delete abbreviation
            _press_backspace(len(abbrev))

            # Resolve tokens
            try:
                expanded = self._resolve_tokens(content)
            except Exception as e:
                print(f"[ExpansionEngine] Token error: {e}")
                expanded = content

            self._insert_text(expanded)
        finally:
            self._injecting = False

    def _insert_text(self, text: str):
        """Insert *text*, honouring any {cursor} marker. Non-destructive."""
        cursor_index = None
        if "{cursor}" in text:
            cursor_index = text.index("{cursor}")
            text = text.replace("{cursor}", "")

        if not text:
            return

        # Clipboard paste is atomic and never drops characters. pynput's
        # Controller typing (even char-by-char) can lose keystrokes in
        # fast apps, so we only fall back to typing if paste fails.
        try:
            self._paste_clipboard(text)
        except Exception:
            try:
                self._type_reliable(text)
            except Exception:
                pass

        if cursor_index is not None:
            n_left = max(0, len(text) - cursor_index)
            for _ in range(n_left):
                _kb.press(Key.left)
                _kb.release(Key.left)
                time.sleep(0.004)

    def _type_reliable(self, text: str):
        """Type *text* character-by-character with small delays.

        pynput's Controller.type() sends every key immediately with no delay,
        which makes fast-target apps drop characters. Typing one char at a time
        with a small gap keeps the insert intact.
        """
        for ch in text:
            _kb.press(ch)
            _kb.release(ch)
            time.sleep(0.008)

    def _paste_clipboard(self, text: str):
        """Paste via the clipboard, restoring the previous contents afterwards."""
        try:
            old_clip = pyperclip.paste()
        except Exception:
            old_clip = ""

        pyperclip.copy(text)
        time.sleep(0.06)

        # Press Ctrl+V with Ctrl held down for a stable paste
        _kb.press(Key.ctrl)
        time.sleep(0.01)
        _kb.press('v')
        time.sleep(0.01)
        _kb.release('v')
        time.sleep(0.01)
        _kb.release(Key.ctrl)

        time.sleep(0.06)

        try:
            if old_clip:
                pyperclip.copy(old_clip)
        except Exception:
            pass

    # ── Token resolution ───────────────────────────────────────────────────────

    def _resolve_tokens(self, content: str, preview: bool = False) -> str:
        now = datetime.datetime.now()

        def _fmt(m, default):
            fmt = m.group(1).strip() or default
            try:
                return now.strftime(fmt)
            except (ValueError, TypeError):
                return now.strftime(default)

        # Parameterized tokens: {date:fmt} {time:fmt} {datetime:fmt}
        content = re.compile(r"\{datetime:([^}]*)\}").sub(
            lambda m: _fmt(m, "%Y-%m-%d %H:%M"), content)
        content = re.compile(r"\{date:([^}]*)\}").sub(
            lambda m: _fmt(m, "%d %B %Y"), content)
        content = re.compile(r"\{time:([^}]*)\}").sub(
            lambda m: _fmt(m, "%H:%M"), content)

        # Plain tokens
        content = content.replace("{date}", now.strftime("%d %B %Y"))
        content = content.replace("{time}", now.strftime("%I:%M %p"))
        content = content.replace("{datetime}", now.strftime("%Y-%m-%d %H:%M"))

        # {clipboard}
        try:
            content = content.replace("{clipboard}", pyperclip.paste())
        except Exception:
            content = content.replace("{clipboard}", "")

        if preview:
            # Hide the caret marker in previews
            content = content.replace("{cursor}", "")

        # {field:Label} — batch all fields (independent replacements)
        field_pattern = re.compile(r"\{field:([^}]+)\}")
        matches = list(field_pattern.finditer(content))

        if matches:
            if preview:
                content = field_pattern.sub(lambda m: f"[{m.group(1)}]", content)
            else:
                labels = [m.group(1) for m in matches]
                key = f"batch_{int(time.time())}_{id(content)}"
                event = threading.Event()
                self._field_events[key] = event
                self._pending_results[key] = None

                # Emit signal for main thread to show dialog with ALL labels (in order)
                self.fieldFillRequested.emit(labels, key, content)

                # Wait for user input (1 hour timeout to ensure it doesn't auto-paste)
                event.wait(timeout=3600)

                results = self._pending_results.pop(key, None)  # list of strings from dialog
                self._field_events.pop(key, None)

                if results and isinstance(results, list):
                    # Replace matches in REVERSE order to keep offsets valid
                    for i in range(len(matches) - 1, -1, -1):
                        m = matches[i]
                        val = results[i] if i < len(results) else ""
                        content = content[:m.start()] + val + content[m.end():]
                else:
                    # User cancelled — drop the field tokens entirely
                    for m in reversed(matches):
                        content = content[:m.start()] + content[m.end():]

        return content
