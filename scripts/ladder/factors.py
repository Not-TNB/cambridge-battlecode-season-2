"""What decides games among top teams: python -I scripts/ladder/factors.py <games.jsonl> [--kind ranked|unranked] [--top N]"""
import json, sys, collections
import numpy as np
R = (50, 100, 150, 200, 300, 400, 500, 750, 1000, 1500)
CLASS = {"Ryth": "tank", "Vesper Vale": "AD", "Tavi": "AD", "Vendetta": "assassin", "Cassi": "assassin",
         "Stellar": "AP", "Prism": "AP", "Willow": "healer", "Marcella": "bruiser", "Ren": "bruiser",
         "Varg": "bruiser", "Heart of Ruin": "bruiser"}
args = sys.argv[1:]
kind = args[args.index("--kind") + 1] if "--kind" in args else None
TOP = int(args[args.index("--top") + 1]) if "--top" in args else 9999
games = []
dropped = collections.Counter()
for l in open(args[0], encoding="utf-8"):
    g = json.loads(l)
    if kind and g["kind"] != kind:
        continue
    if max(g["rank"].values()) > TOP:
        continue
    if g["win"] not in ("teamA", "teamB"):
        dropped["no winner"] += 1
        continue
    if any(g["teams"][s]["stats"].get("earnedGold", [0] * 10)[4] < 50 for s in ("teamA", "teamB")):
        dropped["broken bot (<50 gold by r300)"] += 1
        continue
    games.append(g)
other = {"teamA": "teamB", "teamB": "teamA"}


def st(g, s, k, i):
    return g["teams"][s]["stats"].get(k, [0] * 10)[i]


def pct(v):
    return f"{100 * v[0] / v[1]:3.0f}% ({v[1]:4})" if v[1] else "    -      "


print(f"{len(games)} games (kind {kind or 'all'}, both top {TOP}); dropped {dict(dropped)}")
better = [0, 0]
for g in games:
    a, b = g["rank"]["teamA"], g["rank"]["teamB"]
    if a != b:
        better[0] += g["win"] == ("teamA" if a < b else "teamB")
        better[1] += 1
print("Baseline: the better-ranked team wins", pct(better))

print("\nTeam ahead at round r wins (fewer is ahead for deaths and damage taken; games still running, not tied):")
STATS = ("earnedGold", "collectedGems", "submittedGems", "campClear", "kill", "death", "itemCombination", "buyItem",
         "championDamageDone", "championDamageTaken", "monsterDamageTaken", "healingDone", "wardPlacement")
FEWER = {"death", "championDamageTaken", "monsterDamageTaken"}
print(f"{'':20}" + "".join(f"{'r' + str(r):>12}" for r in R))
for k in STATS:
    row = []
    for i, r in enumerate(R):
        v = [0, 0]
        for g in games:
            if r >= g["rounds"]:
                continue
            a, b = st(g, "teamA", k, i), st(g, "teamB", k, i)
            if k in FEWER:
                a, b = b, a
            if a == b:
                continue
            v[0] += g["win"] == ("teamA" if a > b else "teamB")
            v[1] += 1
        row.append(pct(v))
    print(f"{k:20}" + "".join(f"{x:>12}" for x in row))

print("\nBy lead size (lead / mean of the two teams):")
for k in ("earnedGold", "collectedGems", "kill", "campClear"):
    for r in (150, 300, 500):
        i = R.index(r)
        bk = collections.defaultdict(lambda: [0, 0])
        for g in games:
            if r >= g["rounds"]:
                continue
            a, b = st(g, "teamA", k, i), st(g, "teamB", k, i)
            if a == b:
                continue
            rel = abs(a - b) / max(1.0, (a + b) / 2)
            name = "<10%" if rel < .1 else "10-25%" if rel < .25 else "25-50%" if rel < .5 else ">50%"
            bk[name][0] += g["win"] == ("teamA" if a > b else "teamB")
            bk[name][1] += 1
        print(f"  {k:14} r{r:<4}" + "".join(f"  {n}: {pct(bk[n])}" for n in ("<10%", "10-25%", "25-50%", ">50%")))

print("\nGold leader vs gems-collected leader when they differ (gold leader wins):")
for r in (150, 300, 500, 750):
    i = R.index(r)
    v = [0, 0]
    for g in games:
        if r >= g["rounds"]:
            continue
        dg = st(g, "teamA", "earnedGold", i) - st(g, "teamB", "earnedGold", i)
        dc = st(g, "teamA", "collectedGems", i) - st(g, "teamB", "collectedGems", i)
        if dg * dc < 0:
            v[0] += g["win"] == ("teamA" if dg > 0 else "teamB")
            v[1] += 1
    print(f"  r{r}: {pct(v)}")

