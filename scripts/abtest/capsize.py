"""Captures by gem size per side in a local batch: python -I capsize.py <batch dir>... (tested side as econ.py)."""
import json, glob, os, sys, collections
B = [(0, 10), (10, 20), (20, 40), (40, 80), (80, 9999)]
for d0 in sys.argv[1:]:
    a = collections.Counter(); n = 0
    for f in glob.glob(os.path.join(d0, "*.json")):
        us = "teamA" if f.endswith("-A.json") else "teamB"
        d = json.load(open(f, encoding="utf-8")); n += 1
        for e in d["events"]:
            if e["kind"] != "gemCollection": continue
            for x in e["entries"]:
                if x["context"] != "campGroup": continue
                v = int(x["value"]); b = next(i for i, (lo, hi) in enumerate(B) if lo <= v < hi)
                who = "us" if e["owner"]["team"] == us else "them"
                a[(who, b)] += 1; a[(who, b, "g")] += v
                if x["timestamp"]["round"] <= 400: a[(who, b, "e")] += 1
    print(d0, n, "games: captures/game (gems) [by r400]")
    for i, (lo, hi) in enumerate(B):
        print(f"   {lo:3}-{hi:<4} tested {a[('us',i)]/n:5.1f} ({a[('us',i,'g')]/n:5.0f}) [{a[('us',i,'e')]/n:4.1f}]   opp {a[('them',i)]/n:5.1f} ({a[('them',i,'g')]/n:5.0f}) [{a[('them',i,'e')]/n:4.1f}]")
