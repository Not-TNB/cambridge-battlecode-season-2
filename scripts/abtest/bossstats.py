"""Boss-battle detail per game: python -I bossstats.py <batch dir>... (tested side by file name, as econ.py)"""
import json, glob, os, sys, collections
for d0 in sys.argv[1:]:
    tot = collections.Counter(); n = 0
    for f in sorted(glob.glob(os.path.join(d0, "*boss*.json"))):
        us = "team" + f[-6]; them = "teamB" if us == "teamA" else "teamA"
        d = json.load(open(f, encoding="utf-8")); n += 1
        sub = {t["team"]: int(t["submittedGems"]) for t in d["teams"]}
        first_kill = {}; trib = collections.Counter(); first_trib = {}; campdeaths = collections.Counter(); lost = collections.Counter(); caps = collections.Counter()
        for e in d["events"]:
            s = e["owner"]["team"]
            for x in e["entries"]:
                r = x["timestamp"]["round"]
                if e["kind"] == "campClear" and (x["x"], x["y"]) in ((23, 20), (24, 20)):
                    first_kill.setdefault(s, r); first_kill[s] = min(first_kill[s], r)
                if e["kind"] == "gemCollection" and (x["x"], x["y"]) in ((23, 20), (24, 20)) and x["context"] == "campGroup":
                    caps[s] += 1
                if e["kind"] == "gemSubmission":
                    first_trib[s] = min(first_trib.get(s, 9999), r)
                if e["kind"] == "death" and x["context"] == "camp": campdeaths[s] += 1
                if e["kind"] == "gemLoss": lost[s] += int(x["value"]) if x["value"].isdigit() else 0
        carried = {s: sum(int(c["metrics"]["gems"]) for c in d["champions"] if c["team"] == s) for s in (us, them)}
        gold = {s: sum(int(c["metrics"]["earnedGold"]) for c in d["champions"] if c["team"] == s) for s in (us, them)}
        win = d["match"]["outcome"] == us
        print(f"  {os.path.basename(f):28} {'WIN ' if win else 'loss'} tributed {sub[us]:5}/{sub[them]:<5} first tribute r{first_trib.get(us, '-')}/{first_trib.get(them, '-')}  monolith clear r{first_kill.get(us, '-')}/{first_kill.get(them, '-')} captures {caps[us]}/{caps[them]}  camp deaths {campdeaths[us]}/{campdeaths[them]}  gems lost {lost[us]}/{lost[them]}  carried {carried[us]}/{carried[them]}  gold {gold[us]}/{gold[them]}")
        tot["win"] += win; tot["sub"] += sub[us]; tot["osub"] += sub[them]; tot["cd"] += campdeaths[us]; tot["lost"] += lost[us]; tot["gold"] += gold[us]; tot["ogold"] += gold[them]
    if n: print(f"{d0}: {tot['win']}/{n} wins, tributed {tot['sub']/n:.0f} vs {tot['osub']/n:.0f}, camp deaths {tot['cd']/n:.1f}, gems lost {tot['lost']/n:.0f}, gold {tot['gold']/n:.0f} vs {tot['ogold']/n:.0f}")