print("\nFirst to ... wins:")
for k in ("campClear", "kill", "itemCombination", "gemSubmission", "wardPlacement"):
    v = [0, 0]
    for g in games:
        fa, fb = g["teams"]["teamA"]["first"].get(k), g["teams"]["teamB"]["first"].get(k)
        if not fa and not fb:
            continue
        first = "teamA" if fb is None or (fa and fa[0] < fb[0]) else "teamB"
        v[0] += g["win"] == first
        v[1] += 1
    print(f"  {k:16} {pct(v)}")

print("\nFewer of ... over the game wins:")
for k in ("fuelExhausted", "botFault", "campDeath", "gemLoss", "death"):
    v = [0, 0]
    for g in games:
        a, b = g["teams"]["teamA"]["count"].get(k, 0), g["teams"]["teamB"]["count"].get(k, 0)
        if a == b:
            continue
        v[0] += g["win"] == ("teamA" if a < b else "teamB")
        v[1] += 1
    print(f"  {k:16} {pct(v)}")

print("\nMore captures of a size (by round 400 / whole game) wins:")
for b_ in ("0-10", "10-20", "20-40", "40+"):
    out = []
    for key in (b_ + "@400", b_):
        v = [0, 0]
        for g in games:
            a, c = g["teams"]["teamA"]["caps"].get(key, 0), g["teams"]["teamB"]["caps"].get(key, 0)
            if a == c:
                continue
            v[0] += g["win"] == ("teamA" if a > c else "teamB")
            v[1] += 1
        out.append(pct(v))
    print(f"  {b_:6} by r400 {out[0]}   game {out[1]}")

print("\nChampion win rate when only one team has it (and pick rate among these teams):")
ch = collections.defaultdict(lambda: [0, 0])
seen = collections.Counter()
teams = 0
for g in games:
    for s in ("teamA", "teamB"):
        teams += 1
        mine, theirs = set(g["teams"][s]["champs"]), set(g["teams"][other[s]]["champs"])
        for c in mine:
            seen[c] += 1
        for c in mine - theirs:
            ch[c][0] += g["win"] == s
            ch[c][1] += 1
for c, v in sorted(ch.items(), key=lambda kv: -kv[1][0] / max(1, kv[1][1])):
    print(f"  {c:14} {pct(v)}  picked by {100 * seen[c] / teams:3.0f}% of teams")

print("\nClass counts (win rate when the two teams differ in that class):")
cl = collections.defaultdict(lambda: [0, 0])
va = collections.defaultdict(lambda: [0, 0])
comp = collections.defaultdict(lambda: [0, 0])
for g in games:
    for s in ("teamA", "teamB"):
        k1 = collections.Counter(CLASS.get(c, "?") for c in g["teams"][s]["champs"])
        k2 = collections.Counter(CLASS.get(c, "?") for c in g["teams"][other[s]]["champs"])
        w = g["win"] == s
        for k in set(k1) | set(k2):
            if k1[k] != k2[k]:
                cl[(k, k1[k])][0] += w
                cl[(k, k1[k])][1] += 1
        if len(k1) != len(k2):
            key = (len(g["teams"][s]["champs"]), len(k1))
            va[key][0] += w
            va[key][1] += 1
        key = " ".join(f"{k}{n}" for k, n in sorted(k1.items()))
        comp[key][0] += w
        comp[key][1] += 1
for k in sorted({k for k, _ in cl}):
    print(f"  {k:9}" + "".join(f"  {n}: {pct(cl[(k, n)])}" for n in range(4) if cl[(k, n)][1]))
print("\nDistinct classes (team size, classes), when the two teams differ:")
for k in sorted(va):
    print(f"  size {k[0]} classes {k[1]}: {pct(va[k])}")
print("\nMost common compositions (all games, mirrors included):")
for k, v in sorted(comp.items(), key=lambda kv: -kv[1][1])[:14]:
    print(f"  {k:42} {pct(v)}")

print("\nLogistic regression on team A minus team B (standardised; bigger |coef| = more decisive with the others held fixed):")
feats = ("earnedGold", "collectedGems", "kill", "death", "campClear", "itemCombination", "championDamageDone")
for r in (150, 300, 500):
    i = R.index(r)
    X, y = [], []
    for g in games:
        if r >= g["rounds"]:
            continue
        X.append([st(g, "teamA", k, i) - st(g, "teamB", k, i) for k in feats])
        y.append(g["win"] == "teamA")
    X = np.array(X, float)
    y = np.array(y, float)
    X = (X - X.mean(0)) / (X.std(0) + 1e-9)
    w = np.zeros(X.shape[1])
    b = 0.0
    for _ in range(3000):
        p = 1 / (1 + np.exp(-(X @ w + b)))
        w -= 0.1 * (X.T @ (p - y) / len(y) + 0.001 * w)
        b -= 0.1 * (p - y).mean()
    acc = ((1 / (1 + np.exp(-(X @ w + b))) > .5) == y).mean()
    print(f"  r{r} ({len(y)} games, accuracy {100 * acc:.0f}%): "
          + ", ".join(f"{k} {c:+.2f}" for k, c in sorted(zip(feats, w), key=lambda kv: -abs(kv[1]))))
