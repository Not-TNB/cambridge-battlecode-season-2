// Build a .map26a from an ASCII layout using the official map editor's engine wasm.
// Usage: node mapgen.mjs layout.txt out.map26a [champs]
// A freshly created map is rejected by the game (engine error 7), so we start from an existing
// map of the same size (from maps/), erase everything on it and draw the layout. Its symmetry
// (and colours) are kept; the builder mirrors team B and every tile we add by that symmetry.
// Glyphs: . ground, # wall, o hole, " bush, @ altar, A team A spawn (one rectangle),
// w wolf, e elk, G gem cluster, $ gold cluster, M monolith. Lines starting with ; are comments.
import fs from 'fs'; import path from 'path'; import { WASI } from 'wasi';
const [,, layoutPath, outPath, champs] = process.argv;
const MAPS = new URL('../../maps', import.meta.url).pathname;
const rows = fs.readFileSync(layoutPath, 'utf8').split('\n').filter(l => l.length && !l.startsWith(';'));
const H = rows.length, W = Math.max(...rows.map(r => r.length));
const wasi = new WASI({ version: 'preview1' });
const inst = new WebAssembly.Instance(new WebAssembly.Module(fs.readFileSync(new URL('./engine.wasm', import.meta.url))), wasi.getImportObject());
wasi.initialize(inst); const ex = inst.exports; const out = ex.Engine_Malloc(64);
const mem = () => new DataView(ex.memory.buffer);
const cstr = p => { const b = new Uint8Array(ex.memory.buffer); let s = ''; while (b[p]) s += String.fromCharCode(b[p++]); return s; };
const check = (c, w) => { if (c) throw new Error(`${w}: engine error ${c}: ${cstr(ex.GetLastErrorMsg())}`); };
const load = file => {
  const b = fs.readFileSync(file); const p = ex.Engine_Malloc(b.length);
  new Uint8Array(ex.memory.buffer, p, b.length).set(b);
  check(ex.MapBuilder_FromFileBytes(p, b.length, out), 'load ' + file);
  return mem().getBigUint64(out, true);
};
const size = file => { const b = fs.readFileSync(file); return [b.readUInt16LE(0x4a), b.readUInt16LE(0x4c)]; };
const base = fs.readdirSync(MAPS).filter(f => f.endsWith('.map26a') && !/-[0-9a-f]{8}-/.test(f))
  .map(f => path.join(MAPS, f)).find(f => { const [w, h] = size(f); return w === W && h === H; });
if (!base) throw new Error(`no map in maps/ is ${W}x${H}; pick a size an existing map uses`);
const h = load(base);
check(ex.MapBuilder_GetSymmetry(h, out), 'sym'); const SYM = mem().getInt32(out, true);
// Erase: camps first (from the end), then every tile feature.
check(ex.MapBuilder_GetNumCamps(h, out), 'ncamps');
for (let i = mem().getUint32(out, true) - 1; i >= 0; i--) {
  check(ex.MapBuilder_GetCampHandle(h, i, out), 'camp handle');
  check(ex.MapBuilder_RemoveCamp(h, mem().getBigUint64(out, true)), 'remove camp');
}
for (let y = 0; y < H; y++) for (let x = 0; x < W; x++)
  for (const f of ['Wall', 'Hole', 'Bush', 'Altar']) ex[`MapBuilder_Remove${f}`](h, x, y);
if (champs) check(ex.MapBuilder_SetNumChampsPerTeam(h, Number(champs)), 'champs');
let spawn = null;
for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) if ((rows[y][x] ?? '.') === 'A')
  spawn = spawn ? { a: Math.min(spawn.a, x), b: Math.min(spawn.b, y), c: Math.max(spawn.c, x), d: Math.max(spawn.d, y) } : { a: x, b: y, c: x, d: y };
if (!spawn) throw new Error('no A spawn');
// The spawn rectangle's max corner is exclusive (a 3x3 spawn at the origin is 0,0,3,3).
[spawn.a, spawn.b, spawn.c + 1, spawn.d + 1].forEach((v, i) => mem().setInt32(out + 4 * i, v, true));
check(ex.MapBuilder_SetTeamASpawnArea(h, out), 'spawn');
const CAMP = { w: 0, e: 1, G: 2, $: 3, M: 4 };
const mirror = (x, y) => SYM === 2 ? [W - 1 - x, H - 1 - y] : SYM === 0 ? [W - 1 - x, y] : [x, H - 1 - y];
const done = new Set();
for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
  const c = rows[y][x] ?? '.', key = x + ',' + y;
  if (done.has(key)) continue;
  const op = c === '#' ? ['AddWall'] : c === 'o' ? ['AddHole'] : c === '"' ? ['AddBush'] : c === '@' ? ['AddAltar'] : null;
  if (op) check(ex[`MapBuilder_${op[0]}`](h, x, y), `${op[0]} ${key}`);
  else if (c in CAMP) check(ex.MapBuilder_AddCamp(h, CAMP[c], x, y, out), `camp ${c} ${key}`);
  else continue;
  const [mx, my] = mirror(x, y); done.add(mx + ',' + my);
}
check(ex.MapBuilder_VerifyIntegrity(h), 'verify');
check(ex.MapBuilder_Encode(h, out, out + 4), 'encode');
fs.writeFileSync(outPath, Buffer.from(new Uint8Array(ex.memory.buffer, mem().getUint32(out, true), mem().getUint32(out + 4, true))));
console.log(`wrote ${outPath} ${W}x${H} from ${path.basename(base)} (symmetry ${['horizontal', 'vertical', 'rotational'][SYM]})`);
