//! GDEP — Goal-Driven Element Partitioning (Rehan's original invention)
//! Now with u128 fast path for hard 64-bit instances.
//!
//! Core idea: dynamically restrict remaining elements to only those
//! smaller than or equal to the current remainder. This shrinks BOTH
//! the goal AND the element pool at every step — unlike MITM (split
//! elements only) or sum-range (split target only).

use num_bigint::BigUint;
use num_traits::Zero;
use crate::controller::{Engine, Shared};

pub struct GdepEngine;

impl Engine for GdepEngine {
    fn name(&self) -> &'static str { "GDEP" }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.target.is_zero() { sh.report(vec![], "GDEP"); return; }
        if p.n == 0 { return; }

        // u128 fast path: zero BigUint allocations
        if p.u128_safe() {
            self.run_u128(sh);
        } else {
            self.run_biguint(sh);
        }
    }
}

impl GdepEngine {
    fn run_u128(&self, sh: &Shared) {
        let p = &sh.profile;
        let target = p.target_u128();
        let nums = p.numbers_u128();
        let n = nums.len();

        // Sort descending: try largest elements first (GDEP core strategy)
        let mut sorted = nums.to_vec();
        sorted.sort_unstable_by(|a, b| b.cmp(a));

        // Suffix sums: suf[i] = sum of sorted[i..] (max achievable from position i)
        let mut suf = vec![0u128; n + 1];
        for i in (0..n).rev() {
            suf[i] = suf[i + 1].wrapping_add(sorted[i]);
        }

        // DFS with u128 arithmetic — no BigUint in hot path
        let mut path: Vec<u128> = Vec::with_capacity(n.min(32));
        if gdep_dfs_u128(&sorted, &suf, target, 0, &mut path, sh) {
            let sol: Vec<BigUint> = path.into_iter().map(BigUint::from).collect();
            sh.report(sol, "GDEP");
        }
    }

    fn run_biguint(&self, sh: &Shared) {
        let p = &sh.profile;
        let target = &p.target;
        let n = p.n;

        let mut ordered: Vec<BigUint> = p.numbers.clone();
        ordered.sort_by(|a, b| {
            let da = if a > target { BigUint::from(u64::MAX) } else { target - a };
            let db = if b > target { BigUint::from(u64::MAX) } else { target - b };
            da.cmp(&db)
        });

        let mut suf: Vec<BigUint> = vec![BigUint::zero(); n + 1];
        for i in (0..n).rev() {
            suf[i] = &suf[i + 1] + &ordered[i];
        }
        let mut path: Vec<BigUint> = Vec::new();
        let zero = BigUint::zero();
        if gdep_dfs_big(&ordered, &suf, target, 0, n, &mut path, &zero, sh) {
            sh.report(path, "GDEP");
        }
    }
}

fn gdep_dfs_u128(
    nums: &[u128], suf: &[u128], remaining: u128,
    start: usize, path: &mut Vec<u128>, sh: &Shared,
) -> bool {
    if remaining == 0 { return true; }
    if sh.stopped() { return false; }
    if start >= nums.len() { return false; }

    for i in start..nums.len() {
        let v = nums[i];
        if v > remaining { continue; }
        if suf[i] < remaining { return false; } // Can't reach target even with all remaining

        path.push(v);
        let new_rem = remaining - v;
        if new_rem == 0 { return true; }
        if suf[i + 1] >= new_rem {
            if gdep_dfs_u128(nums, suf, new_rem, i + 1, path, sh) {
                return true;
            }
        }
        path.pop();
    }
    false
}

fn gdep_dfs_big(
    nums: &[BigUint], suf: &[BigUint], remaining: &BigUint,
    start: usize, n: usize, path: &mut Vec<BigUint>,
    _current_sum: &BigUint, sh: &Shared,
) -> bool {
    if remaining.is_zero() { return true; }
    if sh.stopped() || start >= n { return false; }

    for i in start..n {
        let v = &nums[i];
        if v > remaining { continue; }
        if suf[i] < *remaining { return false; }

        path.push(v.clone());
        let new_target = remaining - v;
        if new_target.is_zero() { return true; }
        if suf[i + 1] >= new_target {
            if gdep_dfs_big(nums, suf, &new_target, i + 1, n, path, _current_sum, sh) {
                return true;
            }
        }
        path.pop();
    }
    false
}
