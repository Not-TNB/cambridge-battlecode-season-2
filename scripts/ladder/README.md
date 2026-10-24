# Ladder analysis

What decides games between top ladder teams (write-up entry 34).

1. **Ladder standings** (needs `cambc login`): `cambc ladder --page N --limit 100 --json > ladder_N.json` for N = 1..8, all in one directory.
2. **Compact records** from a summaries zip (files named `kind_matchId_teamA_vs_teamB.camstat`):
   `python -I scripts/ladder/extract_summaries.py <zip> <dir with ladder_*.json> <out.jsonl> <path to cambc.exe>`
   - It keeps ranked games with both teams in the top 150 and unranked games with both in the top 100.
   - Each `.camstat` is converted with `cambc summary` (offline, about 0.1 s each, 8 threads).
   - Needs cambc 3.0.5 or later; 3.0.4 shows champions and items as "Unknown".
3. **Factors:** `python -I scripts/ladder/factors.py <out.jsonl> [--kind ranked|unranked] [--top N]`. For each checkpoint round it shows how often the team ahead on each stat wins, the lead sizes, first events, captures by size, champion and class win rates, and a regression.

Downloading summaries one by one from the platform hits rate limits (about 5 a minute), so use the organisers' zips. `cambc matches list --kind ranked` without `--team` returns low-ranked games.
