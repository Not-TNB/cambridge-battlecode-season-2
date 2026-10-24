"""Compact records from the summaries zip: python -I scripts/ladder/extract_summaries.py <zip> <ladder dir> <out.jsonl> <cambc.exe>
Keeps ranked games with both teams in the top 150 and unranked with both in the top 100 (current ladder).
Each record: kind, match id, team ids/ranks, outcome, rounds, map hash/size, and per team: stats at
checkpoint rounds, champions in draft order, first events, faults, fuel-exhausted turns, captures by size."""
import zipfile, json, glob, os, sys, subprocess, collections, concurrent.futures as cf, threading
ZIP, LAD, OUT, CAMBC = sys.argv[1:5]
rank = {}
for f in glob.glob(os.path.join(LAD, "ladder_*.json")):
    for t in json.load(open(f, encoding="utf-8"))["teams"]: rank[t["teamId"]] = t["rank"]
R = (50, 100, 150, 200, 300, 400, 500, 750, 1000, 1500)
def value_at(runs, r):
    t = 0
    for run in runs:
        t += run["duration"]
        if t > r: return float(run["value"])
    return float(runs[-1]["value"]) if runs else 0.0
z = zipfile.ZipFile(ZIP); lock = threading.Lock()
jobs = []
for n in z.namelist():
    kind, mid, rest = n[:-8].split("_", 2); a, b = rest.split("_vs_")
    lim = 150 if kind == "ranked" else 100
    if max(rank.get(a, 9999), rank.get(b, 9999)) <= lim: jobs.append((n, kind, mid, a, b))
done = set()
if os.path.exists(OUT):
    for l in open(OUT, encoding="utf-8"): done.add(json.loads(l)["id"])
jobs = [j for j in jobs if j[2] not in done]
print(len(jobs), "to do", flush=True)
def one(job):
    n, kind, mid, a, b = job
    with lock: raw = z.read(n)
    tmp = os.path.join(os.path.dirname(OUT), "t_" + mid[:8] + ".camstat")
    open(tmp, "wb").write(raw)
    p = subprocess.run([CAMBC, "summary", tmp], capture_output=True); os.remove(tmp)
    if p.returncode: return None
    d = json.loads(p.stdout)
    rec = {"id": mid, "kind": kind, "ids": {"teamA": a, "teamB": b}, "rank": {"teamA": rank.get(a), "teamB": rank.get(b)},
           "win": d["match"]["outcome"], "rounds": d["match"]["roundsCompleted"], "map": d["match"]["mapHash"][:12],
           "size": [d["match"]["width"], d["match"]["height"]], "submitted": {t["team"]: int(t["submittedGems"]) for t in d["teams"]}}
    T = {s: {"stats": collections.defaultdict(lambda: [0.0] * len(R)), "first": {}, "count": collections.Counter(), "caps": collections.Counter()} for s in ("teamA", "teamB")}
    for t in d["championTimelines"]:
        if t["metric"] in ("earnedGold", "collectedGems", "submittedGems", "kills", "deaths", "championDamageDone", "monsterDamageTaken", "championDamageTaken", "healingDone", "shieldingDone"):
            v = T[t["owner"]["team"]]["stats"][t["metric"]]
            for i, r in enumerate(R): v[i] += value_at(t["runs"], r)
    for e in d["events"]:
        k = e["kind"]; s = e["owner"]["team"]
        if s not in T: continue
        for x in e["entries"]:
            r = x["timestamp"]["round"]; o = x["timestamp"]["ordinal"]
            T[s]["count"][k] += 1
            if k in ("campClear", "itemCombination", "kill", "death", "wardPlacement", "buyItem"):
                v = T[s]["stats"][k]
                for i, c in enumerate(R):
                    if r <= c: v[i] += 1
            if k == "death" and x.get("context") == "camp": T[s]["count"]["campDeath"] += 1
            if k == "gemCollection" and x.get("context") == "campGroup":
                g = int(x["value"]); b_ = "0-10" if g < 10 else "10-20" if g < 20 else "20-40" if g < 40 else "40+"
                T[s]["caps"][b_] += 1
                if r <= 400: T[s]["caps"][b_ + "@400"] += 1
            if k not in T[s]["first"] or o < T[s]["first"][k][0]: T[s]["first"][k] = (o, r)
    champs = {s: [c["name"] for c in sorted(d["roster"]["champions"], key=lambda c: c["draftPick"]) if c["team"] == s and c["name"] != "Issac"] for s in T}
    rec["teams"] = {s: {"stats": dict(T[s]["stats"]), "first": T[s]["first"], "count": dict(T[s]["count"]), "caps": dict(T[s]["caps"]), "champs": champs[s]} for s in T}
    return rec
n = 0
with open(OUT, "a", encoding="utf-8") as out, cf.ThreadPoolExecutor(8) as ex:
    for rec in ex.map(one, jobs):
        if rec: out.write(json.dumps(rec) + "\n")
        n += 1
        if n % 500 == 0: print(n, flush=True)
print("extract done", n, flush=True)
