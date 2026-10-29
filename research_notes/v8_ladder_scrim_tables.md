# v8 ladder scrim analysis: full tables

Generated 2026-10-25 by `analyze.py` and `deep_report.py` in `~/Desktop/Code/battlecode-scrim-2026-10-25/scrim/`. Companion to `v8_ladder_scrim_analysis.md`.

# Part A: per-game features (analyze.py)

## A. Scrims: v8 vs top 100 (n=485 games, 97 opponents)

Overall: 104-381 (21%, 95% CI 18%–25%)

### Win rate by opponent rank (at queue time)

| Ranks | Games | Wins | Win rate [95% CI] | Gems us/them | Gold@300 us/them | K/D us |
|---|---|---|---|---|---|---|
| 1–10 | 50 | 3 | 6% [2%–16%] | 351/994 | 1,048/1,496 | 3.7/13 |
| 11–20 | 50 | 3 | 6% [2%–16%] | 350/1,017 | 1,042/1,497 | 4.2/14 |
| 21–30 | 40 | 0 | 0% [0%–9%] | 315/1,114 | 983/1,671 | 3.5/17 |
| 31–40 | 45 | 8 | 18% [9%–31%] | 472/931 | 1,014/1,379 | 6.4/15 |
| 41–50 | 50 | 10 | 20% [11%–33%] | 541/886 | 1,154/1,418 | 7.0/15 |
| 51–60 | 40 | 10 | 25% [14%–40%] | 565/903 | 1,136/1,298 | 6.3/13 |
| 61–70 | 50 | 15 | 30% [19%–44%] | 681/845 | 1,153/1,122 | 7.9/12 |
| 71–80 | 50 | 11 | 22% [13%–35%] | 548/910 | 1,003/1,215 | 6.7/13 |
| 81–90 | 60 | 27 | 45% [33%–58%] | 724/771 | 1,216/1,272 | 9.3/8.8 |
| 91–100 | 50 | 17 | 34% [22%–48%] | 632/816 | 1,101/1,176 | 9.6/13 |

**Logistic fit of win probability on opponent rating:**

