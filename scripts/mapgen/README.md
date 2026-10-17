# Map generator

Builds `.map26a` maps from ASCII layouts using the official map editor's engine (wasm), so test
maps can live in version control as text.

    scripts/mapgen/fetch-engine.sh                      # once: saves engine.wasm next to this file
    node scripts/mapgen/mapgen.mjs scripts/mapgen/layouts/spokes.txt maps/spokes.map26a 5

Layout glyphs: `.` ground, `#` wall, `o` hole, `"` bush, `@` altar, `A` team A spawn (one rectangle,
at least 3x3), `w` wolf, `e` elk, `G` gem cluster, `$` gold cluster, `M` monolith. Lines starting
with `;` are comments. Draw team A's side; team B and every placed tile are mirrored by symmetry.

A map created from scratch with the builder is rejected by the game (engine error 7), so the script
starts from a map in `maps/` with the same width and height, erases it and draws the layout. That map's
symmetry is kept, so the layout's size picks the symmetry: 41x30 (moat) is horizontal, 60x50
(orchard) vertical, 30x30, 45x30 and 30x80 rotational. Adjacent camps form one group.

Gotchas found while writing it: the spawn rectangle's max corner is exclusive in the builder API,
and the game wants a spawn of at least 9 tiles.
