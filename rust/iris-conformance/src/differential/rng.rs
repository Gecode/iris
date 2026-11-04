//! The SplitMix64 generator that derives each differential case from its seed
//! and index alone.

/// SplitMix64: small, fast, and good enough to drive a fuzzer; its state is
/// the whole of a case's randomness, so equal seeds give equal cases.
pub(crate) struct Rng {
    state: u64,
}

const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

impl Rng {
    /// The generator of case `index` in the run with `seed`.
    pub(crate) fn for_case(seed: u64, index: u64) -> Self {
        let mut seeded = Self { state: seed };
        Self {
            state: seeded.next_u64() ^ index.wrapping_mul(GOLDEN_GAMMA),
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// A number in `0..bound`; `bound` must be positive.
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    /// A number in `low..=high`.
    pub(crate) fn between(&mut self, low: i64, high: i64) -> i64 {
        let span = high.abs_diff(low).wrapping_add(1);
        if span == 0 {
            return self.next_u64() as i64;
        }
        low.wrapping_add((self.next_u64() % span) as i64)
    }

    /// True with probability `percent` / 100.
    pub(crate) fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }

    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}
