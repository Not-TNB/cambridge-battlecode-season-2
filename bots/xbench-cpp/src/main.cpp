// Fuel benchmark (2026-10-25): the same loops as bots/xbench-rs, in C++, timed in fuel.
#include <cambc.h>
#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <utility>
#include <vector>

using cambc::vec2;

static std::uint64_t fuel() { return cambc::fuel_remaining(); }

static void report(const char* name, std::uint64_t spent, std::size_t units) {
    std::printf("BENCH cpp %s fuel %llu units %zu per %.1f\n", name, (unsigned long long)spent, units,
                double(spent) / double(units ? units : 1));
}

static std::vector<std::pair<int, std::uint32_t>> memo;
volatile long long sink;

static void bench(int step) {
    const int w = cambc::map::width(), h = cambc::map::height();
    const auto& walls = cambc::map::walls();
    const auto blocked = (walls | cambc::map::holes()).positions();
    std::vector<vec2> camps;
    for (const auto& group : cambc::map::camp_groups())
        for (const auto& camp : group) camps.push_back(camp.spawn_position());
    const vec2 here = cambc::self().position();

    std::uint64_t a = 0, b = 0;
    if (step == 0) {
    a = fuel();
    b = fuel();
    report("fuel-read", a - b, 1);
    }
    if (step == 1) {

    // B1: flood fill over a padded grid.
    a = fuel();
    const long width = w + 2;
    const std::size_t size = std::size_t(w + 2) * std::size_t(h + 2);
    auto at = [&](vec2 p) { return std::size_t((p.y + 1) * width + (p.x + 1)); };
    std::vector<std::uint8_t> cell(size, 0);
    for (std::size_t x = 0; x < std::size_t(width); ++x) { cell[x] = 1; cell[size - 1 - x] = 1; }
    for (std::size_t y = 0; y < std::size_t(h + 2); ++y) { cell[y * width] = 1; cell[y * width + width - 1] = 1; }
    for (auto p : blocked) cell[at(p)] = 1;
    b = fuel();
    report("grid-init", a - b, size);
    a = fuel();
    std::vector<std::uint16_t> dist(size, 0);
    std::vector<std::uint32_t> queue;
    queue.reserve(size);
    const auto start = at(here);
    cell[start] = 2;
    queue.push_back(std::uint32_t(start));
    const long offsets[8] = { -width - 1, -width, -width + 1, -1, 1, width - 1, width, width + 1 };
    std::size_t head = 0;
    while (head < queue.size() && head < 100) {
        const long i = queue[head++];
        const auto d = dist[i];
        for (long off : offsets) {
            const auto j = std::size_t(i + off);
            if (cell[j] == 0) { cell[j] = 2; dist[j] = d + 1; queue.push_back(std::uint32_t(j)); }
        }
    }
    sink = dist[queue.back()];
    b = fuel();
    report("flood-pop", a - b, head);
    sink = (long long)(head);
    }
    if (step == 2) {

    // B2: nearest candidate after a cooldown filter.
    const std::pair<vec2, std::uint32_t> abandoned[3] = { { vec2{ 1, 1 }, 5 }, { vec2{ 2, 3 }, 5 }, { vec2{ 7, 7 }, 5 } };
    a = fuel();
    int found = 0;
    for (int r = 0; r < 20; ++r) {
        std::vector<vec2> available;
        for (auto t : camps)
            if (std::none_of(std::begin(abandoned), std::end(abandoned), [&](const auto& o) { return o.first == t; }))
                available.push_back(t);
        auto best = std::min_element(available.begin(), available.end(),
            [&](vec2 x, vec2 y) { return x.chebyshev_distance(here) < y.chebyshev_distance(here); });
        if (best != available.end()) found += best->x;
    }
    b = fuel();
    report("nearest-tile", a - b, 20 * camps.size());
    sink = (long long)(found);
    }
    if (step == 3) {

    // B3: host bitboard lookups.
    a = fuel();
    int n = 0;
    for (int i = 0; i < 200; ++i) n += walls.is_set(vec2{ i % w, (i / w) % h }) ? 1 : 0;
    b = fuel();
    report("is-set", a - b, 200);
    sink = (long long)(n);
    }
    if (step == 4) {

    // B4: linear find in a list of (group, tile).
    std::vector<std::pair<std::size_t, vec2>> live;
    for (std::size_t i = 0; i < camps.size() && i < 18; ++i) live.push_back({ i, camps[i] });
    a = fuel();
    int hits = 0;
    for (std::size_t q = 0; q < camps.size() && q < 30; ++q)
        if (std::any_of(live.begin(), live.end(), [&](const auto& e) { return e.second == camps[q]; })) ++hits;
    b = fuel();
    report("find-18", a - b, std::min<std::size_t>(30, camps.size()));
    sink = (long long)(hits);
    }
    if (step == 5) {

    // B5: memo lookups (a global vector; C++ has no thread-local cost here).
    memo.clear();
    for (int i = 0; i < 8; ++i) memo.push_back({ i, std::uint32_t(i * 3) });
    a = fuel();
    std::uint32_t sum = 0;
    for (int i = 0; i < 200; ++i) {
        auto it = std::find_if(memo.begin(), memo.end(), [&](const auto& e) { return e.first == i % 8; });
        sum += it == memo.end() ? 0 : it->second;
    }
    b = fuel();
    report("tls-memo", a - b, 200);
    sink = (long long)(sum);
    }
}

int main() {
    int step = 0;
    while (true) {
        try {
            if (cambc::is_draft()) {
                const auto mine = cambc::draft::current_draft(cambc::team::ally);
                for (int id : { 0, 1, 2, 6, 11, 10, 9 }) {
                    auto pick = static_cast<cambc::champion_draft>(id);
                    if (std::find(mine.begin(), mine.end(), pick) == mine.end()) { cambc::draft::pick(pick); break; }
                }
            } else if (step < 6) {
                bench(step++);
            }
        } catch (const std::exception& e) {
            std::printf("bench error: %s\n", e.what());
        }
        cambc::end_turn();
    }
}
