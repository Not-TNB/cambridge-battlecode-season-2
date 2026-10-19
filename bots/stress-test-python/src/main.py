# A* fuel stress test (Python). Bots never move: each turn they run random A* searches
# until fuel is nearly gone, log a summary, and end the turn.
#
# Fuel exhaustion does NOT crash a bot: it is suspended and resumes next round with a fresh
# budget. So A* checks fuel every CHECK_EVERY node expansions and aborts before the budget runs
# out, which keeps every turn's numbers clean. If a search is suspended anyway, it is flagged.
import heapq
import cambc as bc

RESERVE = 50_000          # fuel kept back for the final summary print (Python prints are costly)
CHECK_EVERY = 8           # node expansions between in-search fuel checks (power of two)
MARGIN = 2                # abort when fuel < RESERVE + MARGIN * worst fuel burned between checks
LOG_EVERY_SEARCH = False   # one line per search (costs fuel itself)
SEED = 12345

CHECK_MASK = CHECK_EVERY - 1
MASK64 = (1 << 64) - 1
DIRS = ((-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1))
OK, NOPATH, ABORT = 0, 1, 2
STATUS = ("ok", "nopath", "ABORT")

_rng = SEED
_ready = False
_w = _h = 0
_blocked = b""
_walkable = []
_g = []
_parent = []
_seen = []
_stamp = 0
_worst = 0                # worst fuel burned over one CHECK_EVERY interval so far
_searches = 0             # every attempted search, completed or aborted
_completed = 0
_aborted = 0
_turn = 0
_sink = 0
_summary_cost = 0         # fuel the previous summary print cost, to help tune RESERVE


def rnd(limit):
    global _rng
    _rng = (_rng * 6364136223846793005 + 1442695040888963407) & MASK64
    return (_rng >> 33) % limit


def setup():
    global _ready, _w, _h, _blocked, _walkable, _g, _parent, _seen
    _w = bc.game_map.width()
    _h = bc.game_map.height()
    mask = bc.game_map.walls() | bc.game_map.holes() | bc.game_map.altars()
    n = _w * _h
    # export_linear_buffer() can come back bit-packed, so read tiles with is_set instead.
    _blocked = bytearray(n)
    for y in range(_h):
        for x in range(_w):
            if mask.is_set(bc.Vec2(x, y)):
                _blocked[y * _w + x] = 1
    _walkable = [i for i in range(n) if not _blocked[i]]
    _g = [0] * n
    _parent = [0] * n
    _seen = [0] * n
    _ready = True


def astar(s, t, f0):
    """8-directional A*, uniform cost, Chebyshev heuristic.
    Returns (path_length, nodes_expanded, status). f0 = fuel when the search starts."""
    global _stamp, _worst
    _stamp += 1
    stamp = _stamp
    w, h = _w, _h
    blocked, g, parent, seen = _blocked, _g, _parent, _seen
    fuel = bc.fuel_remaining
    tx, ty = t % w, t // w
    sx, sy = s % w, s // w
    h0 = max(abs(sx - tx), abs(sy - ty))
    g[s] = 0
    seen[s] = stamp
    parent[s] = s
    heap = [(h0 << 22) | (h0 << 14) | s]            # key = f<<22 | h<<14 | idx
    push, pop = heapq.heappush, heapq.heappop
    nodes = 0
    last = f0
    while heap:
        key = pop(heap)
        idx = key & 0x3FFF
        hh = (key >> 14) & 0xFF
        gc = (key >> 22) - hh
        if gc > g[idx]:
            continue                                  # stale entry
        nodes += 1
        if idx == t:
            steps = 0
            cur = t
            while cur != s:                           # walk the parent chain
                cur = parent[cur]
                steps += 1
            return steps, nodes, OK
        if not (nodes & CHECK_MASK):
            f = fuel()
            if f <= last and last - f > _worst:
                _worst = last - f
            last = f
            if f < RESERVE + MARGIN * _worst:
                return -1, nodes, ABORT
        x = idx % w
        y = idx // w
        ng = gc + 1
        for dx, dy in DIRS:
            nx = x + dx
            ny = y + dy
            if nx < 0 or ny < 0 or nx >= w or ny >= h:
                continue
            ni = ny * w + nx
            if blocked[ni]:
                continue
            if seen[ni] == stamp and g[ni] <= ng:
                continue
            seen[ni] = stamp
            g[ni] = ng
            parent[ni] = idx
            nh = max(abs(nx - tx), abs(ny - ty))
            push(heap, ((ng + nh) << 22) | (nh << 14) | ni)
    return -1, nodes, NOPATH


def stress_turn():
    global _searches, _completed, _aborted, _turn, _sink, _summary_cost
    fuel = bc.fuel_remaining
    _turn += 1
    turn_start = fuel()
    if not _ready:
        setup()
        print(f"[stress] setup {_w}x{_h}, {len(_walkable)} walkable, "
              f"setup fuel cost {turn_start - fuel()} (turn started with {turn_start})")
    loop_start = fuel()

    done = nopath = aborted = suspended = 0
    cost_sum = cost_min = cost_max = nodes_sum = wasted = 0
    while True:
        a = _walkable[rnd(len(_walkable))]
        b = _walkable[rnd(len(_walkable))]
        f0 = fuel()
        if f0 < RESERVE + MARGIN * _worst:
            break
        _searches += 1
        length, nodes, status = astar(a, b, f0)
        f1 = fuel()
        cost = f0 - f1
        if f1 > f0:                                   # fuel went UP: suspended and refilled mid-search
            suspended += 1
            if LOG_EVERY_SEARCH:
                print(f"search {_searches}: fuel_before={f0} SUSPENDED mid-search (cost unknown) nodes={nodes}")
            continue
        if status == ABORT:
            aborted += 1
            _aborted += 1
            wasted += cost
        else:
            done += 1
            _completed += 1
            if status == NOPATH:
                nopath += 1
            cost_sum += cost
            nodes_sum += nodes
            if done == 1 or cost < cost_min:
                cost_min = cost
            if cost > cost_max:
                cost_max = cost
            _sink += length
        if LOG_EVERY_SEARCH:
            print(f"search {_searches}: fuel_before={f0} cost={cost} nodes={nodes} "
                  f"len={length} {STATUS[status]}")

    end = fuel()
    avg = cost_sum // done if done else 0
    per_node = cost_sum // nodes_sum if nodes_sum else 0
    attempts = done + aborted + suspended
    all_in = (loop_start - end) // attempts if attempts and end <= loop_start else 0
    flag = " !! SUSPENDED: turn spanned rounds, totals invalid" if (suspended or end > loop_start) else ""
    before = fuel()
    print(f"[stress] turn {_turn}: completed {done} ({nopath} unreachable), aborted {aborted} "
          f"(wasted {wasted}), total completed {_completed}, total aborted {_aborted} | "
          f"fuel {loop_start} -> {end} | A*-only cost avg {avg} min {cost_min} max {cost_max}, "
          f"{per_node}/node | all-in avg {all_in}/attempt incl. logging | "
          f"worst interval {_worst}, prev summary cost {_summary_cost}, checksum {_sink}{flag}")
    _summary_cost = before - fuel()


def main():
    while True:
        if bc.is_draft():
            pass                      # replace with your draft logic
        else:
            stress_turn()
        bc.end_turn()
    
if __name__ == "__main__":
    main()