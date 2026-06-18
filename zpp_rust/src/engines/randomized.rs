//! Randomized multi-start engine.
//! Probabilistic exploration: shuffle the indices, greedily fill,
//! and check whether we hit the target.  Useful as a low-cost
//! backstop for inputs that defeat all deterministic strategies.

use num_bigint::BigUint;

use crate::controller::{Engine, Shared};

pub struct RandomizedEngine;

// Tiny xorshift64* PRNG — no external dep.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x9E3779B97F4A7C15 } else { seed })
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn range(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() as usize) % n
        }
    }
}

impl Engine for RandomizedEngine {
    fn name(&self) -> &'static str {
        "Randomized"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        let mut rng = Rng::new(0xCAFEBABE);
        let mut indices: Vec<usize> = (0..p.n).collect();

        for _ in 0..5000 {
            if sh.stopped() {
                return;
            }
            for i in (1..p.n).rev() {
                let j = rng.range(i + 1);
                indices.swap(i, j);
            }
            let mut picked: Vec<BigUint> = Vec::new();
            let mut total = BigUint::from(0u32);
            for &i in &indices {
                let cand = &total + &p.numbers[i];
                if cand <= p.target {
                    picked.push(p.numbers[i].clone());
                    total = cand;
                    if total == p.target {
                        sh.report(picked, "Randomized");
                        return;
                    }
                }
            }
        }
    }
}
