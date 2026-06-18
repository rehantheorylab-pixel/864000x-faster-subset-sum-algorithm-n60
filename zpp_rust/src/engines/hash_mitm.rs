//! HashMITM — Pure u128 Meet-in-the-Middle with HashMap collision
//!
//! Fundamentally different from Schroeppel-Shamir (no heaps, no two-pointer walk).
//! Splits elements into two halves, generates ALL subset sums for each half
//! using u128 Gray-code, then uses O(1) HashMap lookup for collision detection.
//!
//! For n=44: 2^22 = 4M sums per half → ~100MB memory → ~0.3s total.

use num_bigint::BigUint;
use std::collections::HashMap;

use crate::controller::{Engine, Shared};

pub struct HashMitmEngine;

const HM_MIN_N: usize = 20;
const HM_MAX_HALF: usize = 24; // 2^24 = 16M entries, ~384MB

impl Engine for HashMitmEngine {
    fn name(&self) -> &'static str { "HashMITM" }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < HM_MIN_N || !p.u128_safe() { return; }

        let target = p.target_u128();
        let nums = p.numbers_u128();
        let n = nums.len();

        let half = (n / 2).min(HM_MAX_HALF);
        if half < 2 { return; }

        // Left half: all 2^half subset sums → HashMap<sum, mask>
        if sh.stopped() { return; }
        let left_map = build_hashmap(&nums[..half], target);
        if sh.stopped() { return; }
        if left_map.is_empty() { return; }

        // Right half: check for target - right_sum in left map
        let right_n = (n - half).min(HM_MAX_HALF);
        let right_elems = &nums[half..(half + right_n)];

        let total_rmasks = 1u64 << right_n;
        let mut rpref = vec![0u128; right_n + 1];
        for i in 0..right_n {
            rpref[i + 1] = rpref[i].wrapping_add(right_elems[i]);
        }

        let mut rs: u128 = 0;
        for rmask in 0u64..total_rmasks {
            if rmask > 0 {
                let k = rmask.trailing_zeros() as usize;
                rs = rs.wrapping_add(right_elems[k]).wrapping_sub(rpref[k]);
            }
            if rs > target { continue; }
            if (rmask & 0xFFF) == 0 && sh.stopped() { return; }

            let need = target - rs;
            if let Some(&lmask) = left_map.get(&need) {
                // Found! Combine masks and rebuild solution
                let combined = lmask | (rmask << half as u32);
                let mut sol: Vec<BigUint> = Vec::new();
                let mut m = combined;
                for &v in nums.iter() {
                    if m & 1 != 0 { sol.push(BigUint::from(v)); }
                    m >>= 1;
                }
                sh.report(sol, "HashMITM");
                return;
            }
        }
    }
}

fn build_hashmap(elems: &[u128], target: u128) -> HashMap<u128, u64> {
    let n = elems.len();
    let total = 1u64 << n;
    let mut map: HashMap<u128, u64> = HashMap::with_capacity(total as usize);

    let mut pref = vec![0u128; n + 1];
    for i in 0..n {
        pref[i + 1] = pref[i].wrapping_add(elems[i]);
    }

    let mut s: u128 = 0;
    for mask in 0u64..total {
        if mask > 0 {
            let k = mask.trailing_zeros() as usize;
            s = s.wrapping_add(elems[k]).wrapping_sub(pref[k]);
        }
        if s <= target {
            map.entry(s).or_insert(mask);
        }
    }
    map
}