- P(win)=70% at rating ≈1700 (≈ rank #100)
- P(win)=50% at rating ≈1742 (≈ rank #100)
- P(win)=30% at rating ≈1853 (≈ rank #74)

### Per-opponent record

| Rank | Opponent | Record | Their comp (most common) |
|---|---|---|---|
| 1 | Intercontinental Fish | 1-4 | Heart of Ruin/Marcella/Vesper Vale |
| 2 | Mite | 1-4 | Heart of Ruin/Marcella/Tavi/Vendetta/Vesper Vale |
| 3 | willow best girl | 0-5 | Heart of Ruin/Marcella/Prism/Vesper Vale/Willow |
| 4 | Trzy Kwaterki | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 5 | Claudus Maximus | 1-4 | Heart of Ruin/Marcella/Tavi/Vendetta/Vesper Vale |
| 6 | policy wonk | 0-5 | Heart of Ruin/Marcella/Vendetta/Vesper Vale/Willow |
| 7 | Sponge(Albert and Bob) | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 8 | prism best girl | 0-5 | Heart of Ruin/Marcella/Prism/Vendetta/Vesper Vale |
| 9 | procless | 0-5 | Heart of Ruin/Vendetta/Vesper Vale |
| 10 | hello | 0-5 | Marcella/Ryth/Tavi/Vendetta/Vesper Vale |
| 11 | new team(4) | 1-4 | Heart of Ruin/Marcella/Ryth/Vesper Vale |
| 13 | Ad Astra | 0-5 | Heart of Ruin/Marcella/Ryth/Varg/Vesper Vale |
| 13 | ひななー | 1-4 | Heart of Ruin/Ryth/Vendetta/Vesper Vale/Willow |
| 15 | NEETCoders | 0-5 | Heart of Ruin/Ryth/Tavi/Vendetta |
| 15 | print dont int | 0-5 | Cassi/Heart of Ruin/Issac/Marcella/Vendetta/Vesper Vale |
| 16 | Nyx | 1-4 | Heart of Ruin/Marcella/Vendetta/Vesper Vale |
| 18 | Univardisty of Shingtonwash | 0-5 | Heart of Ruin/Marcella/Prism/Tavi/Willow |
| 19 | Supaswag | 0-5 | Heart of Ruin/Marcella/Vendetta/Vesper Vale |
| 20 | Nameless | 0-5 | Heart of Ruin/Marcella/Ryth/Vesper Vale |
| 20 | Noobot | 0-5 | Marcella/Ryth/Tavi/Vesper Vale/Willow |
| 21 | Tartarus | 0-5 | Heart of Ruin/Marcella/Ryth/Tavi/Vesper Vale |
| 21 | the ruliad | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 23 | Zarzorki | 0-5 | Heart of Ruin/Marcella/Vesper Vale |
| 24 | conqueror_of_openai | 0-5 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 25 | 𓎼𓃭𓅱𓂋𓇌 𓏏𓅱 𓂋𓄿 | 0-5 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 28 | FürFortnite | 0-5 | Heart of Ruin/Marcella/Ryth/Tavi/Vesper Vale |
| 30 | EMPEROR Tung | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 30 | 红包控股 | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 32 | Let's LARP again | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 33 | Jupiter | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 35 | meow | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 35 | smurphy | 0-5 | Heart of Ruin/Marcella/Varg/Vendetta/Vesper Vale |
| 36 | Sheeesh--- | 0-5 | Marcella/Prism/Tavi/Varg/Willow |
| 37 | PENTAKILL | 0-5 | Heart of Ruin/Marcella/Tavi/Vendetta/Vesper Vale |
| 38 | E=mc^2+AI^3 | 4-1 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 38 | 💩 Super Intelligence 💩 | 0-5 | Heart of Ruin/Marcella/Vendetta/Vesper Vale/Willow |
| 40 | SSSA | 3-2 | Heart of Ruin/Marcella/Vendetta/Vesper Vale/Willow |
| 42 | will think later | 2-3 | Marcella/Vendetta/Vesper Vale |
| 44 | Nanyang Technological University | 2-3 | Heart of Ruin/Marcella/Ren/Ryth/Vesper Vale |
| 44 | ha²t | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta |
| 45 | Valenki | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 46 | geese geese go | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 47 | dromedaries | 1-4 | Heart of Ruin/Marcella/Ryth/Tavi/Vesper Vale |
| 48 | EggsAfter11AM | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 48 | Quant Merch Traders | 0-5 | Marcella/Ryth/Tavi/Vendetta/Vesper Vale |
| 50 | Bamboo Teahouse | 1-4 | Marcella/Ryth/Vesper Vale/Willow |
| 50 | SilverSamurai | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 51 | The Party of Heroes | 0-5 | Heart of Ruin/Marcella/Vendetta/Vesper Vale/Willow |
| 53 | OneTap | 1-4 | Marcella/Prism/Ryth/Vesper Vale |
| 53 | 天天开心 :( | 1-4 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 54 | Shawn | 1-4 | Marcella/Vendetta/Vesper Vale |
| 55 | Sneaky Bush Campers | 1-4 | Heart of Ruin/Marcella/Vesper Vale |
| 55 | janestreethireusplease | 3-2 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 56 | test4 | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 57 | SquidProxyLovers | 2-3 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 61 | jazz | 3-2 | Heart of Ruin/Marcella/Ryth/Vesper Vale |
| 62 | Zkey Farming | 0-5 | Marcella/Ryth/Tavi/Vesper Vale/Willow |
| 62 | error404 | 3-2 | Cassi/Issac/Marcella/Ryth/Varg/Vendetta |
| 63 | Serious-Wyso | 2-3 | Tavi/Vendetta/Vesper Vale |
| 64 | PtitLK | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 64 | filter | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 65 | 剑桥夜爬者 | 1-4 | Heart of Ruin/Prism/Stellar/Tavi/Willow |
| 67 | The Mania | 2-3 | Heart of Ruin/Marcella/Ryth/Varg/Vendetta |
| 68 | We NEED internships 🙏 | 2-3 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 69 | SkuBap | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 71 | Feral Monoliths | 1-4 | Marcella/Tavi/Varg/Vendetta/Vesper Vale |
| 71 | Sydney's Realist | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 72 | Still Testing | 2-3 | Marcella/Ren/Ryth |
| 75 | Patient Builder | 2-3 | Marcella/Tavi/Varg/Vendetta/Vesper Vale |
| 76 | AlphaSlop | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 76 | HouseOfCoders | 0-5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 78 | :3 | 2-3 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 79 | Brakence Fan Club | 3-2 | Cassi/Issac/Marcella/Ren/Ryth/Vesper Vale |
| 79 | Rogue Lock Pickers | 0-5 | Marcella/Varg/Vendetta/Vesper Vale/Willow |
| 80 | Live Love Larp | 0-5 | Marcella/Ren/Ryth/Vendetta/Vesper Vale |
| 81 | 2/27TELEVIZR+1/6EIT | 2-3 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 81 | BattleCats | 2-3 | Heart of Ruin/Ryth/Varg/Vendetta/Vesper Vale |
| 81 | Lucky Lock Pickers | 3-2 | Marcella/Ryth/Tavi/Vendetta/Vesper Vale |
| 82 | Velvet Tributers | 2-3 | Heart of Ruin/Marcella/Varg/Vendetta/Vesper Vale |
| 84 | Ninja Turtles | 0-5 | Heart of Ruin/Ryth/Varg/Vendetta/Vesper Vale |
| 85 | an iq too high | 3-2 | Marcella/Prism/Ryth/Vesper Vale |
| 86 | Manta | 4-1 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 86 | Stranger Danger | 2-3 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 88 | DOOMSDAY | 3-2 | Marcella/Prism/Ryth/Varg/Vendetta |
| 88 | German Empire Destroyers | 0-5 | Heart of Ruin/Marcella/Varg/Vesper Vale/Willow |
| 89 | KKB | 3-2 | Marcella/Ren/Ryth/Varg/Vendetta |
| 89 | cronjohn | 3-2 | Heart of Ruin/Marcella/Ryth/Tavi/Vesper Vale |
| 91 | bentheboss | 1-4 | Stellar/Tavi/Varg/Vendetta/Vesper Vale |
| 92 | kitfunso | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 93 | Tzunami | 3-2 | Heart of Ruin/Marcella/Varg/Vendetta/Vesper Vale |
| 93 | sudo rm -rf Imperial | 2-3 | Heart of Ruin/Marcella/Ryth/Vesper Vale |
| 95 | Hidden Altar Keepers | 3-2 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 96 | KratosMessi | 2-3 | Heart of Ruin/Ren/Varg/Vendetta/Vesper Vale |
| 96 | Team USA🇺🇸 | 2-3 | Marcella/Ryth/Vendetta |
| 96 | we were fed fish | 1-4 | Heart of Ruin/Ryth/Varg/Vendetta/Vesper Vale |
| 98 | Velvet Recallers | 1-4 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 98 | cheesynachos | 1-4 | Heart of Ruin/Marcella/Varg/Vendetta/Vesper Vale |

### Our wins: us vs opponent (n=104)

| Metric | Us | Opponent | Us − Opp |
|---|---|---|---|
| Win | 100% | 0% | |
| Gold earned @100 | 378 | 326 | 52 |
| Gold earned @300 | 1,384 | 1,080 | 304 |
| Gold earned @500 | 2,704 | 1,991 | 713 |
| Gold earned @1000 | 6,738 | 4,283 | 2,455 |
| Gems tributed @300 | 81 | 69 | 12 |
| Gems tributed @500 | 188 | 142 | 46 |
| Gems tributed @1000 | 504 | 307 | 198 |
| Gems collected @500 | 248 | 212 | 36 |
| Gems carried @500 | 53 | 46 | 7.3 |
| Gold earned (game) | 11,559 | 6,267 | 5,292 |
| Gems collected (game) | 1,262 | 837 | 425 |
| Gems tributed (game) | 1,007 | 473 | 534 |
| Kills | 16 | 6.0 | 10 |
| Deaths | 7.0 | 18 | -11 |
| Deaths @300 | 1.5 | 2.9 | -1.4 |
| Kills @300 | 2.1 | 1.0 | 1.0 |
| Camp clears | 79 | 52 | 27 |
| Tributes | 31 | 16 | 15 |
| Gems per tribute | 52 | 38 | 14 |
| First tribute (round) | 252 | 241 | 11 |
| First kill (round) | 265 | 329 | -64 |
| First death (round) | 239 | 187 | 52 |
| Gems lost on death (events) | 7.0 | 18 | -11 |
| Carried gems at end | 98 | 103 | -5.4 |
| Unspent gold at end | 715 | 217 | 499 |
| Monster damage | 153,524 | 107,340 | 46,184 |
| Champion damage done | 57,107 | 40,545 | 16,562 |
| Healing done | 0.0 | 1,274 | -1,274 |
| Shielding done | 22,268 | 15,422 | 6,846 |
| Wards placed | 168 | 179 | -11 |
| Altar locks bought | 9.6 | 4.8 | 4.8 |
| Teleport crystals bought | 0.0 | 3.1 | -3.1 |
| Fuel-exhausted turns | 143 | 13 | 130 |
| Game length | 1,423 | 1,423 | 0.0 |

### Our losses: us vs opponent (n=381)

| Metric | Us | Opponent | Us − Opp |
|---|---|---|---|
| Win | 0% | 100% | |
| Gold earned @100 | 285 | 367 | -82 |
| Gold earned @300 | 1,009 | 1,420 | -411 |
| Gold earned @500 | 1,859 | 2,757 | -898 |
| Gold earned @1000 | 4,281 | 6,854 | -2,573 |
| Gems tributed @300 | 63 | 114 | -51 |
| Gems tributed @500 | 130 | 245 | -115 |
| Gems tributed @1000 | 288 | 582 | -294 |
| Gems collected @500 | 192 | 317 | -124 |
| Gems carried @500 | 44 | 61 | -16 |
| Gold earned (game) | 5,471 | 9,516 | -4,045 |
| Gems collected (game) | 571 | 1,170 | -598 |
| Gems tributed (game) | 394 | 1,031 | -637 |
| Kills | 3.9 | 14 | -9.7 |
| Deaths | 15 | 5.2 | 9.5 |
| Deaths @300 | 2.2 | 1.2 | 1.0 |
| Kills @300 | 0.7 | 1.7 | -1.0 |
| Camp clears | 53 | 63 | -10 |
| Tributes | 16 | 29 | -12 |
| Gems per tribute | 31 | 52 | -21 |
| First tribute (round) | 208 | 193 | 15 |
| First kill (round) | 367 | 281 | 86 |
| First death (round) | 220 | 285 | -65 |
| Gems lost on death (events) | 15 | 5.2 | 9.5 |
| Carried gems at end | 64 | 63 | 1.0 |
| Unspent gold at end | 222 | 769 | -547 |
| Monster damage | 86,526 | 140,755 | -54,229 |
| Champion damage done | 26,926 | 44,036 | -17,109 |
| Healing done | 0.0 | 2,001 | -2,001 |
| Shielding done | 11,193 | 20,173 | -8,980 |
| Wards placed | 142 | 200 | -58 |
| Altar locks bought | 8.4 | 4.1 | 4.3 |
| Teleport crystals bought | 0.0 | 5.5 | -5.5 |
| Fuel-exhausted turns | 140 | 13 | 127 |
| Game length | 1,230 | 1,230 | 0.0 |

### Against ranks 1–30 (n=140)

| Metric | Us | Opponent | Us − Opp |
|---|---|---|---|
| Win | 4% | 96% | |
| Gold earned @100 | 293 | 400 | -107 |
| Gold earned @300 | 1,027 | 1,546 | -519 |
| Gold earned @500 | 1,876 | 3,072 | -1,197 |
| Gold earned @1000 | 4,123 | 7,561 | -3,438 |
| Gems tributed @300 | 63 | 121 | -58 |
| Gems tributed @500 | 131 | 266 | -135 |
| Gems tributed @1000 | 260 | 616 | -356 |
| Gems collected @500 | 200 | 359 | -159 |
| Gems carried @500 | 45 | 82 | -36 |
| Gold earned (game) | 4,930 | 9,448 | -4,518 |
| Gems collected (game) | 545 | 1,199 | -653 |
| Gems tributed (game) | 340 | 1,036 | -696 |
| Kills | 3.8 | 14 | -9.7 |
| Deaths | 14 | 5.0 | 9.3 |
| Deaths @300 | 2.3 | 1.2 | 1.1 |
| Kills @300 | 0.8 | 1.9 | -1.1 |
| Camp clears | 45 | 60 | -15 |
| Tributes | 13 | 28 | -14 |
| Gems per tribute | 29 | 57 | -28 |
| First tribute (round) | 192 | 175 | 18 |
| First kill (round) | 350 | 260 | 91 |
| First death (round) | 223 | 286 | -63 |
| Gems lost on death (events) | 14 | 5.0 | 9.3 |
| Carried gems at end | 50 | 69 | -19 |
| Unspent gold at end | 204 | 932 | -728 |
| Monster damage | 78,020 | 139,049 | -61,029 |
| Champion damage done | 25,448 | 40,814 | -15,366 |
| Healing done | 0.0 | 2,027 | -2,027 |
| Shielding done | 10,053 | 18,894 | -8,841 |
| Wards placed | 115 | 145 | -30 |
| Altar locks bought | 8.2 | 4.1 | 4.1 |
| Teleport crystals bought | 0.0 | 6.7 | -6.7 |
| Fuel-exhausted turns | 122 | 9.0 | 113 |
| Game length | 1,122 | 1,122 | 0.0 |

### Against ranks 31–60 (n=135)

| Metric | Us | Opponent | Us − Opp |
|---|---|---|---|
| Win | 21% | 79% | |
| Gold earned @100 | 319 | 365 | -47 |
| Gold earned @300 | 1,102 | 1,370 | -268 |
| Gold earned @500 | 2,043 | 2,576 | -533 |
| Gold earned @1000 | 4,786 | 6,119 | -1,332 |
| Gems tributed @300 | 69 | 96 | -27 |
| Gems tributed @500 | 143 | 198 | -55 |
| Gems tributed @1000 | 314 | 491 | -177 |
| Gems collected @500 | 193 | 261 | -68 |
| Gems carried @500 | 39 | 52 | -13 |
| Gold earned (game) | 7,042 | 9,075 | -2,033 |
| Gems collected (game) | 728 | 1,157 | -429 |
| Gems tributed (game) | 525 | 906 | -381 |
| Kills | 6.6 | 13 | -6.6 |
| Deaths | 14 | 7.8 | 6.5 |
| Deaths @300 | 1.9 | 1.5 | 0.5 |
| Kills @300 | 1.0 | 1.5 | -0.5 |
| Camp clears | 64 | 62 | 1.7 |
| Tributes | 20 | 25 | -4.6 |
| Gems per tribute | 35 | 50 | -15 |
| First tribute (round) | 229 | 236 | -6.9 |
| First kill (round) | 343 | 291 | 52 |
| First death (round) | 223 | 262 | -38 |
| Gems lost on death (events) | 14 | 7.8 | 6.5 |
| Carried gems at end | 48 | 129 | -81 |
| Unspent gold at end | 345 | 633 | -288 |
| Monster damage | 102,815 | 135,166 | -32,351 |
| Champion damage done | 36,922 | 48,297 | -11,374 |
| Healing done | 0.0 | 2,463 | -2,463 |
| Shielding done | 15,239 | 21,823 | -6,584 |
| Wards placed | 142 | 218 | -76 |
| Altar locks bought | 8.4 | 4.3 | 4.1 |
| Teleport crystals bought | 0.0 | 4.1 | -4.1 |
| Fuel-exhausted turns | 137 | 18 | 119 |
| Game length | 1,336 | 1,336 | 0.0 |

### Against ranks 61–100 (n=210)

| Metric | Us | Opponent | Us − Opp |
|---|---|---|---|
| Win | 33% | 67% | |
| Gold earned @100 | 304 | 325 | -22 |
| Gold earned @300 | 1,123 | 1,200 | -77 |
| Gold earned @500 | 2,148 | 2,285 | -136 |
| Gold earned @1000 | 5,332 | 5,609 | -276 |
| Gems tributed @300 | 69 | 99 | -30 |
| Gems tributed @500 | 150 | 210 | -60 |
| Gems tributed @1000 | 399 | 477 | -78 |
| Gems collected @500 | 215 | 273 | -58 |
| Gems carried @500 | 52 | 45 | 6.6 |
| Gold earned (game) | 7,837 | 8,236 | -399 |
| Gems collected (game) | 830 | 994 | -164 |
| Gems tributed (game) | 650 | 832 | -182 |
| Kills | 8.4 | 10 | -1.8 |
| Deaths | 11 | 10 | 1.2 |
| Deaths @300 | 1.9 | 1.8 | 0.1 |
| Kills @300 | 1.2 | 1.4 | -0.2 |
| Camp clears | 64 | 60 | 3.8 |
| Tributes | 23 | 25 | -2.2 |
| Gems per tribute | 39 | 43 | -3.7 |
| First tribute (round) | 228 | 201 | 27 |
| First kill (round) | 336 | 313 | 23 |
| First death (round) | 226 | 249 | -23 |
| Gems lost on death (events) | 11 | 10 | 1.2 |
| Carried gems at end | 100 | 36 | 64 |
| Unspent gold at end | 399 | 474 | -74 |
| Monster damage | 114,904 | 128,936 | -14,032 |
| Champion damage done | 36,432 | 41,715 | -5,283 |
| Healing done | 0.0 | 1,327 | -1,327 |
| Shielding done | 14,836 | 17,612 | -2,776 |
| Wards placed | 173 | 215 | -41 |
| Altar locks bought | 9.2 | 4.4 | 4.9 |
| Teleport crystals bought | 0.0 | 4.5 | -4.5 |
| Fuel-exhausted turns | 155 | 12 | 143 |
| Game length | 1,330 | 1,330 | 0.0 |

### By map

| Map | Games | Wins | Win rate | Mean opp rank |
|---|---|---|---|---|
| 00interlock | 11 | 0 | 0% | 41 |
| 01shortcut | 11 | 0 | 0% | 42 |
| 01elk_arena | 13 | 0 | 0% | 59 |
| 00expanse | 13 | 1 | 8% | 64 |
| 01gold_rush | 13 | 1 | 8% | 52 |
| 00lost_world | 12 | 1 | 8% | 57 |
| 00ambush | 12 | 1 | 8% | 58 |
| 00great_wall | 12 | 1 | 8% | 49 |
| 00road_of_riches | 11 | 1 | 9% | 51 |
| 01big_world | 11 | 1 | 9% | 48 |
| 00quadrants | 11 | 1 | 9% | 49 |
| 00lanes | 11 | 1 | 9% | 50 |
| 00split | 11 | 1 | 9% | 55 |
| 00corners | 14 | 2 | 14% | 42 |
| 00rows | 14 | 2 | 14% | 46 |
| 01fracture | 13 | 2 | 15% | 68 |
| 00gem_fields | 13 | 2 | 15% | 55 |
| 00collision | 13 | 2 | 15% | 49 |
| 01corridors | 12 | 2 | 17% | 68 |
| 00reverse_spiral | 11 | 2 | 18% | 54 |
| 00pilgrimage | 11 | 2 | 18% | 37 |
| 00stampede | 14 | 3 | 21% | 57 |
| 01starvation | 14 | 3 | 21% | 45 |
| 01starfish | 9 | 2 | 22% | 55 |
| 01yellow_brick_road | 13 | 3 | 23% | 59 |
| 01boss_battle | 12 | 3 | 25% | 37 |
| 00eight | 12 | 3 | 25% | 45 |
| 01islands | 11 | 3 | 27% | 47 |
| 01concentrated_wealth | 11 | 3 | 27% | 49 |
| 01the_farm | 11 | 3 | 27% | 55 |
| 01hooks | 12 | 4 | 33% | 37 |
| 01broken_curve | 12 | 4 | 33% | 50 |
| 00packs | 11 | 4 | 36% | 61 |
| 00monolith | 13 | 5 | 38% | 53 |
| 00swirl | 13 | 5 | 38% | 57 |
| 00contest | 14 | 6 | 43% | 61 |
| 00gemfight | 11 | 5 | 45% | 45 |
| 00fragment | 13 | 6 | 46% | 49 |
| 00elkring | 14 | 7 | 50% | 49 |
| 00rushbait_v2 | 12 | 6 | 50% | 54 |

### By side

- Side A: 54/239 (23%)
- Side B: 50/246 (20%)

### End reasons

- loss by gems: 365
- loss by round_limit: 16
- win by gems: 92
- win by round_limit: 12

### Opponent champion presence vs our result

| Opponent has | Games | Our win rate |
|---|---|---|
| Stellar | 9 | 0% |
| Willow | 66 | 9% |
| Tavi | 87 | 18% |
| Prism | 41 | 20% |
| Vesper Vale | 427 | 21% |
| Marcella | 419 | 21% |
| Heart of Ruin | 353 | 21% |
| Vendetta | 335 | 21% |
| Ryth | 295 | 22% |
| Varg | 100 | 23% |
| Cassi | 22 | 32% |
| Issac | 22 | 32% |
| Ren | 30 | 33% |

### Our champions: production per game, wins vs losses

| Champion | Games | Gold W/L | Gems collected W/L | Kills W/L | Deaths W/L |
|---|---|---|---|---|---|
| Heart of Ruin | 485 | 1,499/746 | 163/85 | 2.1/0.6 | 1.8/3.3 |
| Marcella | 485 | 2,114/826 | 219/88 | 2.7/0.5 | 1.5/3.4 |
| Ryth | 317 | 1,394/738 | 221/87 | 1.0/0.3 | 2.0/3.0 |
| Vendetta | 412 | 1,965/872 | 239/101 | 3.5/0.9 | 2.1/3.9 |
| Vesper Vale | 485 | 5,555/2,651 | 560/253 | 8.2/1.9 | 0.8/2.6 |

### Opponent champions that beat us: production in our losses

| Champion | Games | Gold | Gems collected | Kills | Deaths |
|---|---|---|---|---|---|
| Vesper Vale | 339 | 3,072 | 346 | 3.5 | 0.8 |
| Marcella | 332 | 2,896 | 310 | 4.6 | 0.9 |
| Vendetta | 264 | 1,931 | 287 | 3.1 | 1.2 |
| Heart of Ruin | 279 | 1,778 | 234 | 3.2 | 1.2 |
| Stellar | 9 | 1,706 | 153 | 6.1 | 2.1 |
| Ren | 20 | 1,566 | 164 | 2.4 | 0.7 |
| Varg | 77 | 1,458 | 229 | 1.1 | 1.3 |
| Prism | 33 | 1,363 | 196 | 3.0 | 1.8 |
| Issac | 15 | 1,258 | 143 | 1.7 | 2.0 |
| Ryth | 229 | 1,242 | 179 | 1.2 | 1.3 |
| Tavi | 71 | 1,003 | 118 | 2.0 | 2.0 |
| Cassi | 15 | 615 | 70 | 1.1 | 2.0 |
| Willow | 60 | 492 | 55 | 0.4 | 1.7 |

### Emblems per game (ours vs opponents in our losses)

| Emblem | Ours (all games) | Opponents (our losses) |
|---|---|---|
| Bruiser Emblem of Strength | 26.06 | 16.28 |
| Bruiser Emblem of Health | 1.81 | 11.69 |
| Bruiser Emblem of Focus | 3.02 | 8.25 |
| Durability Emblem of Health | 2.95 | 3.20 |
| Power Emblem of Strength | 0.00 | 4.35 |
| Treasure Emblem of Strength | 1.75 | 1.49 |
| Treasure Emblem of Health | 1.47 | 1.36 |
| Jungle Emblem of Strength | 0.65 | 1.83 |
| Jungle Emblem of Health | 1.25 | 0.75 |
| Power Emblem of Focus | 0.00 | 1.98 |
| Treasure Emblem of Focus | 0.89 | 0.80 |
| Jungle Emblem of Focus | 0.78 | 0.76 |
| Power Emblem of Health | 0.00 | 1.33 |
| Monster Emblem of Strength | 0.00 | 0.52 |
| Durability Emblem of Strength | 0.00 | 0.24 |
| Home Emblem of Health | 0.00 | 0.22 |
| Monster Emblem of Health | 0.00 | 0.20 |
| Monster Emblem of Focus | 0.00 | 0.18 |
| Durability Emblem of Focus | 0.00 | 0.14 |
| Home Emblem of Focus | 0.00 | 0.05 |
| Home Emblem of Strength | 0.00 | 0.04 |

## A. Top-100 recent ranked games (n=747 matches, 99 teams)

| Metric | 1–10 | 11–30 | 31–60 | 61–100 |
|---|---|---|---|---|
| Team-games | 150 | 300 | 445 | 565 |
| Win | 51% | 51% | 52% | 49% |
| Gold @100 | 385 | 351 | 331 | 310 |
| Gold @300 | 1,438 | 1,341 | 1,237 | 1,123 |
| Gold @500 | 2,732 | 2,591 | 2,364 | 2,133 |
| Gold @1000 | 6,410 | 5,918 | 5,550 | 5,145 |
| Gems tributed @500 | 199 | 233 | 194 | 195 |
| Gems tributed @1000 | 442 | 473 | 462 | 448 |
| Gold earned (game) | 7,739 | 7,241 | 7,609 | 7,163 |
| Kills | 9.0 | 7.6 | 8.6 | 8.0 |
| Deaths | 10 | 8.7 | 9.2 | 9.4 |
| Camp clears | 54 | 52 | 54 | 57 |
| Gems per tribute | 43 | 43 | 46 | 40 |
| First tribute | 199 | 186 | 229 | 204 |
| Carried gems at end | 87 | 69 | 45 | 67 |
| Unspent gold | 547 | 441 | 442 | 362 |
| Altar locks | 5.4 | 5.3 | 4.8 | 4.6 |
| Crystals | 4.7 | 4.8 | 4.3 | 2.5 |
| Wards | 142 | 156 | 229 | 218 |
| Healing | 1,442 | 1,908 | 2,912 | 1,051 |
| Fuel-exhausted turns | 27 | 3.9 | 24 | 12 |
| Game length | 1,158 | 1,163 | 1,265 | 1,279 |

### Champion pick rate by bucket (top-100 ranked)

| Champion | 1–10 | 11–30 | 31–60 | 61–100 |
|---|---|---|---|---|
| Cassi | 11% | 5% | 5% | 5% |
| Heart of Ruin | 79% | 75% | 81% | 70% |
| Issac | 11% | 5% | 5% | 5% |
| Marcella | 81% | 88% | 83% | 82% |
| Prism | 25% | 3% | 11% | 8% |
| Ren | 2% | 2% | 6% | 10% |
| Ryth | 47% | 76% | 60% | 69% |
| Stellar | 1% | 3% | 3% | 4% |
| Tavi | 21% | 17% | 19% | 13% |
| Varg | 3% | 19% | 12% | 25% |
| Vendetta | 62% | 63% | 66% | 70% |
| Vesper Vale | 90% | 89% | 89% | 84% |
| Willow | 13% | 15% | 19% | 9% |

### Most common comps in ranks 1–30

- Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale: 116 team-games, win rate 51%
- Heart of Ruin/Marcella/Vesper Vale: 27 team-games, win rate 56%
- Heart of Ruin/Marcella/Ryth/Vesper Vale: 25 team-games, win rate 48%
- Marcella/Ryth/Varg/Vendetta/Vesper Vale: 20 team-games, win rate 60%
- Marcella/Ryth/Tavi/Vendetta/Vesper Vale: 18 team-games, win rate 67%
- Heart of Ruin/Marcella/Ryth/Varg/Vesper Vale: 15 team-games, win rate 40%
- Heart of Ruin/Vendetta/Vesper Vale: 13 team-games, win rate 46%
- Heart of Ruin/Marcella/Ryth/Tavi/Vesper Vale: 13 team-games, win rate 62%

### Emblems per team-game by bucket

| Emblem | 1–10 | 11–30 | 31–60 | 61–100 |
|---|---|---|---|---|
| Bruiser Emblem of Strength | 15.25 | 12.79 | 12.04 | 13.28 |
| Bruiser Emblem of Health | 10.77 | 9.81 | 7.20 | 6.65 |
| Bruiser Emblem of Focus | 5.39 | 7.16 | 7.30 | 4.34 |
| Power Emblem of Strength | 1.92 | 1.47 | 3.76 | 4.42 |
| Durability Emblem of Health | 2.30 | 2.90 | 2.88 | 3.02 |
| Jungle Emblem of Strength | 1.83 | 1.76 | 1.76 | 1.55 |
| Treasure Emblem of Health | 1.51 | 1.26 | 1.32 | 1.61 |
| Treasure Emblem of Strength | 1.18 | 1.67 | 1.37 | 1.34 |
| Power Emblem of Focus | 0.73 | 0.61 | 2.26 | 1.77 |
| Power Emblem of Health | 0.02 | 0.00 | 1.54 | 1.76 |
| Jungle Emblem of Focus | 0.89 | 0.76 | 0.96 | 0.55 |
| Treasure Emblem of Focus | 0.65 | 0.97 | 0.81 | 0.46 |
| Jungle Emblem of Health | 0.68 | 0.58 | 0.51 | 0.93 |
| Monster Emblem of Strength | 0.61 | 0.40 | 0.40 | 0.57 |
| Durability Emblem of Strength | 0.50 | 0.12 | 0.11 | 0.23 |
| Monster Emblem of Health | 0.49 | 0.10 | 0.19 | 0.08 |
| Home Emblem of Health | 0.38 | 0.17 | 0.07 | 0.10 |
| Monster Emblem of Focus | 0.23 | 0.04 | 0.23 | 0.19 |
| Durability Emblem of Focus | 0.25 | 0.07 | 0.13 | 0.13 |
| Home Emblem of Strength | 0.00 | 0.00 | 0.12 | 0.02 |

### Per-team profile, ranks 1–30 (recent ranked)

| Rank | Team | n | WR | Gold@300 | Gems@1000 | K/D | Gems/tribute | Locks | Crystals | Fuel-ex | Main comp |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Intercontinental Fish | 15 | 67% | 1,526 | 611 | 11/15 | 50 | 5.2 | 0.0 | 0.0 | Cassi/Heart of Ruin/Issac/Vendetta/Vesper Vale |
| 2 | Mite | 15 | 67% | 1,719 | 539 | 11/12 | 36 | 9.2 | 3.0 | 25 | Marcella/Ryth/Tavi/Vendetta/Vesper Vale |
| 3 | willow best girl | 15 | 53% | 1,382 | 422 | 11/4.9 | 38 | 7.4 | 0.2 | 4.1 | Heart of Ruin/Marcella/Prism/Vesper Vale/Willow |
| 4 | Trzy Kwaterki | 15 | 53% | 1,522 | 372 | 8.1/7.3 | 29 | 0.0 | 7.5 | 22 | Marcella/Ryth/Vendetta |
| 5 | Claudus Maximus | 15 | 60% | 1,573 | 589 | 11/9.5 | 36 | 11 | 0.0 | 0.1 | Heart of Ruin/Marcella/Tavi/Vendetta/Vesper Vale |
| 6 | policy wonk | 15 | 47% | 1,388 | 392 | 7.7/9.1 | 67 | 4.5 | 4.0 | 0.0 | Heart of Ruin/Marcella/Tavi/Vesper Vale/Willow |
| 7 | Sponge(Albert and Bob) | 15 | 67% | 1,482 | 464 | 10/8.7 | 35 | 9.1 | 22 | 13 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 8 | prism best girl | 15 | 20% | 1,182 | 94 | 6.4/18 | 55 | 0.0 | 3.5 | 0.0 | Heart of Ruin/Marcella/Prism/Ryth/Vesper Vale |
| 9 | procless | 15 | 40% | 1,473 | 439 | 7.9/10 | 41 | 0.0 | 0.9 | 1.3 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 10 | :3 | 15 | 33% | 1,134 | 418 | 4.3/9.9 | 43 | 7.6 | 5.7 | 206 | Heart of Ruin/Marcella/Prism/Vendetta/Vesper Vale |
| 11 | new team(4) | 15 | 80% | 1,420 | 547 | 4.4/6.1 | 53 | 8.2 | 11 | 0.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 12 | hello | 15 | 60% | 1,286 | 552 | 8.8/6.7 | 37 | 4.5 | 0.5 | 3.0 | Marcella/Ryth/Tavi/Vendetta/Vesper Vale |
| 13 | Ad Astra | 15 | 47% | 1,283 | 610 | 6.8/4.0 | 52 | 4.0 | 0.0 | 0.0 | Heart of Ruin/Marcella/Ryth/Varg/Vesper Vale |
| 14 | ひななー | 15 | 40% | 1,707 | 677 | 3.7/6.3 | 44 | 0.0 | 1.9 | 3.7 | Heart of Ruin/Ryth/Vendetta/Vesper Vale/Willow |
| 15 | NEETCoders | 15 | 33% | 1,385 | 479 | 9.3/8.7 | 33 | 5.1 | 4.5 | 5.7 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 16 | Univardisty of Shingtonwash | 15 | 33% | 1,313 | 409 | 7.2/9.4 | 41 | 0.0 | 2.9 | 23 | Marcella/Ryth/Tavi/Vendetta/Willow |
| 17 | print dont int | 15 | 60% | 1,373 | 402 | 11/8.9 | 45 | 2.3 | 6.3 | 2.9 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 18 | Supaswag | 15 | 53% | 1,193 | 543 | 8.3/12 | 36 | 0.0 | 0.0 | 0.0 | Heart of Ruin/Marcella/Vendetta/Vesper Vale/Willow |
| 19 | Nameless | 15 | 53% | 1,372 | 489 | 11/5.0 | 57 | 5.1 | 15 | 4.6 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 20 | Tartarus | 15 | 60% | 1,350 | 426 | 15/10 | 26 | 4.7 | 1.1 | 8.6 | Heart of Ruin/Marcella/Ryth/Tavi/Vesper Vale |
| 21 | the ruliad | 15 | 53% | 1,443 | 620 | 5.2/5.3 | 42 | 11 | 2.3 | 0.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 22 | Zarzorki | 15 | 47% | 1,595 | 282 | 7.7/8.2 | 36 | 9.0 | 0.0 | 0.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 23 | Noobot | 15 | 47% | 1,283 | 426 | 6.5/8.7 | 52 | 8.1 | 10 | 1.1 | Marcella/Ryth/Tavi/Vesper Vale/Willow |
| 24 | 𓎼𓃭𓅱𓂋𓇌 𓏏𓅱 𓂋𓄿 | 15 | 60% | 1,371 | 632 | 7.9/8.3 | 37 | 10 | 4.8 | 0.5 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 25 | EMPEROR Tung | 15 | 60% | 1,442 | 516 | 3.7/9.3 | 59 | 6.7 | 13 | 8.2 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 26 | conqueror_of_openai | 15 | 53% | 1,332 | 359 | 7.2/13 | 40 | 6.0 | 6.9 | 0.5 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 27 | Nyx | 15 | 67% | 1,112 | 481 | 6.4/5.5 | 58 | 5.4 | 0.0 | 11 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 28 | Let's LARP again | 15 | 20% | 1,022 | 430 | 3.9/11 | 39 | 5.1 | 0.0 | 0.1 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 29 | subscribe to caterpillow | 15 | 27% | 1,316 | 387 | 3.3/11 | 38 | 9.5 | 0.0 | 0.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 30 | FürFortnite | 15 | 73% | 1,226 | 389 | 13/17 | 36 | 1.3 | 15 | 4.5 | Heart of Ruin/Marcella/Ryth/Vesper Vale/Willow |

### What separates winners from losers in top-100 ranked games

| Metric | Winner | Loser | Winner higher in |
|---|---|---|---|
| Gold @100 | 364 | 301 | 59% |
| Gold @300 | 1,420 | 1,048 | 74% |
| Gold @500 | 2,756 | 1,957 | 77% |
| Gold @1000 | 6,680 | 4,325 | 85% |
| Gems tributed @500 | 242 | 162 | 64% |
| Gems tributed @1000 | 569 | 339 | 79% |
| Gold earned (game) | 9,270 | 5,443 | 87% |
| Kills | 12 | 4.2 | 74% |
| Deaths | 5.5 | 13 | 18% |
| Camp clears | 62 | 48 | 72% |
| Gems per tribute | 51 | 33 | 66% |
| First tribute | 210 | 205 | 45% |
| Carried gems at end | 55 | 69 | 34% |
| Unspent gold | 613 | 225 | 77% |
| Altar locks | 4.8 | 5.1 | 40% |
| Crystals | 4.0 | 3.3 | 24% |
| Wards | 213 | 192 | 56% |
| Healing | 1,939 | 1,754 | 12% |
| Fuel-exhausted turns | 13 | 19 | 30% |
| Game length | 1,239 | 1,239 | 0% |

### P(leader wins), top-100 ranked

| Round | Gold-earned leader | Tributed-gems leader | Kill leader |
|---|---|---|---|
| 100 | 61% (n=728) | 55% (n=419) | 73% (n=228) |
| 300 | 74% (n=746) | 66% (n=662) | 74% (n=455) |
| 500 | 77% (n=743) | 70% (n=674) | 74% (n=537) |
| 1000 | 85% (n=551) | 84% (n=518) | 78% (n=486) |

# Part B: event-level features (deep_report.py)

Scrim games: 485; top-100 ranked team-games: 1455 (ranks 1-30: 450)

### Fuel-exhausted turns per game (team total)

| Group | Per game | In combat (took champion damage within 3 rounds) | Rounds 0–100 | 100–500 | 500+ | Per 100 rounds |
|---|---|---|---|---|---|---|
| v8 (scrims) | 140 | 17 (12%) | 20 | 48 | 72 | 12 |
| Scrim opponents | 13 | 1.1 (9%) | 4.3 | 3.3 | 5.3 | 1.1 |
| Top-100 ranked | 15 | 1.1 (7%) | 5.9 | 3.5 | 5.8 | 1.3 |
| Ranks 1–30 ranked | 12 | 0.9 (8%) | 7.8 | 2.1 | 1.7 | 1.3 |

**v8 fuel-exhausted turns by champion (per game it was drafted):**

| Champion | Games | Per game | In combat | Share of games with ≥50 |
|---|---|---|---|---|
| Vendetta | 412 | 39 | 4.4 | 26% |
| Heart of Ruin | 485 | 35 | 4.7 | 20% |
| Ryth | 317 | 33 | 3.7 | 22% |
| Marcella | 485 | 28 | 3.5 | 15% |
| Vesper Vale | 485 | 22 | 2.2 | 11% |

**Fuel-exhausted turns vs result (v8, scrims):**

- 0–50 per game: 129 games, win rate 26%
- 50–100 per game: 120 games, win rate 18%
- 100–200 per game: 146 games, win rate 21%
- 200–∞ per game: 90 games, win rate 22%

### Deaths

| Group | Deaths/game | to champions | to camps | Gems lost on death/game | Deaths before round 300 | Median first death |
|---|---|---|---|---|---|---|
| v8 (scrims) | 13 | 12 | 1.1 | 123 | 2.0 | 139 |
| Scrim opponents | 8.0 | 6.7 | 1.4 | 115 | 1.5 | 172 |
| Top-100 ranked | 9.3 | 8.1 | 1.2 | 130 | 1.8 | 160 |
| Ranks 1–30 ranked | 9.3 | 8.4 | 0.9 | 146 | 1.9 | 164 |

**v8 deaths by champion (scrims):**

| Champion | Deaths/game | Gems lost/game | Opponent same champion: deaths/game |
|---|---|---|---|
| Vendetta | 3.5 | 21 | 1.9 (n=335) |
| Heart of Ruin | 3.0 | 14 | 1.9 (n=353) |
| Marcella | 3.0 | 22 | 1.5 (n=419) |
| Ryth | 2.8 | 25 | 1.9 (n=295) |
| Vesper Vale | 2.2 | 52 | 1.4 (n=427) |

**Who kills v8 (champion, scrims):**

Marcella 27%, Vesper Vale 21%, Heart of Ruin 15%, Vendetta 15%, (camp/other) 8%, Ryth 5%, Tavi 2%, Varg 2%

### Same champion, head to head in scrims (gold earned)

| Champion | Games both drafted | v8 gold@300 | Opp gold@300 | v8 gold@1000 | Opp gold@1000 |
|---|---|---|---|---|---|
| Vesper Vale | 427 | 527 | 462 | 2,386 | 2,093 |
| Marcella | 419 | 157 | 375 | 731 | 1,779 |
| Heart of Ruin | 353 | 145 | 257 | 642 | 1,149 |
| Vendetta | 301 | 171 | 259 | 824 | 1,277 |
| Ryth | 226 | 153 | 184 | 679 | 892 |

### First three emblems by champion: v8 vs ranks 1–30 (ranked)

**Vesper Vale**

- v8 (n=448): Treasure Emblem of Strength → Bruiser Emblem of Strength → Durability Emblem of Health (100%)
- Ranks 1–30 (n=348): Treasure Emblem of Strength → Jungle Emblem of Strength → Bruiser Emblem of Health (31%); Treasure Emblem of Strength → Power Emblem of Strength → Durability Emblem of Health (10%); Treasure Emblem of Strength → Jungle Emblem of Strength → Durability Emblem of Health (9%); Treasure Emblem of Strength → Jungle Emblem of Strength → Power Emblem of Strength (5%)
- Emblem stat mix, v8: Strength 95%, Health 5%; ranks 1–30: Strength 73%, Health 27%
- Emblems completed per game, v8: 20; ranks 1–30: 14

**Marcella**

- v8 (n=330): Treasure Emblem of Health → Jungle Emblem of Health → Bruiser Emblem of Strength (100%)
- Ranks 1–30 (n=330): Treasure Emblem of Health → Jungle Emblem of Strength → Bruiser Emblem of Health (20%); Treasure Emblem of Focus → Durability Emblem of Health → Bruiser Emblem of Focus (9%); Treasure Emblem of Focus → Jungle Emblem of Focus → Durability Emblem of Health (8%); Treasure Emblem of Focus → Jungle Emblem of Health → Bruiser Emblem of Focus (5%)
- Emblem stat mix, v8: Strength 65%, Health 35%; ranks 1–30: Focus 41%, Health 35%, Strength 25%
- Emblems completed per game, v8: 6.4; ranks 1–30: 12

**Vendetta**

- v8 (n=277): Treasure Emblem of Strength → Jungle Emblem of Strength → Bruiser Emblem of Strength (100%)
- Ranks 1–30 (n=231): Treasure Emblem of Health → Jungle Emblem of Strength → Bruiser Emblem of Health (39%); Treasure Emblem of Strength → Jungle Emblem of Strength → Durability Emblem of Strength (14%); Treasure Emblem of Strength → Jungle Emblem of Health → Bruiser Emblem of Strength (7%); Treasure Emblem of Health → Jungle Emblem of Strength → Power Emblem of Strength (6%)
- Emblem stat mix, v8: Strength 91%, Health 9%; ranks 1–30: Strength 61%, Health 39%
- Emblems completed per game, v8: 6.5; ranks 1–30: 9.4

**Heart of Ruin**

- v8 (n=333): Treasure Emblem of Focus → Jungle Emblem of Focus → Bruiser Emblem of Focus (100%)
- Ranks 1–30 (n=260): Treasure Emblem of Health → Jungle Emblem of Focus → Bruiser Emblem of Health (24%); Treasure Emblem of Health → Jungle Emblem of Focus → Durability Emblem of Health (10%); Jungle Emblem of Focus → Treasure Emblem of Focus → Durability Emblem of Health (10%); Treasure Emblem of Focus → Jungle Emblem of Focus → Bruiser Emblem of Focus (6%)
- Emblem stat mix, v8: Focus 89%, Health 11%; ranks 1–30: Focus 62%, Health 37%, Strength 1%
- Emblems completed per game, v8: 5.2; ranks 1–30: 7.9

**Ryth**

- v8 (n=203): Treasure Emblem of Health → Jungle Emblem of Health → Bruiser Emblem of Health (100%)
- Ranks 1–30 (n=192): Treasure Emblem of Health → Jungle Emblem of Strength → Bruiser Emblem of Health (44%); Treasure Emblem of Health → Durability Emblem of Health → Bruiser Emblem of Health (5%); Treasure Emblem of Health → Bruiser Emblem of Health → Durability Emblem of Health (5%); Durability Emblem of Health → Jungle Emblem of Health → Bruiser Emblem of Health (5%)
- Emblem stat mix, v8: Health 100%; ranks 1–30: Health 55%, Strength 44%, Focus 1%
- Emblems completed per game, v8: 4.9; ranks 1–30: 5.5

### Teleport crystals in top-100 ranked games

- Teams buying at least one: 29% of team-games; 3.7 per team-game (ranks 1–30: 4.7).
- Bought by: Marcella 24%, Vesper Vale 19%, Heart of Ruin 15%, Vendetta 14%, Ryth 14%, Varg 4%
- Purchase round: median 533, quartiles 255–854
- Win rate with crystals 48% (n=426), without 52% (n=1029)
- v8 crystals bought in 485 scrims: 0

# Part C: match-database dump, 24 Oct parts 1–2 (3,642 ranked top-100 games)

## Top-100 recent ranked games (n=3642 matches, 99 teams)

| Metric | 1–10 | 11–30 | 31–60 | 61–100 |
|---|---|---|---|---|
| Team-games | 631 | 1301 | 1990 | 2608 |
| Win | 52% | 51% | 51% | 50% |
| Gold @100 | 381 | 351 | 327 | 310 |
| Gold @300 | 1,454 | 1,317 | 1,199 | 1,135 |
| Gold @500 | 2,793 | 2,497 | 2,259 | 2,159 |
| Gold @1000 | 6,235 | 5,840 | 5,337 | 5,145 |
| Gems tributed @500 | 211 | 211 | 196 | 194 |
| Gems tributed @1000 | 459 | 486 | 453 | 447 |
| Gold earned (game) | 7,427 | 7,402 | 7,337 | 7,178 |
| Kills | 7.8 | 8.0 | 8.8 | 8.0 |
| Deaths | 8.6 | 8.8 | 9.9 | 9.5 |
| Camp clears | 53 | 56 | 55 | 57 |
| Gems per tribute | 47 | 44 | 43 | 43 |
| First tribute | 211 | 203 | 204 | 209 |
| Carried gems at end | 103 | 82 | 52 | 67 |
| Unspent gold | 529 | 450 | 428 | 380 |
| Altar locks | 4.4 | 4.7 | 5.0 | 4.7 |
| Crystals | 5.6 | 3.9 | 4.2 | 2.6 |
| Wards | 147 | 152 | 225 | 218 |
| Healing | 1,517 | 1,813 | 2,205 | 1,014 |
| Fuel-exhausted turns | 23 | 4.4 | 25 | 20 |
| Game length | 1,101 | 1,182 | 1,268 | 1,282 |

### Champion pick rate by bucket (top-100 ranked)

| Champion | 1–10 | 11–30 | 31–60 | 61–100 |
|---|---|---|---|---|
| Cassi | 8% | 5% | 5% | 7% |
| Heart of Ruin | 84% | 80% | 75% | 70% |
| Issac | 8% | 5% | 5% | 7% |
| Marcella | 75% | 85% | 83% | 85% |
| Prism | 29% | 8% | 12% | 6% |
| Ren | 1% | 3% | 8% | 12% |
| Ryth | 46% | 69% | 59% | 72% |
| Stellar | 1% | 4% | 5% | 5% |
| Tavi | 23% | 15% | 17% | 13% |
| Varg | 4% | 18% | 16% | 24% |
| Vendetta | 64% | 61% | 66% | 72% |
| Vesper Vale | 94% | 88% | 87% | 82% |
| Willow | 15% | 15% | 18% | 8% |

### Most common comps in ranks 1–30

- Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale: 441 team-games, win rate 53%
- Heart of Ruin/Marcella/Ryth/Vesper Vale: 118 team-games, win rate 58%
- Heart of Ruin/Marcella/Prism/Vendetta/Vesper Vale: 92 team-games, win rate 52%
- Heart of Ruin/Marcella/Vesper Vale: 89 team-games, win rate 54%
- Heart of Ruin/Marcella/Ryth/Varg/Vesper Vale: 72 team-games, win rate 47%
- Marcella/Ryth/Tavi/Vendetta/Vesper Vale: 66 team-games, win rate 48%
- Heart of Ruin/Vendetta/Vesper Vale: 64 team-games, win rate 52%
- Marcella/Ryth/Varg/Vendetta/Vesper Vale: 57 team-games, win rate 53%

### Emblems per team-game by bucket

| Emblem | 1–10 | 11–30 | 31–60 | 61–100 |
|---|---|---|---|---|
| Bruiser Emblem of Strength | 13.00 | 12.92 | 11.83 | 13.76 |
| Bruiser Emblem of Health | 10.02 | 8.84 | 7.05 | 6.54 |
| Bruiser Emblem of Focus | 4.97 | 7.40 | 5.95 | 4.34 |
| Power Emblem of Strength | 3.59 | 2.37 | 3.96 | 4.47 |
| Durability Emblem of Health | 2.24 | 3.20 | 2.93 | 2.87 |
| Jungle Emblem of Strength | 1.66 | 1.53 | 2.01 | 1.56 |
| Power Emblem of Focus | 0.68 | 0.75 | 2.48 | 1.74 |
| Treasure Emblem of Strength | 1.21 | 1.60 | 1.38 | 1.36 |
| Treasure Emblem of Health | 1.40 | 1.03 | 1.14 | 1.46 |
| Power Emblem of Health | 0.07 | 0.51 | 1.21 | 1.68 |
| Jungle Emblem of Health | 0.75 | 0.74 | 0.52 | 1.03 |
| Jungle Emblem of Focus | 0.83 | 0.73 | 0.85 | 0.49 |
| Treasure Emblem of Focus | 0.55 | 1.04 | 0.71 | 0.47 |
| Monster Emblem of Strength | 0.58 | 0.50 | 0.57 | 0.49 |
| Durability Emblem of Strength | 0.46 | 0.13 | 0.15 | 0.21 |
| Monster Emblem of Health | 0.55 | 0.10 | 0.10 | 0.07 |
| Home Emblem of Health | 0.39 | 0.19 | 0.10 | 0.11 |
| Monster Emblem of Focus | 0.28 | 0.13 | 0.18 | 0.14 |
| Durability Emblem of Focus | 0.24 | 0.06 | 0.11 | 0.13 |
| Home Emblem of Focus | 0.10 | 0.00 | 0.05 | 0.03 |

### Per-team profile, ranks 1–30 (recent ranked)

| Rank | Team | n | WR | Gold@300 | Gems@1000 | K/D | Gems/tribute | Locks | Crystals | Fuel-ex | Main comp |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Intercontinental Fish | 65 | 55% | 1,603 | 468 | 10/11 | 50 | 4.1 | 4.9 | 0.0 | Cassi/Heart of Ruin/Issac/Ryth/Vendetta/Vesper Vale |
| 2 | Mite | 61 | 48% | 1,282 | 465 | 6.5/11 | 47 | 5.3 | 4.4 | 25 | Marcella/Ryth/Tavi/Vendetta/Vesper Vale |
| 3 | willow best girl | 67 | 52% | 1,501 | 432 | 9.9/5.3 | 33 | 7.7 | 0.0 | 4.7 | Heart of Ruin/Marcella/Prism/Vesper Vale/Willow |
| 4 | Trzy Kwaterki | 52 | 50% | 1,523 | 508 | 6.3/6.6 | 32 | 0.0 | 8.2 | 43 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 5 | Claudus Maximus | 62 | 53% | 1,474 | 523 | 6.4/11 | 38 | 8.0 | 0.0 | 0.0 | Heart of Ruin/Marcella/Tavi/Vendetta/Vesper Vale |
| 6 | policy wonk | 67 | 57% | 1,508 | 488 | 6.2/8.3 | 54 | 4.8 | 7.5 | 0.0 | Heart of Ruin/Marcella/Tavi/Vesper Vale/Willow |
| 7 | Sponge(Albert and Bob) | 55 | 49% | 1,282 | 348 | 11/8.4 | 53 | 7.3 | 20 | 13 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 8 | prism best girl | 69 | 49% | 1,474 | 377 | 9.5/9.0 | 62 | 0.0 | 6.3 | 0.0 | Heart of Ruin/Marcella/Prism/Ryth/Vesper Vale |
| 9 | procless | 70 | 54% | 1,526 | 525 | 7.3/6.9 | 48 | 0.0 | 1.3 | 1.3 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 10 | :3 | 63 | 48% | 1,334 | 472 | 5.1/8.7 | 56 | 7.4 | 6.2 | 153 | Heart of Ruin/Marcella/Prism/Vendetta/Vesper Vale |
| 11 | new team(4) | 58 | 64% | 1,363 | 515 | 5.0/7.4 | 46 | 9.8 | 8.2 | 0.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 12 | hello | 55 | 51% | 1,192 | 497 | 6.3/6.5 | 50 | 4.6 | 0.0 | 2.4 | Marcella/Ryth/Tavi/Vendetta/Vesper Vale |
| 13 | Ad Astra | 67 | 51% | 1,446 | 471 | 10/5.3 | 63 | 4.3 | 0.0 | 0.3 | Heart of Ruin/Marcella/Ryth/Varg/Vesper Vale |
| 14 | ひななー | 70 | 41% | 1,219 | 458 | 6.5/8.7 | 47 | 0.0 | 2.5 | 1.4 | Heart of Ruin/Ryth/Vendetta/Vesper Vale/Willow |
| 15 | NEETCoders | 68 | 53% | 1,360 | 511 | 9.4/9.6 | 33 | 4.4 | 2.0 | 3.1 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 16 | Univardisty of Shingtonwash | 68 | 44% | 1,357 | 493 | 8.1/9.3 | 46 | 0.0 | 7.2 | 32 | Heart of Ruin/Stellar/Tavi/Vesper Vale/Willow |
| 17 | print dont int | 63 | 41% | 1,205 | 429 | 8.4/10 | 35 | 0.0 | 7.2 | 6.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 18 | Supaswag | 74 | 55% | 1,224 | 488 | 9.5/13 | 36 | 0.0 | 0.0 | 0.0 | Heart of Ruin/Marcella/Vendetta/Vesper Vale/Willow |
| 19 | Nameless | 67 | 60% | 1,575 | 399 | 10/6.6 | 43 | 6.4 | 7.4 | 4.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 20 | Tartarus | 70 | 56% | 1,337 | 501 | 12/6.4 | 46 | 4.7 | 1.6 | 12 | Heart of Ruin/Marcella/Ryth/Tavi/Vesper Vale |
| 21 | the ruliad | 77 | 57% | 1,424 | 622 | 6.6/6.9 | 39 | 7.9 | 1.4 | 0.1 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 22 | Zarzorki | 64 | 50% | 1,378 | 444 | 7.5/7.8 | 55 | 8.4 | 0.0 | 0.2 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 23 | Noobot | 62 | 44% | 1,374 | 480 | 5.7/6.2 | 50 | 7.5 | 8.4 | 1.9 | Marcella/Ryth/Tavi/Vesper Vale/Willow |
| 24 | 𓎼𓃭𓅱𓂋𓇌 𓏏𓅱 𓂋𓄿 | 65 | 52% | 1,326 | 483 | 8.4/12 | 36 | 10 | 2.9 | 0.2 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 25 | EMPEROR Tung | 61 | 52% | 1,304 | 498 | 5.9/8.7 | 49 | 6.3 | 14 | 11 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 26 | conqueror_of_openai | 64 | 55% | 1,307 | 492 | 8.0/11 | 41 | 5.4 | 10 | 1.7 | Marcella/Ryth/Varg/Vendetta/Vesper Vale |
| 27 | Nyx | 67 | 42% | 1,246 | 377 | 8.9/8.9 | 52 | 0.4 | 0.0 | 0.2 | Heart of Ruin/Marcella/Prism/Vendetta/Vesper Vale |
| 28 | Let's LARP again | 53 | 62% | 1,201 | 501 | 8.3/8.6 | 41 | 5.3 | 0.0 | 3.5 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 29 | subscribe to caterpillow | 60 | 52% | 1,344 | 562 | 3.1/12 | 37 | 9.0 | 0.5 | 0.0 | Heart of Ruin/Marcella/Ryth/Vendetta/Vesper Vale |
| 30 | FürFortnite | 68 | 43% | 1,112 | 503 | 9.9/11 | 44 | 0.6 | 6.4 | 6.7 | Cassi/Heart of Ruin/Issac/Marcella/Ryth/Vesper Vale |

### What separates winners from losers in top-100 ranked games

| Metric | Winner | Loser | Winner higher in |
|---|---|---|---|
| Gold @100 | 359 | 294 | 59% |
| Gold @300 | 1,385 | 1,026 | 73% |
| Gold @500 | 2,687 | 1,879 | 78% |
| Gold @1000 | 6,492 | 4,168 | 83% |
| Gems tributed @500 | 239 | 156 | 66% |
| Gems tributed @1000 | 570 | 335 | 82% |
| Gold earned (game) | 9,075 | 5,349 | 86% |
| Kills | 12 | 4.4 | 75% |
| Deaths | 5.7 | 14 | 17% |
| Camp clears | 63 | 48 | 71% |
| Gems per tribute | 52 | 34 | 67% |
| First tribute | 206 | 207 | 47% |
| Carried gems at end | 63 | 70 | 35% |
| Unspent gold | 596 | 229 | 77% |
| Altar locks | 4.8 | 4.5 | 44% |
| Crystals | 4.0 | 3.1 | 23% |
| Wards | 214 | 191 | 57% |
| Healing | 1,720 | 1,487 | 12% |
| Fuel-exhausted turns | 18 | 20 | 33% |
| Game length | 1,248 | 1,248 | 0% |

### P(leader wins), top-100 ranked

| Round | Gold-earned leader | Tributed-gems leader | Kill leader |
|---|---|---|---|
| 100 | 61% (n=3573) | 55% (n=2006) | 68% (n=1163) |
| 300 | 73% (n=3639) | 63% (n=3225) | 72% (n=2189) |
| 500 | 78% (n=3622) | 72% (n=3310) | 75% (n=2611) |
| 1000 | 83% (n=2695) | 86% (n=2588) | 78% (n=2340) |

### Deaths

| Group | Deaths/game | to champions | to camps | Gems lost on death/game | Deaths before round 300 | Median first death |
|---|---|---|---|---|---|---|
| v8 (scrims) | 13 | 12 | 1.1 | 123 | 2.0 | 139 |
| Scrim opponents | 8.0 | 6.7 | 1.4 | 115 | 1.5 | 172 |
| Top-100 ranked | 9.4 | 8.2 | 1.3 | 101 | 1.8 | 159 |
| Ranks 1–30 ranked | 8.8 | 7.9 | 0.9 | 140 | 1.7 | 162 |

### First three emblems by champion: v8 vs ranks 1–30 (ranked)

**Vesper Vale**

- v8 (n=448): Treasure Emblem of Strength → Bruiser Emblem of Strength → Durability Emblem of Health (100%)
- Ranks 1–30 (n=1544): Treasure Emblem of Strength → Jungle Emblem of Strength → Bruiser Emblem of Health (24%); Treasure Emblem of Strength → Power Emblem of Strength → Durability Emblem of Health (9%); Treasure Emblem of Strength → Jungle Emblem of Strength → Durability Emblem of Health (8%); Treasure Emblem of Strength → Bruiser Emblem of Strength → Durability Emblem of Health (7%)
- Emblem stat mix, v8: Strength 95%, Health 5%; ranks 1–30: Strength 73%, Health 27%
- Emblems completed per game, v8: 20; ranks 1–30: 14

**Marcella**

- v8 (n=330): Treasure Emblem of Health → Jungle Emblem of Health → Bruiser Emblem of Strength (100%)
- Ranks 1–30 (n=1387): Treasure Emblem of Health → Jungle Emblem of Strength → Bruiser Emblem of Health (15%); Bruiser Emblem of Strength → Bruiser Emblem of Health → Bruiser Emblem of Focus (8%); Treasure Emblem of Focus → Durability Emblem of Health → Bruiser Emblem of Focus (8%); Treasure Emblem of Focus → Jungle Emblem of Focus → Durability Emblem of Health (7%)
- Emblem stat mix, v8: Strength 65%, Health 35%; ranks 1–30: Focus 43%, Health 33%, Strength 23%
- Emblems completed per game, v8: 6.4; ranks 1–30: 12

**Vendetta**

- v8 (n=277): Treasure Emblem of Strength → Jungle Emblem of Strength → Bruiser Emblem of Strength (100%)
- Ranks 1–30 (n=987): Treasure Emblem of Health → Jungle Emblem of Strength → Bruiser Emblem of Health (32%); Treasure Emblem of Strength → Jungle Emblem of Strength → Durability Emblem of Strength (14%); Treasure Emblem of Strength → Jungle Emblem of Strength → Durability Emblem of Health (7%); Treasure Emblem of Strength → Jungle Emblem of Health → Monster Emblem of Strength (6%)
- Emblem stat mix, v8: Strength 91%, Health 9%; ranks 1–30: Strength 62%, Health 38%
- Emblems completed per game, v8: 6.5; ranks 1–30: 9.5

**Heart of Ruin**

- v8 (n=333): Treasure Emblem of Focus → Jungle Emblem of Focus → Bruiser Emblem of Focus (100%)
- Ranks 1–30 (n=1255): Treasure Emblem of Health → Jungle Emblem of Focus → Bruiser Emblem of Health (19%); Jungle Emblem of Focus → Treasure Emblem of Focus → Durability Emblem of Health (9%); Treasure Emblem of Health → Jungle Emblem of Focus → Durability Emblem of Health (7%); Treasure Emblem of Focus → Jungle Emblem of Focus → Bruiser Emblem of Focus (5%)
- Emblem stat mix, v8: Focus 89%, Health 11%; ranks 1–30: Focus 61%, Health 36%, Strength 3%
- Emblems completed per game, v8: 5.2; ranks 1–30: 8.1

**Ryth**

- v8 (n=203): Treasure Emblem of Health → Jungle Emblem of Health → Bruiser Emblem of Health (100%)
- Ranks 1–30 (n=800): Treasure Emblem of Health → Jungle Emblem of Strength → Bruiser Emblem of Health (43%); Jungle Emblem of Health → Treasure Emblem of Health → Durability Emblem of Health (7%); Treasure Emblem of Strength → Jungle Emblem of Health → Bruiser Emblem of Strength (6%); Treasure Emblem of Health → Durability Emblem of Health → Bruiser Emblem of Health (4%)
- Emblem stat mix, v8: Health 100%; ranks 1–30: Health 52%, Strength 47%, Focus 0%
- Emblems completed per game, v8: 4.9; ranks 1–30: 5.8

### Teleport crystals in top-100 ranked games

- Teams buying at least one: 28% of team-games; 3.6 per team-game (ranks 1–30: 4.5).
- Bought by: Marcella 23%, Vesper Vale 19%, Heart of Ruin 16%, Vendetta 15%, Ryth 14%, Varg 4%
- Purchase round: median 523, quartiles 258–852
- Win rate with crystals 52% (n=1792), without 50% (n=4676)
- v8 crystals bought in 485 scrims: 0
