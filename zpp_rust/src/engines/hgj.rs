//! HGJ-lite — single-level Howgrave-Graham–Joux representation.
//!
//! Reference: Howgrave-Graham & Joux, "New generic algorithms for
//! hard knapsacks", EUROCRYPT 2010.  Original algorithm achieves
//! O(2^0.337n) by exploiting that a single solution has multiple
//! representations as (left + right) under a random hash modulus.
//!
//! Here we implement the *level-1* simplification: pick a random
//! modulus M = 2^(n/4), enumerate left-half subsets whose sums are
//! ≡ r (mod M) and right-half subsets ≡ (target - r) (mod M),
//! intersect.  Repeat for several random r.  Memory is O(2^(n/4))
//! per attempt — far below MITM's O(2^(n/2)).
//!
//! For provably optimal HGJ at full strength see BCJ (Becker-Coron-
//! Joux 2011) and Bonnetain et al. 2020 — those are Phase 5.
//!
//! This engine targets the regime n ∈ [40, 80] where Schroeppel-
//! Shamir starts to slow down but bitset DP can't fit the target.

use num_bigint::BigUint;
use std::collections::HashMap;

use crate::controller::{Engine, Shared};

pub struct HgjEngine;

const HGJ_MIN_N: usize = 40;
const HGJ_MAX_N: usize = 28;
const HGJ_ROUNDS: usize = 16;

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self { Self(seed | 1) }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
}

impl Engine for HgjEngine {
    fn name(&self) -> &'static str { "HGJ" }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < HGJ_MIN_N || p.n > HGJ_MAX_N {
            return;
        }
        if !p.u128_safe() {
            return;
        }
        let target = p.target_u128();
        let nums = p.numbers_u128();
        let n = nums.len();
        let mid = n / 2;
        let left = &nums[..mid];
        let right = &nums[mid..];

        // Choose modulus M ≈ 2^(n/4).  We use a power of two for
        // fast modular arithmetic.
        let m_bits = (n / 4).clamp(8, 24);
        let modulus: u128 = 1u128 << m_bits;
        let mod_mask: u128 = modulus - 1;

        let mut rng = Rng::new(0xCBF29CE484222325 ^ (n as u64));

        for round in 0..HGJ_ROUNDS {
            if sh.stopped() {
                return;
            }
            let r: u128 = (rng.next_u64() as u128) & mod_mask;

            // ---- Left side: enumerate subsets, group by sum mod M ----
            let mut left_buckets: HashMap<u128, Vec<(u128, u64)>> = HashMap::new();
            if left.len() > 30 {
                continue;
            }
            let l_total: u64 = 1u64 << left.len();
            for mask in 0..l_total {
                if sh.stopped() {
                    return;
                }
                let mut s: u128 = 0;
                let mut m = mask;
                let mut ok = true;
                while m != 0 {
                    let bit = m.trailing_zeros() as usize;
                    let (ns, of) = s.overflowing_add(left[bit]);
                    if of || ns > target {
                        ok = false;
                        break;
                    }
                    s = ns;
                    m &= m - 1;
                }
                if !ok {
                    continue;
                }
                let res = s & mod_mask;
                if res == r {
                    left_buckets.entry(s).or_default().push((s, mask));
                }
            }
            if left_buckets.is_empty() {
                continue;
            }

            // ---- Right side: enumerate subsets where sum ≡ (target - r) mod M ----
            if right.len() > 30 {
                continue;
            }
            let need_res = target.wrapping_sub(r) & mod_mask;
            let r_total: u64 = 1u64 << right.len();
            for mask in 0..r_total {
                if sh.stopped() {
                    return;
                }
                let mut s: u128 = 0;
                let mut m = mask;
                let mut ok = true;
                while m != 0 {
                    let bit = m.trailing_zeros() as usize;
                    let (ns, of) = s.overflowing_add(right[bit]);
                    if of || ns > target {
                        ok = false;
                        break;
                    }
                    s = ns;
                    m &= m - 1;
                }
                if !ok {
                    continue;
                }
                let res = s & mod_mask;
                if res != need_res {
                    continue;
                }
                let comp = target.wrapping_sub(s);
                if let Some(matches) = left_buckets.get(&comp) {
                    if let Some((_ls, lmask)) = matches.first() {
                        let mut sol: Vec<BigUint> = Vec::new();
                        let mut lm = *lmask;
                        while lm != 0 {
                            let bit = lm.trailing_zeros() as usize;
                            sol.push(BigUint::from(left[bit]));
                            lm &= lm - 1;
                        }
                        let mut rm = mask;
                        while rm != 0 {
                            let bit = rm.trailing_zeros() as usize;
                            sol.push(BigUint::from(right[bit]));
                            rm &= rm - 1;
                        }
                        sh.report(sol, "HGJ");
                        return;
                    }
                }
            }
            let _ = round;
        }
    }
}
