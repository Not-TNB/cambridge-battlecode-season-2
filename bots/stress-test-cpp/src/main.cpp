// A* fuel stress test (C++). Bots never move: each turn they run random A* searches
// until fuel is nearly gone, then log a summary. Call stress::play_turn() from your
// per-turn entry point (before bc::end_turn()).
//
// Fuel exhaustion does NOT crash a bot: it is suspended and resumes next round with a fresh
// budget. So A* checks fuel every CHECK_EVERY node expansions and aborts before the budget runs
// out, which keeps every turn's numbers clean. If a search is suspended anyway, it is flagged.
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <functional>
#include <queue>
#include <vector>
#include <cambc.h>   // your SDK header(s)

namespace bc = cambc;

namespace stress {

constexpr std::uint64_t RESERVE = 10'000;  // fuel kept back for the final summary print
constexpr std::uint32_t CHECK_EVERY = 8;   // node expansions between in-search fuel checks
constexpr std::uint64_t MARGIN = 2;        // abort when fuel < RESERVE + MARGIN * worst fuel burned between checks
constexpr bool LOG_EVERY_SEARCH = false;    // one line per search (costs fuel itself)
constexpr std::uint64_t SEED = 12345;
constexpr int DX[8] = {-1, 0, 1, -1, 1, -1, 0, 1};
constexpr int DY[8] = {-1, -1, -1, 0, 0, 1, 1, 1};

enum class status { ok, nopath, abort_ };
static const char* status_name(status s) { return s == status::ok ? "ok" : s == status::nopath ? "nopath" : "ABORT"; }

struct result { std::int64_t len; std::uint32_t nodes; status st; };

static bool ready = false;
static int W = 0, H = 0;
static std::vector<std::uint8_t> blocked;
static std::vector<std::uint32_t> walkable, parent, seen;
static std::vector<std::uint32_t> g;
static std::uint32_t stamp = 0;
static std::uint64_t rng = SEED, worst = 0, searches = 0, completed = 0, aborted_total = 0, turn = 0, summary_cost = 0;
static volatile std::int64_t sink = 0;  // keeps the optimizer from deleting the searches

static std::size_t rnd(std::size_t limit) {
    rng = rng * 6364136223846793005ULL + 1442695040888963407ULL;
    return static_cast<std::size_t>((rng >> 33) % limit);
}

static void setup() {
    W = bc::map::width();
    H = bc::map::height();
    auto mask = bc::map::walls() | bc::map::holes() | bc::map::altars();
    const std::size_t n = static_cast<std::size_t>(W) * H;
    // export_linear_buffer() can come back bit-packed, so read tiles with is_set instead.
    blocked.assign(n, 0);
    for (int y = 0; y < H; ++y)
        for (int x = 0; x < W; ++x)
            if (mask.is_set(bc::vec2(x, y))) blocked[static_cast<std::size_t>(y) * W + x] = 1;
    walkable.clear();
    for (std::uint32_t i = 0; i < n; ++i)
        if (!blocked[i]) walkable.push_back(i);
    g.assign(n, 0);
    parent.assign(n, 0);
    seen.assign(n, 0);
    ready = true;
}

// 8-directional A*, uniform cost, Chebyshev heuristic. f0 = fuel when the search starts.
static result astar(std::uint32_t s, std::uint32_t t, std::uint64_t f0) {
    ++stamp;
    const int tx = t % W, ty = t / W, sx = s % W, sy = s / W;
    const std::uint64_t h0 = std::max(std::abs(sx - tx), std::abs(sy - ty));
    g[s] = 0;
    seen[s] = stamp;
    parent[s] = s;
    std::priority_queue<std::uint64_t, std::vector<std::uint64_t>, std::greater<>> heap;
    heap.push((h0 << 22) | (h0 << 14) | s);  // key = f<<22 | h<<14 | idx
    std::uint32_t nodes = 0;
    std::uint64_t last = f0;
    while (!heap.empty()) {
        const std::uint64_t key = heap.top();
        heap.pop();
        const std::uint32_t idx = key & 0x3FFF;
        const std::uint64_t hh = (key >> 14) & 0xFF;
        const std::uint32_t gc = static_cast<std::uint32_t>((key >> 22) - hh);
        if (gc > g[idx]) continue;  // stale entry
        ++nodes;
        if (idx == t) {
            std::int64_t steps = 0;
            for (std::uint32_t cur = t; cur != s; cur = parent[cur]) ++steps;
            return {steps, nodes, status::ok};
        }
        if (nodes % CHECK_EVERY == 0) {
            const std::uint64_t f = bc::fuel_remaining();
            if (f <= last && last - f > worst) worst = last - f;
            last = f;
            if (f < RESERVE + MARGIN * worst) return {-1, nodes, status::abort_};
        }
        const int x = idx % W, y = idx / W;
        const std::uint32_t ng = gc + 1;
        for (int d = 0; d < 8; ++d) {
            const int nx = x + DX[d], ny = y + DY[d];
            if (nx < 0 || ny < 0 || nx >= W || ny >= H) continue;
            const std::uint32_t ni = ny * W + nx;
            if (blocked[ni]) continue;
            if (seen[ni] == stamp && g[ni] <= ng) continue;
            seen[ni] = stamp;
            g[ni] = ng;
            parent[ni] = idx;
            const std::uint64_t nh = std::max(std::abs(nx - tx), std::abs(ny - ty));
            heap.push(((ng + nh) << 22) | (nh << 14) | ni);
        }
    }
    return {-1, nodes, status::nopath};
}

#define U(x) static_cast<unsigned long long>(x)

inline void play_turn() {
    ++turn;
    const std::uint64_t turn_start = bc::fuel_remaining();
    if (!ready) {
        setup();
        const std::uint64_t after = bc::fuel_remaining();
        std::printf("[stress] setup %dx%d, %zu walkable, setup fuel cost %llu (turn started with %llu)\n", W, H,
                    walkable.size(), U(turn_start - after), U(turn_start));
    }
    const std::uint64_t loop_start = bc::fuel_remaining();

    std::uint64_t done = 0, nopath = 0, aborted = 0, suspended = 0;
    std::uint64_t cost_sum = 0, cost_min = 0, cost_max = 0, nodes_sum = 0, wasted = 0;
    for (;;) {
        const std::uint32_t a = walkable[rnd(walkable.size())];
        const std::uint32_t b = walkable[rnd(walkable.size())];
        const std::uint64_t f0 = bc::fuel_remaining();
        if (f0 < RESERVE + MARGIN * worst) break;
        ++searches;
        const result r = astar(a, b, f0);
        const std::uint64_t f1 = bc::fuel_remaining();
        if (f1 > f0) {  // fuel went UP: suspended and refilled mid-search
            ++suspended;
            if (LOG_EVERY_SEARCH)
                std::printf("search %llu: fuel_before=%llu SUSPENDED mid-search (cost unknown) nodes=%u\n", U(searches), U(f0), r.nodes);
            continue;
        }
        const std::uint64_t cost = f0 - f1;
        if (r.st == status::abort_) {
            ++aborted;
            ++aborted_total;
            wasted += cost;
        } else {
            ++done;
            ++completed;
            if (r.st == status::nopath) ++nopath;
            cost_sum += cost;
            nodes_sum += r.nodes;
            if (done == 1 || cost < cost_min) cost_min = cost;
            if (cost > cost_max) cost_max = cost;
            sink = sink + r.len;
        }
        if (LOG_EVERY_SEARCH)
            std::printf("search %llu: fuel_before=%llu cost=%llu nodes=%u len=%lld %s\n", U(searches), U(f0), U(cost),
                        r.nodes, (long long)r.len, status_name(r.st));
    }

    const std::uint64_t end = bc::fuel_remaining();
    const std::uint64_t avg = done ? cost_sum / done : 0;
    const std::uint64_t per_node = nodes_sum ? cost_sum / nodes_sum : 0;
    const std::uint64_t attempts = done + aborted + suspended;
    const std::uint64_t all_in = (attempts && end <= loop_start) ? (loop_start - end) / attempts : 0;
    const char* flag = (suspended || end > loop_start) ? " !! SUSPENDED: turn spanned rounds, totals invalid" : "";
    const std::uint64_t before = bc::fuel_remaining();
    std::printf(
        "[stress] turn %llu: completed %llu (%llu unreachable), aborted %llu (wasted %llu), total completed %llu, "
        "total aborted %llu | fuel %llu -> %llu | A*-only cost avg %llu min %llu max %llu, %llu/node | "
        "all-in avg %llu/attempt incl. logging | worst interval %llu, prev summary cost %llu, checksum %lld%s\n",
        U(turn), U(done), U(nopath), U(aborted), U(wasted), U(completed), U(aborted_total), U(loop_start), U(end),
        U(avg), U(cost_min), U(cost_max), U(per_node), U(all_in), U(worst), U(summary_cost), (long long)sink, flag);
    summary_cost = before - bc::fuel_remaining();
}

}  // namespace stress

// Entry point: run the stress test every turn. If your project already has a main() elsewhere,
// compile with -DSTRESS_NO_MAIN and call stress::play_turn() from it instead.
#ifndef STRESS_NO_MAIN
int main() {
    for (;;) {
        if (bc::is_draft()) {
            // replace with your draft logic
        } else {
            stress::play_turn();
        }
        bc::end_turn();
    }
}
#endif