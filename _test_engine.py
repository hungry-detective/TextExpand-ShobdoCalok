import os, sys, tempfile, datetime
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "textqml"))

from viewmodels.snippet_engine import SnippetStore
from viewmodels.expansion_engine import ExpansionEngine

# Safety: never touch the real keyboard/clipboard in tests.
import threading as _th

def _fake_thread(*args, **kwargs):
    return None

_th.Thread = None  # break Thread usage: see below

d = tempfile.mkdtemp()
st = SnippetStore(os.path.join(d, "s.json"))
st.import_data({"folders": [{"name": "G", "snippets": [
    {"abbreviation": ".t", "content": "Hi {date:%d/%m/%Y} at {time:%H:%M} - {field:Name} {cursor}"},
    {"abbreviation": ".sig", "content": "signature"},
    {"abbreviation": ".long", "content": "A" * 200},
]}]})
e = ExpansionEngine(st)

# 1) preview: parameterized formats, [Label] placeholder, cursor stripped
out = e.preview(st.snippets_for_folder("G")[0]["content"])
now = datetime.datetime.now()
assert now.strftime("%d/%m/%Y") in out, out
assert now.strftime("%H:%M") in out, out
assert "[Name]" in out, out
assert "{cursor}" not in out, out
print("1) preview OK:", repr(out))

# 2) _is_word_char
from viewmodels.expansion_engine import _is_word_char
assert _is_word_char("a") and _is_word_char("_") and not _is_word_char(" ") and not _is_word_char(None)
print("2) word-char OK")

# 3) boundary selection: expand only when preceding char is a word boundary.
#    Patch the expansion side-effect so nothing touches the keyboard.
calls = []
import viewmodels.expansion_engine as eng_mod
_orig_thread = eng_mod.threading.Thread
def _sync_target(*args, **kwargs):
    return None
class _SyncThread:
    def __init__(self, target=None, args=(), kwargs=None, daemon=None):
        target(*args, **(kwargs or {}))
    def start(self):
        pass
eng_mod.threading.Thread = _SyncThread
e._perform_expansion = lambda abbrev, content: calls.append(abbrev)

cases = [
    ("photo.sig", []),        # inside a word -> no trigger
    ("hello .sig", [".sig"]), # after space -> trigger
    (".sig", [".sig"]),       # at start -> trigger
    ("x.sig", []),            # joined to a letter -> no trigger
]
for buf, expected in cases:
    calls.clear()
    e._buffer = buf
    e._try_expand_immediate(buf)
    assert calls == expected, f"{buf!r}: got {calls}, want {expected}"
    print(f"3) boundary {buf!r} -> {calls}")

# 4) plain date token format (the one the user requested)
plain = e.preview(".sig content {date} suffix")
assert now.strftime("%d %B %Y") in plain, plain
print("4) plain date OK:", repr(plain))

# 5) insert method choice logic (no real insert)
import viewmodels.expansion_engine as m
assert m._TYPE_THRESHOLD > 0
print("5) threshold OK:", m._TYPE_THRESHOLD)

print("ALL TESTS PASSED")
