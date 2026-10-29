// Small helpers shared by every module.
use cambc::Vec2;

pub const DIRS: [Vec2; 8] = [
    Vec2::NORTH,
    Vec2::NORTH_EAST,
    Vec2::EAST,
    Vec2::SOUTH_EAST,
    Vec2::SOUTH,
    Vec2::SOUTH_WEST,
    Vec2::WEST,
    Vec2::NORTH_WEST,
];

pub fn cheb(a: Vec2, b: Vec2) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

pub fn fuel() -> u64 {
    cambc::fuel_remaining().unwrap_or(0)
}

pub fn round() -> i32 {
    cambc::round_number().unwrap_or(0) as i32
}

// Deterministic per-champion randomness (WASI randomness is always zero in matches).
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9e37_79b9_7f4a_7c15)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u32) -> u32 {
        (self.next() % u64::from(n.max(1))) as u32
    }
}
