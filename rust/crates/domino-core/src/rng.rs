//! Seeded PRNG (SplitMix64) powering shuffles.
//!
//! The TypeScript engine relies on `Math.random()`; for the Rust port, shuffles
//! are funneled through this tiny dependency-free generator so that every random
//! path (`generate_tiles`, `start_round`, `create_game`, `next_round`) can be
//! reproduced bit-for-bit in tests via an explicit seed, while production code
//! can seed from OS entropy.

/// A cheap SplitMix64 PRNG (only used for tile shuffles).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRng {
    state: u64,
}

impl GameRng {
    /// Build a reproducible generator from a fixed seed.
    pub fn from_seed(seed: u64) -> Self {
        GameRng { state: seed }
    }

    /// Seed from the operating system (Web Crypto in browser builds).
    /// Fail explicitly if secure entropy is unavailable instead of dealing
    /// predictable hands from the wall clock.
    pub fn from_entropy() -> Self {
        let mut seed = [0u8; 8];
        getrandom::getrandom(&mut seed).expect("secure randomness required to deal dominoes");
        Self::from_seed(u64::from_le_bytes(seed))
    }

    /// Next raw 64-bit state.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `[0, n)` using rejection sampling to avoid modulo bias.
    #[must_use]
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "random bound must be positive");
        let threshold = n.wrapping_neg() % n;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return value % n;
            }
        }
    }

    /// In-place Fisher–Yates shuffle, iterating `len-1 → 1` exactly like the
    /// TS implementation.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below((i + 1) as u64) as usize;
            items.swap(i, j);
        }
    }
}
