"""Economy breakdown for vsbatch batches: python -I econ.py <batch dir glob prefix>... [--per-map]

Per side (tested = the batch's first bot, opponent): wins; gold earned per round (all champions);
gems collected, submitted, lost on death, and carried at the end; deaths; kills; camp groups
cleared; gold spent and gold left unspent at the end; and the split of earned gold at rounds
300 / 1000 / 2000. Gem accounting: collected = submitted + lost + carried (+ stolen counts as
collected for the killer)."""
import json, os, sys, collections, glob

def value_at(runs, r):
    t = 0
    for run in runs:
        t += run["duration"]
        if t > r:
            return float(run["value"])
    return float(runs[-1]["value"]) if runs else 0.0

def final(runs):
    return float(runs[-1]["value"]) if runs else 0.0

per_map = "--per-map" in sys.argv
dirs = []
for pat in [a for a in sys.argv[1:] if not a.startswith("--")]:
    dirs += sorted(p for p in glob.glob(pat) if os.path.isdir(p))
tot = {s: collections.Counter() for s in ("us", "them")}
maps = collections.defaultdict(lambda: {s: collections.Counter() for s in ("us", "them")})
games = 0; rounds = 0
for d_ in dirs:
    for f in sorted(os.listdir(d_)):
        if not f.endswith(".json"):
            continue
        d = json.load(open(os.path.join(d_, f)))
        us = "team" + f[-6]
        games += 1
        r = d["match"]["roundsCompleted"]; rounds += r
        m = f[:-7]
        for side, team in (("us", us), ("them", "teamB" if us == "teamA" else "teamA")):
            c = tot[side]; cm = maps[m][side]
            def add(k, v):
                c[k] += v; cm[k] += v
            add("games", 1); add("rounds", r)
            add("wins", d["match"]["outcome"] == team)
            add("submitted", int(next(x["submittedGems"] for x in d["teams"] if x["team"] == team)))
            for tl in d["championTimelines"]:
                if tl["owner"]["team"] != team:
                    continue
                k = tl["metric"]; runs = tl["runs"]
                if k == "earnedGold":
                    add("gold", final(runs)); add("gold300", value_at(runs, 300)); add("gold1000", value_at(runs, 1000))
                elif k == "collectedGems":
                    add("collected", final(runs)); add("collected300", value_at(runs, 300)); add("collected1000", value_at(runs, 1000))
                elif k == "gems":
                    add("carried_end", final(runs))
                elif k == "gold":
                    add("gold_unspent_end", final(runs))
                elif k == "spentGold":
                    add("spent", final(runs))
                elif k in ("deaths", "kills", "monsterDamageDone"):
                    add(k, final(runs))
            for e in d["events"]:
                o = e["owner"]
                if o.get("team") != team:
                    continue
                if e["kind"] == "campClear":
                    add("clears", len(e["entries"]))
                elif e["kind"] == "gemLoss":
                    add("gems_lost", sum(float(x.get("value", 0)) for x in e["entries"]))
                elif e["kind"] == "gemSubmission":
                    add("tributes", len(e["entries"]))
                elif e["kind"] == "fuelExhausted":
                    add("fuel", len(e["entries"]))

def show(label, c):
    g = max(c["games"], 1); rr = max(c["rounds"], 1)
    print(f"{label:10} wins {c['wins']:4}/{g}  gold/round {c['gold']/rr:5.2f}  gold@300 {c['gold300']/g:6.0f}  gold@1000 {c['gold1000']/g:6.0f}  "
          f"gems: collected {c['collected']/g:5.0f} (@300 {c['collected300']/g:4.0f}, @1000 {c['collected1000']/g:4.0f}) submitted {c['submitted']/g:5.0f} lost {c['gems_lost']/g:4.0f} carried-at-end {c['carried_end']/g:4.0f}  "
          f"tributes {c['tributes']/g:4.1f}  clears {c['clears']/g:5.1f}  deaths {c['deaths']/g:4.1f} kills {c['kills']/g:4.1f}  unspent gold at end {c['gold_unspent_end']/g:5.0f}  fuel-exh {c['fuel']*1000/rr:4.0f}/1000r")

print(f"{games} games")
show("tested", tot["us"]); show("opponent", tot["them"])
if per_map:
    for m in sorted(maps):
        a, b = maps[m]["us"], maps[m]["them"]
        g = max(a["games"], 1)
        print(f"  {m:22} wins {a['wins']}/{g}  gold/r {a['gold']/max(a['rounds'],1):5.2f} vs {b['gold']/max(b['rounds'],1):5.2f}  collected {a['collected']/g:5.0f} vs {b['collected']/g:5.0f}  submitted {a['submitted']/g:5.0f} vs {b['submitted']/g:5.0f}  clears {a['clears']/g:4.1f} vs {b['clears']/g:4.1f}  deaths {a['deaths']/g:4.1f} vs {b['deaths']/g:4.1f}")
