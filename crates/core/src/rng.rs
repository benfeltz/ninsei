//! Small seeded RNG (SplitMix64). Deterministic on every platform, so a seed replays a run exactly.

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        debug_assert!(lo <= hi);
        let span = u64::from(hi - lo) + 1;
        lo + (self.next_u64() % span) as u32
    }

    pub fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        (self.next_u64() % n as u64) as usize
    }

    /// True with `pct` percent probability.
    pub fn chance(&mut self, pct: u32) -> bool {
        self.range(1, 100) <= pct
    }

    /// Index of an item picked by weight. Returns `None` if every weight is zero.
    pub fn weighted(&mut self, weights: impl IntoIterator<Item = u32> + Clone) -> Option<usize> {
        let total: u64 = weights.clone().into_iter().map(u64::from).sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.next_u64() % total;
        for (i, w) in weights.into_iter().enumerate() {
            let w = u64::from(w);
            if roll < w {
                return Some(i);
            }
            roll -= w;
        }
        None
    }
}
