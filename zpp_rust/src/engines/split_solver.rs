use num_bigint::BigUint;
use num_traits::Zero;

use crate::controller::{Engine, Shared};

/// Gap-based decomposition engine.
///
/// Key insight (O(n log n)): after sorting, if adjacent elements satisfy
///   sorted[i] + sorted[i+1] > target
/// then no solution can contain elements from BOTH sides of this gap
/// (any cross-pair exceeds target).  The problem decomposes into
/// independent segments — solve each with a fast subset-sum sweep
/// (MITM for segments up to 32, brute enumeration for ≤16).
pub struct SplitSolver;

impl Engine for SplitSolver {
    fn name(&self) -> &'static str {
        "SplitSolver"
    }

    fn run(&self, sh: &Shared) {
        let p = &sh.profile;
        if p.n < 4 || p.n > 48 || p.target.is_zero() {
            return;
        }

        let mut sorted: Vec<BigUint> = p.numbers.clone();
        sorted.sort();

        // ---- Step 1: Find gap split points ----
        // Split at i where sorted[i] + sorted[i+1] > target
        let mut splits: Vec<usize> = Vec::new();
        for i in 0..sorted.len() - 1 {
            if &sorted[i] + &sorted[i + 1] > p.target {
                splits.push(i + 1);
            }
        }
        if splits.is_empty() {
            return;
        }

        // ---- Step 2: Build segments ----
        let mut segments: Vec<&[BigUint]> = Vec::new();
        let mut start = 0usize;
        for &sp in &splits {
            if sp > start {
                segments.push(&sorted[start..sp]);
                start = sp;
            }
        }
        if start < sorted.len() {
            segments.push(&sorted[start..]);
        }
        if segments.len() < 2 {
            return;
        }

        // ---- Step 3: Solve each segment independently ----
        for seg in &segments {
            if sh.stopped() {
                return;
            }
            let seg_sum: BigUint = seg.iter().sum();
            if seg_sum < p.target {
                continue;
            }
            if let Some(sol) = solve_segment(seg, &p.target, sh) {
                sh.report(sol, "SplitSolver");
                return;
            }
        }
    }
}

fn solve_segment(seg: &[BigUint], target: &BigUint, sh: &Shared) -> Option<Vec<BigUint>> {
    if sh.stopped() {
        return None;
    }
    // Check if u128-safe for fast enumeration
    let u128_ok = target.bits() <= 128
        && seg.iter().all(|x| x.bits() <= 128);

    if u128_ok {
        solve_segment_u128(seg, target, sh)
    } else {
        solve_segment_bigint(seg, target, sh)
    }
}

fn solve_segment_u128(seg: &[BigUint], target: &BigUint, sh: &Shared) -> Option<Vec<BigUint>> {
    use num_traits::ToPrimitive;
    let t = target.to_u128().unwrap_or(0);
    let nums: Vec<u128> = seg.iter().map(|x| x.to_u128().unwrap_or(0)).collect();
    let n = nums.len();

    if n <= 12 {
        // Brute force all subsets (2^n)
        for mask in 1u32..(1u32 << n) {
            if sh.stopped() { return None; }
            let mut sum = 0u128;
            for j in 0..n {
                if mask & (1 << j) != 0 {
                    sum += nums[j];
                    if sum > t { break; }
                }
            }
            if sum == t {
                let mut sol: Vec<BigUint> = Vec::new();
                for j in 0..n {
                    if mask & (1 << j) != 0 {
                        sol.push(seg[j].clone());
                    }
                }
                return Some(sol);
            }
        }
    } else {
        // MITM for 13..=32
        let mid = n / 2;
        let left_subs = enumerate_sums_u128(&nums[..mid], t);
        if sh.stopped() { return None; }
        let right_subs = enumerate_sums_u128(&nums[mid..], t);
        if sh.stopped() { return None; }

        let mut right_map: std::collections::HashMap<u128, u64> = std::collections::HashMap::new();
        for &(sum, mask) in &right_subs {
            let entry = right_map.entry(sum).or_insert(0u64);
            *entry = mask;
        }
        for &(sum, mask) in &left_subs {
            let need = t - sum;
            if let Some(&rmask) = right_map.get(&need) {
                if sh.stopped() { return None; }
                let mut sol: Vec<BigUint> = Vec::new();
                for j in 0..mid {
                    if mask & (1 << j) != 0 {
                        sol.push(seg[j].clone());
                    }
                }
                for j in mid..n {
                    if rmask & (1 << (j - mid)) != 0 {
                        sol.push(seg[j].clone());
                    }
                }
                return Some(sol);
            }
        }
    }
    None
}

fn enumerate_sums_u128(nums: &[u128], target: u128) -> Vec<(u128, u64)> {
    let n = nums.len();
    let total = 1u64 << n;
    let mut results = Vec::with_capacity(total as usize);
    for mask in 0u64..total {
        let mut sum = 0u128;
        for j in 0..n {
            if mask & (1 << j) != 0 {
                sum += nums[j];
                if sum > target { break; }
            }
        }
        if sum <= target {
            results.push((sum, mask));
        }
    }
    results
}

fn solve_segment_bigint(seg: &[BigUint], target: &BigUint, sh: &Shared) -> Option<Vec<BigUint>> {
    let n = seg.len();
    if n <= 10 {
        for mask in 1u32..(1u32 << n) {
            if sh.stopped() { return None; }
            let mut sum = BigUint::zero();
            for j in 0..n {
                if mask & (1 << j) != 0 {
                    sum += &seg[j];
                    if sum > *target { break; }
                }
            }
            if sum == *target {
                let mut sol = Vec::new();
                for j in 0..n {
                    if mask & (1 << j) != 0 {
                        sol.push(seg[j].clone());
                    }
                }
                return Some(sol);
            }
        }
    }
    // MITM for BigUint
    let mid = n / 2;
    let left = enumerate_sums_bigint(&seg[..mid], target);
    if sh.stopped() { return None; }
    let right = enumerate_sums_bigint(&seg[mid..], target);
    if sh.stopped() { return None; }

    let mut right_map: std::collections::HashMap<BigUint, u64> = std::collections::HashMap::new();
    for (sum, mask) in &right {
        right_map.insert(sum.clone(), *mask);
    }
    for (sum, mask) in &left {
        if sum > target { continue; }
        let need = target - sum;
        if let Some(rmask) = right_map.get(&need) {
            if sh.stopped() { return None; }
            let mut sol = Vec::new();
            for j in 0..mid {
                if mask & (1 << j) != 0 {
                    sol.push(seg[j].clone());
                }
            }
            for j in mid..n {
                if rmask & (1 << (j - mid)) != 0 {
                    sol.push(seg[j].clone());
                }
            }
            return Some(sol);
        }
    }
    None
}

fn enumerate_sums_bigint(nums: &[BigUint], target: &BigUint) -> Vec<(BigUint, u64)> {
    let n = nums.len();
    let total = 1u64 << n;
    let mut results = Vec::new();
    for mask in 0u64..total {
        let mut sum = BigUint::zero();
        let mut overflow = false;
        for j in 0..n {
            if mask & (1 << j) != 0 {
                sum += &nums[j];
                if sum > *target { overflow = true; break; }
            }
        }
        if !overflow {
            results.push((sum, mask));
        }
    }
    results
}
