"""Copy bot-v2's turn logic into the Tavi squad as the module `v2`, for followers playing on their own.

Usage (from the repository root): python scripts/sync-v2-into-squad.py

Copies bots/bot-v2/src (everything but main.rs and draft.rs) to bots/tavi-tank-squad/src/v2/,
rewriting `crate::<module>` paths to `crate::v2::<module>` and routing shared-memory access through
`crate::v2::shared_memory()`: the squad's commands use all 8 bytes, so the copy only gets them once
the squad has handed them over (squad::v2_owns_shared_memory). The squad's own files in v2/
(mod.rs) are kept. Re-run it after changing bot-v2 to bring the changes over.
"""
import os
import re
import shutil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "bots", "bot-v2", "src")
DST = os.path.join(ROOT, "bots", "tavi-tank-squad", "src", "v2")
SKIP = {"main.rs", "draft.rs"}
KEEP = {"mod.rs"}  # v2/mod.rs is the squad's, not bot-v2's
MODULES = "bot|champions|common|constants|opening|regions|random_number"

header = "// Copied from bots/bot-v2 by scripts/sync-v2-into-squad.py: edit bot-v2 and re-run it.\n"

# Clear out the previous copy (but not the squad's own mod.rs).
if os.path.isdir(DST):
    for name in os.listdir(DST):
        path = os.path.join(DST, name)
        if name in KEEP:
            continue
        if os.path.isdir(path):
            shutil.rmtree(path)
        else:
            os.remove(path)
else:
    os.makedirs(DST)

copied = 0
for folder, _, files in os.walk(SRC):
    rel = os.path.relpath(folder, SRC)
    for name in files:
        if not name.endswith(".rs") or (rel == "." and (name in SKIP or name in KEEP)):
            continue
        text = open(os.path.join(folder, name), encoding="utf-8", newline="").read()
        text = re.sub(rf"\bcrate::({MODULES})\b", r"crate::v2::\1", text)
        text = text.replace("cambc::shared_memory()", "crate::v2::shared_memory()")
        out_dir = os.path.join(DST, rel)
        os.makedirs(out_dir, exist_ok=True)
        open(os.path.join(out_dir, name), "w", encoding="utf-8", newline="").write(header + text)
        copied += 1
print(f"copied {copied} files into {os.path.relpath(DST, ROOT)}")
